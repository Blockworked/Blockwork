use crate::config::MacroFiles;
use crate::input::types::{Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::{Evaluated, Op, Value};
use crate::macros::backend::{InputBackend, create_backend};
use crate::macros::host_clock;
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{
    BlockKind, ImageMatch, ImagePick, Instruction, InstructionKind, LaunchOptions, Macro, Role,
    StopScope, Strand,
};
use blockstitch_core::graph::{
    ListItem, ListStore, is_list_reporter, list_index, resolve_list_reporter,
};
use spin_sleep::{SpinSleeper, SpinStrategy};
use std::collections::HashMap;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;

/// Shared, macro-wide variable store, so a `Set`/`Change` in one strand is
/// visible to others running concurrently.
pub type VariableStore = Arc<Mutex<HashMap<String, Evaluated>>>;

/// A custom block's runtime shape: input names in prototype order (matched
/// positionally against call args) plus its body instructions. Built once
/// per run and shared read-only via `ExecCtx::block_table`.
pub struct BlockRuntime {
    pub input_names: Vec<String>,
    pub branch_names: Vec<String>,
    pub body: Vec<Instruction>,
    pub variable_defaults: Vec<(String, Evaluated)>,
}

/// Guards against runaway recursive custom blocks; without it a cycle would
/// eventually stack-overflow the whole process instead of failing cleanly.
/// Kept low because each level crosses several real Rust stack frames and
/// execution threads don't get an enlarged stack.
const MAX_CALL_DEPTH: u32 = 64;

/// Stack for each entry strand's thread. Interpreting a macro recurses per
/// nested call and body, which the default 2 MiB can't hold to `MAX_CALL_DEPTH`.
const STRAND_STACK_SIZE: usize = 8 * 1024 * 1024;

// Macro helpers run inside a desktop app; console executables must not
// allocate a terminal window each time a strand executes one of them.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How a block list ended. Loops catch `Break`/`Continue`; a custom block
/// call also stops them, so loop control never crosses into a block body.
#[derive(Debug, Clone, PartialEq)]
enum Flow {
    Normal,
    Return(Evaluated),
    Break,
    Continue,
}

/// Everything one strand's execution needs, threaded by `&mut` through
/// `run_block` and its recursive custom-block calls. `pressed_keys`/
/// `pressed_buttons` are shared across the whole call tree so cleanup only
/// happens once, at the outermost level. `param_env` is swapped (not
/// shared) around nested calls so each invocation's bound parameters stay
/// isolated from its caller's.
struct ExecCtx<'a> {
    emulator: Arc<Mutex<dyn InputBackend>>,
    gamepad: Option<crate::input::virtual_gamepad::Lease>,
    stop_flag: Option<StrandStop>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    block_table: BlockTable,
    pressed_keys: &'a mut Vec<MacroKey>,
    /// Mouse buttons held by this strand, tracked and cleaned up like
    /// `pressed_keys`. Without this, stopping between a press and release
    /// leaves the button physically stuck down for the rest of the session.
    pressed_buttons: &'a mut Vec<MacroButton>,
    param_env: HashMap<String, Evaluated>,
    branch_env: HashMap<String, Vec<Instruction>>,
    /// Names this run in the run log.
    source: Arc<crate::run_log::Source>,
    /// Block behind the error currently unwinding, for the run log.
    error_block: String,
    /// How many `TryCatch` bodies are executing; inside one, a failing
    /// block raises an error instead of being skipped.
    try_depth: u32,
    /// Publishes the block this strand is on, for the editor's highlight.
    trace: Option<crate::macros::trace::Tracker>,
    /// Iterations any one loop may run; 0 means no limit.
    max_loop_iterations: u64,
    /// Signal strands started without waiting; joined when this strand ends
    /// so the run lasts until they finish.
    signal_threads: Vec<std::thread::JoinHandle<()>>,
    /// Virtual time this strand has waited through in host-clock mode.
    host_time_ms: f64,
    /// Set when the backend can't send input; input blocks are skipped and
    /// the first one reports why.
    input_unavailable: Option<String>,
    input_unavailable_reported: bool,
}

impl<'a> ExecCtx<'a> {
    /// Resolves `value` into a tree `Value::eval()` can consume: macro-wide
    /// `Var` reads first (falling back to an empty environment if the lock
    /// is poisoned), then `Param` reads and any `Call` nodes.
    fn resolve(&mut self, value: &Value, depth: u32) -> Result<Value, String> {
        let vars_resolved = match self.variables.lock() {
            Ok(guard) => value.resolve_vars(&guard),
            Err(_) => value.resolve_vars(&HashMap::new()),
        };
        resolve_calls_and_params(&vars_resolved, self, depth)
    }
}

/// Walks a `Var`-resolved tree substituting `Param` leaves and `Call` nodes
/// (by actually running the callee's body and capturing its `Return`).
/// Lives here rather than in `input/value.rs` because it needs real
/// execution capability that module deliberately doesn't have.
fn resolve_calls_and_params(value: &Value, ctx: &mut ExecCtx, depth: u32) -> Result<Value, String> {
    match value {
        Value::Number { value } => Ok(Value::Number { value: *value }),
        Value::Text { value } => Ok(Value::Text {
            value: value.clone(),
        }),
        Value::Bool => Ok(Value::Bool),
        // Shouldn't appear here (resolve_vars already ran), but a harmless
        // passthrough rather than a hard error keeps this function total.
        Value::Var { name } => Ok(Value::Var { name: name.clone() }),
        Value::Param { name } => Ok(match ctx.param_env.get(name) {
            Some(e) => e.clone().into_value(),
            None => Value::number(0.0),
        }),
        Value::Op { op, args, .. } if is_list_reporter(op) => {
            let args = args
                .iter()
                .map(|arg| resolve_calls_and_params(arg, ctx, depth))
                .collect::<Result<Vec<_>, _>>()?;
            let name: Box<str> = match op {
                Op::Ext(name) => name.clone(),
                _ => unreachable!("validated by is_list_reporter"),
            };
            let lists = ctx
                .lists
                .lock()
                .map(|lists| lists.clone())
                .unwrap_or_default();
            resolve_list_reporter(&name, args, &lists)
        }
        Value::Op { op, args, saved } => {
            let args = args
                .iter()
                .map(|a| resolve_calls_and_params(a, ctx, depth))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Op {
                op: op.clone(),
                args,
                saved: saved.clone(),
            })
        }
        Value::Call {
            block_id,
            args,
            branches,
            ..
        } => {
            // Args evaluate in the caller's scope, fully resolved to
            // concrete values before call_block swaps in the callee's scope.
            let mut evaluated_args = Vec::with_capacity(args.len());
            for a in args {
                let resolved = resolve_calls_and_params(a, ctx, depth)?;
                evaluated_args.push(resolved.eval()?);
            }
            let branch_values = branches
                .iter()
                .map(|body| crate::frontend::reporter_branch_to_instructions(body))
                .collect::<Result<Vec<_>, _>>()?;
            match call_block(block_id, evaluated_args, branch_values, ctx, depth + 1)? {
                Some(e) => Ok(e.into_value()),
                None => Err(format!("custom block '{block_id}' didn't return a value")),
            }
        }
    }
}

/// Runs custom block `block_id` with `arg_values` bound to its declared
/// inputs, returning whatever `Return` produced (`None` if it ran to
/// completion without one). Shared by both the value-position `Value::Call`
/// and command-position `InstructionKind::CallBlock` call sites.
fn call_block(
    block_id: &str,
    arg_values: Vec<Evaluated>,
    branch_values: Vec<Vec<Instruction>>,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Option<Evaluated>, String> {
    if depth > MAX_CALL_DEPTH {
        return Err("custom block call depth exceeded (possible infinite recursion)".to_string());
    }
    // Clone the Arc (not the table) to avoid a borrow-checker conflict
    // between reading block_table and mutating param_env below.
    let block_table = Arc::clone(&ctx.block_table);
    let runtime = block_table
        .get(block_id)
        .ok_or_else(|| format!("call to unknown custom block '{block_id}'"))?;
    if !runtime.variable_defaults.is_empty()
        && let Ok(mut vars) = ctx.variables.lock()
    {
        for (key, value) in &runtime.variable_defaults {
            vars.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    let new_env: HashMap<String, Evaluated> = runtime
        .input_names
        .iter()
        .cloned()
        .zip(arg_values)
        .collect();
    let new_branches: HashMap<String, Vec<Instruction>> = runtime
        .branch_names
        .iter()
        .cloned()
        .zip(branch_values)
        .collect();
    let saved_env = std::mem::replace(&mut ctx.param_env, new_env);
    let saved_branches = std::mem::replace(&mut ctx.branch_env, new_branches);
    let result = run_block(&runtime.body, ctx, depth, Instant::now());
    ctx.param_env = saved_env;
    ctx.branch_env = saved_branches;
    // A stray Break/Continue reaching a custom block's own top level (no
    // enclosing loop within its body) is absorbed here, same as Normal -
    // a custom block is its own execution context, not an extension of the
    // caller's loop.
    result.map(|flow| match flow {
        Flow::Return(v) => Some(v),
        Flow::Normal | Flow::Break | Flow::Continue => None,
    })
}

thread_local! {
    /// Message of the error the innermost running `TryCatch` caught. Strands
    /// run on their own threads, so this is per strand.
    static LAST_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Backs the `LastError` reporter.
pub fn last_error() -> String {
    LAST_ERROR.with(|e| e.borrow().clone())
}

/// A block couldn't do its job. Inside a `TryCatch` that's an error the
/// catch handles; elsewhere the block is skipped and the run log says why.
fn soft_fail(
    ctx: &mut ExecCtx,
    block_id: &str,
    block: &str,
    error: impl std::fmt::Display,
) -> Result<(), String> {
    let message = format!("{block}: {error}");
    if ctx.try_depth > 0 {
        ctx.error_block = block_id.to_string();
        return Err(message);
    }
    warn!("{message}");
    crate::run_log::block_error(&ctx.source, block_id, message);
    Ok(())
}

// Optional wait limits use real time, just like the existing wait blocks.
fn optional_number(
    ctx: &mut ExecCtx,
    depth: u32,
    value: Option<&Value>,
    fallback: f64,
) -> Result<f64, String> {
    value.map_or(Ok(fallback), |v| ctx.resolve(v, depth)?.eval_number())
}

fn wait_limit_reached(
    ctx: &mut ExecCtx,
    block_id: &str,
    started: Instant,
    timeout: f64,
    error_timeout: f64,
    message: &str,
) -> Result<bool, String> {
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    // If both are present, the earlier deadline wins; errors win a tie.
    if error_timeout > 0.0
        && (timeout <= 0.0 || error_timeout <= timeout)
        && elapsed >= error_timeout
    {
        ctx.error_block = block_id.to_string();
        return Err(format!("{message} after {error_timeout} ms"));
    }
    Ok(timeout > 0.0 && elapsed >= timeout)
}

/// Counts one loop iteration. Past the macro's loop limit it reports the
/// loop (an error inside a try) and returns `true`: stop looping.
fn loop_limit_hit(ctx: &mut ExecCtx, block_id: &str, iterations: &mut u64) -> Result<bool, String> {
    *iterations += 1;
    if ctx.max_loop_iterations == 0 || *iterations <= ctx.max_loop_iterations {
        return Ok(false);
    }
    let limit = ctx.max_loop_iterations;
    soft_fail(
        ctx,
        block_id,
        "Loop",
        format!("stopped after {limit} iterations (this macro's loop limit)"),
    )?;
    Ok(true)
}

/// Sleeps `ms` (scaled by playback speed), waking early when the run is
/// stopped. Returns `true` if it was.
fn sleep_unless_stopped(ctx: &ExecCtx, ms: f64) -> bool {
    let secs = (ms / ctx.speed_multiplier).max(0.0) / 1000.0;
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    loop {
        if ctx.stop_flag.as_ref().is_some_and(stop_requested) {
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        std::thread::sleep((deadline - now).min(STOP_POLL_INTERVAL));
    }
}

static EMULATOR_FAILED: AtomicBool = AtomicBool::new(false);

pub fn emulator_failed() -> bool {
    EMULATOR_FAILED.load(Ordering::Relaxed)
}

fn spin_sleeper() -> &'static SpinSleeper {
    static SLEEPER: OnceLock<SpinSleeper> = OnceLock::new();
    SLEEPER.get_or_init(|| SpinSleeper::default().with_spin_strategy(SpinStrategy::SpinLoopHint))
}

/// Polling granularity during a `Wait`, so a stop signal lands quickly
/// instead of only being noticed once the full wait elapses.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(15);
/// How often wait-until blocks re-check (on top of the check's own time).
const CONDITION_POLL_MS: f64 = 100.0;
/// How long a browser block waits for the extension's answer; it waits up
/// to 5 s itself for an element to show up.
const BROWSER_LIMIT: Duration = Duration::from_secs(20);
const BROWSER_LOAD_LIMIT: Duration = Duration::from_secs(40);
// Long enough to allow a new script in the browser's approval window.
const BROWSER_SCRIPT_LIMIT: Duration = Duration::from_secs(120);
/// A box this close to where the aimed-at target should be is taken to be it.
const RELOCK_MIN_PX: f64 = 40.0;
/// How often the aim mover sends a move: 500 Hz, like a gaming mouse.
const GLIDE_TICK: f64 = 0.002;
/// How far behind the capture call a captured frame is; measured at about
/// 10 ms with Minecraft on KWin.
const FRAME_LAG: f64 = 0.010;

// Time for the clipboard to take the new text, and for the app to read it.
const PASTE_SETTLE: Duration = Duration::from_millis(80);
const PASTE_RESTORE: Duration = Duration::from_millis(400);

/// Pastes `text` with Ctrl+V (Cmd+V on macOS), then restores the clipboard
/// if it held text.
fn paste_text(ctx: &ExecCtx, text: String) -> Result<(), String> {
    let previous = crate::clipboard::get_text().ok();
    crate::clipboard::set_text(text)?;
    std::thread::sleep(PASTE_SETTLE);
    {
        let mut em = ctx.emulator.lock().map_err(|e| e.to_string())?;
        let modifier = if cfg!(target_os = "macos") {
            MacroKey::Meta
        } else {
            MacroKey::LControl
        };
        em.key(modifier.clone(), Direction::Press)?;
        let pressed = em.key(MacroKey::Unicode('v'), Direction::Click);
        em.key(modifier, Direction::Release)?;
        pressed?;
    }
    std::thread::sleep(PASTE_RESTORE);
    if let Some(previous) = previous {
        crate::clipboard::set_text(previous)?;
    }
    Ok(())
}

/// Custom block bodies keyed by block id, shared read-only by a run's strands.
pub type BlockTable = Arc<HashMap<String, BlockRuntime>>;

/// Block table key prefix for `WhenSignal` strands; can't clash with a block id.
const SIGNAL_KEY: &str = "\u{1}signal:";

/// The bodies of this run's `WhenSignal` strands listening for `name`.
fn signal_bodies(ctx: &mut ExecCtx, name: &str, depth: u32) -> Vec<Vec<Instruction>> {
    let table = Arc::clone(&ctx.block_table);
    let mut keys: Vec<_> = table.keys().filter(|k| k.starts_with(SIGNAL_KEY)).collect();
    keys.sort();
    keys.into_iter()
        .filter_map(|key| {
            let body = &table[key].body;
            let Some(InstructionKind::WhenSignal(listens)) = body.first().map(|i| &i.kind) else {
                return None;
            };
            let listens = ctx
                .resolve(&listens.value, depth)
                .and_then(|v| v.eval_text())
                .ok()?;
            listens
                .trim()
                .eq_ignore_ascii_case(name.trim())
                .then(|| body[1..].to_vec())
        })
        .collect()
}

/// `bash -c command`, for the run command blocks.
fn shell_process(command: &str) -> Command {
    let mut process = Command::new("bash");
    #[cfg(windows)]
    process.creation_flags(CREATE_NO_WINDOW);
    process.args(["-c", command]);
    process
}

/// Shows a dialog for this strand. `None` when there's no answer to use:
/// cancelled (which stops the run unless a try block catches it), stopped,
/// or failed.
fn ask(
    ctx: &mut ExecCtx,
    block_id: &str,
    block: &str,
    request: crate::prompt::Request,
) -> Result<Option<crate::prompt::Reply>, String> {
    let flag = ctx.stop_flag.clone();
    let stopped = || flag.as_ref().is_some_and(stop_requested);
    match crate::prompt::ask(&request, &stopped) {
        Ok(reply) if reply.answer == "cancel" => {
            if ctx.try_depth > 0 {
                ctx.error_block = block_id.to_string();
                return Err(format!("{block}: cancelled"));
            }
            crate::run_log::info(
                &ctx.source,
                format!("{block}: cancelled, so the run stopped"),
            );
            if let Some(flag) = &ctx.stop_flag {
                flag.stop(StopScope::All);
            }
            Ok(None)
        }
        Ok(reply) => Ok(Some(reply)),
        Err(_) if stopped() => Ok(None),
        Err(e) => {
            soft_fail(ctx, block_id, block, e)?;
            Ok(None)
        }
    }
}

fn set_variable(ctx: &ExecCtx, name: &str, value: Evaluated) {
    if name.is_empty() {
        return;
    }
    if let Ok(mut vars) = ctx.variables.lock() {
        vars.insert(name.to_string(), value);
    }
}

mod apps;
mod browser;
mod control;
mod data;
mod dialogs;
mod input;
mod stop;
mod system;
mod vision;
use apps::*;
use stop::{StrandStop, stop_requested};

/// How a Run plays: what presses the keys, how it is stopped, and the
/// stores its blocks read and write.
pub struct RunOptions {
    pub emulator: Arc<Mutex<dyn InputBackend>>,
    /// Cleared to stop the run; `None` gives the run a flag of its own.
    pub stop_flag: Option<Arc<Mutex<bool>>>,
    /// Scales every `Wait` inversely (2.0 runs twice as fast).
    pub speed_multiplier: f64,
    pub variables: VariableStore,
    /// The run's lists; `None` starts from the macro's saved lists. A host
    /// passes its own to save what the run changed.
    pub lists: Option<ListStore>,
    /// Pretends the run started this long ago, so a run that starts late
    /// (a synced start) keeps its waits on the intended schedule.
    pub offset: Duration,
}

impl RunOptions {
    /// Full speed, no stop flag, saved lists, no offset.
    pub fn new(emulator: Arc<Mutex<dyn InputBackend>>, variables: VariableStore) -> Self {
        Self {
            emulator,
            stop_flag: None,
            speed_multiplier: 1.0,
            variables,
            lists: None,
            offset: Duration::ZERO,
        }
    }
}

/// Running a macro and the stores a run reads; `Macro` itself lives in
/// `blockwork_model`, which can't run anything.
pub trait Run {
    /// A fresh variable store holding the macro's saved values.
    fn variable_store(&self) -> VariableStore;

    /// Runs to the end, then saves the variables and lists the run finished
    /// with. For hosts with no open editor copy to merge into.
    fn run_and_save(self, options: RunOptions) -> Result<(), String>;

    /// A fresh list store holding the macro's saved lists.
    fn list_store(&self) -> ListStore;

    /// Builds the custom block table from this macro's `BlockHeader` strands.
    fn block_table(&self) -> BlockTable;

    /// Runs every "When Ran" entry-point strand concurrently on its own
    /// thread, sharing one `stop_flag`, from the macro's saved lists. A custom
    /// block's header strand isn't run directly - its body is extracted into
    /// `block_table` and only executes via `CallBlock`/`Value::Call`. Blocks
    /// until every strand finishes or is stopped. [`Run::run_with`] takes
    /// the rest of the options.
    fn run(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
    );

    /// [`Run::run`] with every option. Trigger strands are fired by the
    /// app's watchers via `run_fired_strand`, never by a manual Run.
    fn run_with(self, options: RunOptions);

    /// Runs just the strand `strand_id`, from its top block, whatever event
    /// heads it. False when there's no such strand or it's a custom block's
    /// definition, which only runs when called.
    fn run_one_strand(self, strand_id: &str, options: RunOptions) -> bool;

    /// Whether `run_one_strand` would run `strand_id`.
    fn can_run_strand(&self, strand_id: &str) -> bool;
}

impl Run for Macro {
    fn variable_store(&self) -> VariableStore {
        Arc::new(Mutex::new(
            self.variables
                .iter()
                .map(|v| (v.name.clone(), v.value.clone()))
                .collect(),
        ))
    }

    fn run_and_save(self, mut options: RunOptions) -> Result<(), String> {
        let lists = Arc::clone(options.lists.get_or_insert_with(|| self.list_store()));
        let variables = Arc::clone(&options.variables);
        let mut saved = self.clone();
        self.run_with(options);
        if let Ok(values) = variables.lock() {
            saved.sync_variables_from(&values);
        }
        if let Ok(values) = lists.lock() {
            saved.sync_lists_from(&values);
        }
        saved.save()
    }

    fn list_store(&self) -> ListStore {
        Arc::new(Mutex::new(
            self.lists
                .iter()
                .map(|list| (list.name.clone(), list.items.clone()))
                .collect(),
        ))
    }

    fn block_table(&self) -> BlockTable {
        let mut table = HashMap::new();
        // Signal strands ride along under reserved keys, header included,
        // so every run entry point can reach them without another argument.
        for (index, strand) in self.graph.strands.iter().enumerate() {
            if matches!(
                strand.instructions.first().map(|i| &i.kind),
                Some(InstructionKind::WhenSignal(_))
            ) {
                let runtime = BlockRuntime {
                    input_names: Vec::new(),
                    branch_names: Vec::new(),
                    body: strand.instructions.clone(),
                    variable_defaults: Vec::new(),
                };
                table.insert(format!("{SIGNAL_KEY}{index}"), runtime);
            }
        }
        for strand in &self.graph.strands {
            let Some(InstructionKind::BlockHeader(id)) =
                strand.instructions.first().map(|i| &i.kind)
            else {
                continue;
            };
            if let Some(def) = self.graph.block_defs.iter().find(|b| &b.id == id) {
                table.insert(
                    id.clone(),
                    BlockRuntime {
                        input_names: def.input_names().map(str::to_string).collect(),
                        branch_names: def.branch_names().map(str::to_string).collect(),
                        body: strand.instructions[1..].to_vec(),
                        variable_defaults: Vec::new(),
                    },
                );
            }
        }
        for pack in &self.settings.block_packs {
            let defaults: Vec<_> = pack
                .variables
                .iter()
                .map(|v| (pack.variable_key(&v.name), v.value.clone()))
                .collect();
            for id in &pack.blocks {
                if crate::block_packs::owner(self, id).is_some_and(|owner| owner.id != pack.id) {
                    continue;
                }
                if let Some(runtime) = table.get_mut(id) {
                    crate::block_packs::scope_body(&mut runtime.body, pack);
                    runtime.variable_defaults = defaults.clone();
                }
            }
            for var in pack
                .variables
                .iter()
                .filter(|v| v.role == crate::block_packs::VariableRole::Output)
            {
                table.insert(
                    pack.output_id(&var.name),
                    BlockRuntime {
                        input_names: Vec::new(),
                        branch_names: Vec::new(),
                        body: vec![Instruction::new(InstructionKind::Return(Value::Var {
                            name: pack.variable_key(&var.name),
                        }))],
                        variable_defaults: defaults.clone(),
                    },
                );
            }
        }
        Arc::new(table)
    }

    fn run(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
    ) {
        self.run_with(RunOptions {
            stop_flag,
            speed_multiplier,
            ..RunOptions::new(emulator, variables)
        });
    }

    fn run_with(self, options: RunOptions) {
        run_entries(
            self,
            |s| {
                matches!(
                    s.instructions.first().map(|i| &i.kind),
                    Some(InstructionKind::WhenRan)
                )
            },
            options,
        )
    }

    fn run_one_strand(self, strand_id: &str, options: RunOptions) -> bool {
        if !self.can_run_strand(strand_id) {
            return false;
        }
        run_entries(self, |s| s.id == strand_id, options);
        true
    }

    fn can_run_strand(&self, strand_id: &str) -> bool {
        self.graph.strand(strand_id).is_some_and(|s| {
            !s.instructions.is_empty()
                && !matches!(s.instructions[0].kind, InstructionKind::BlockHeader(_))
        })
    }
}

/// Runs every strand `pick` accepts side by side, each on its own thread.
fn run_entries(mac: Macro, pick: impl Fn(&Strand) -> bool, options: RunOptions) {
    let RunOptions {
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        lists,
        offset: initial_offset,
    } = options;
    let stop = StrandStop::new(stop_flag, &mac.id);
    let run = StrandRun {
        source: Arc::new(crate::run_log::Source::for_macro(&mac.id, &mac.name)),
        emulator,
        stop_flag: Some(stop.clone()),
        speed_multiplier,
        variables,
        lists: lists.unwrap_or_else(|| mac.list_store()),
        block_table: mac.block_table(),
        max_loop_iterations: mac.settings.max_loop_iterations,
    };
    let entry_strands: Vec<(Vec<Instruction>, StrandStop)> = mac
        .graph
        .strands
        .into_iter()
        .filter(|s| pick(s))
        .map(|s| (s.instructions, stop.fork()))
        .collect();

    if entry_strands.is_empty() {
        return;
    }
    if host_clock::enabled() {
        host_clock::attach(entry_strands.len());
    }
    // Every entry strand gets its own thread, including the first, so
    // deep recursion errors out on a known stack instead of aborting.
    std::thread::scope(|scope| {
        for (instructions, stop) in entry_strands {
            let mut run = run.clone();
            run.stop_flag = Some(stop);
            let spawned = std::thread::Builder::new()
                .stack_size(STRAND_STACK_SIZE)
                .spawn_scoped(scope, move || run_strand(run, instructions, initial_offset));
            if let Err(e) = spawned {
                warn!("Failed to start a strand thread: {e}");
            }
        }
    });
}

/// Runs a function when dropped.
struct Defer(Option<fn()>);

impl Drop for Defer {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

/// What every strand of one run shares.
#[derive(Clone)]
pub(super) struct StrandRun {
    pub(super) source: Arc<crate::run_log::Source>,
    pub(super) emulator: Arc<Mutex<dyn InputBackend>>,
    pub(super) stop_flag: Option<StrandStop>,
    pub(super) speed_multiplier: f64,
    pub(super) variables: VariableStore,
    pub(super) lists: ListStore,
    pub(super) block_table: BlockTable,
    pub(super) max_loop_iterations: u32,
}

/// Per-thread entry point for one strand: runs it via `run_block`, then
/// releases any keys/buttons still held, whether the strand finished,
/// stopped, or errored. A `Return` reaching strand level has nowhere to hand
/// its value, so it is dropped.
pub(super) fn run_strand(run: StrandRun, instructions: Vec<Instruction>, initial_offset: Duration) {
    let StrandRun {
        source,
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        lists,
        block_table,
        max_loop_iterations,
    } = run;
    let _detach = host_clock::enabled().then(|| Defer(Some(host_clock::detach)));
    raise_current_thread_priority();
    // Priority-raise happens first (its latency shouldn't eat into the
    // offset), then anchor "now" for this strand's Wait chain, backdated by
    // initial_offset. `checked_sub` guards the (only theoretically
    // reachable) case of an offset larger than the process's own uptime.
    let start = Instant::now()
        .checked_sub(initial_offset)
        .unwrap_or_else(Instant::now);
    let mut pressed_keys: Vec<MacroKey> = Vec::new();
    let mut pressed_buttons: Vec<MacroButton> = Vec::new();
    let permission_stop = stop_flag.clone();
    let _permissions = crate::permissions::enter_with_stop(
        Arc::clone(&source),
        Arc::new(move || permission_stop.as_ref().is_some_and(stop_requested)),
    );
    {
        let mut ctx = ExecCtx {
            gamepad: None,
            emulator: Arc::clone(&emulator),
            stop_flag,
            speed_multiplier,
            variables,
            lists,
            block_table,
            pressed_keys: &mut pressed_keys,
            pressed_buttons: &mut pressed_buttons,
            param_env: HashMap::new(),
            branch_env: HashMap::new(),
            source: Arc::clone(&source),
            error_block: String::new(),
            try_depth: 0,
            trace: crate::macros::trace::Tracker::new(&source.macro_id),
            max_loop_iterations: max_loop_iterations.into(),
            signal_threads: Vec::new(),
            host_time_ms: 0.0,
            input_unavailable: emulator.lock().ok().and_then(|em| em.unavailable_reason()),
            input_unavailable_reported: false,
        };
        // An error reaching the top ends this strand.
        if let Err(e) = run_block(&instructions, &mut ctx, 0, start) {
            warn!("Strand ended with an error: {e}");
            crate::run_log::block_error(&source, &ctx.error_block, e);
        }
        for thread in ctx.signal_threads.drain(..) {
            let _ = thread.join();
        }
    }

    if !pressed_keys.is_empty() || !pressed_buttons.is_empty() {
        if let Ok(mut em) = emulator.lock() {
            for key in pressed_keys.into_iter().rev() {
                if let Err(err) = em.key(key.clone(), Direction::Release) {
                    warn!("Failed to release key {:?} during cleanup: {}", key, err);
                }
            }
            for button in pressed_buttons.into_iter().rev() {
                if let Err(err) = em.button(button.clone(), Direction::Release) {
                    warn!(
                        "Failed to release button {:?} during cleanup: {}",
                        button, err
                    );
                }
            }
        } else {
            warn!("Failed to lock emulator mutex for input cleanup");
        }
    }
}

/// Runs `instructions` with no custom blocks or lists, for tests and
/// callers that don't need them.
pub fn run_instructions(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
) {
    run_fired_strand(
        crate::run_log::Source::default(),
        instructions,
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        0,
    );
}

/// Runs one trigger-fired strand body, logging as `source`.
/// `block_table` comes from `Macro::block_table` so custom block calls resolve.
#[allow(clippy::too_many_arguments)]
pub fn run_fired_strand(
    source: crate::run_log::Source,
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    block_table: BlockTable,
    max_loop_iterations: u32,
) {
    let stop = StrandStop::new(stop_flag, &source.macro_id).fork();
    let run = StrandRun {
        source: Arc::new(source),
        emulator,
        stop_flag: Some(stop),
        speed_multiplier,
        variables,
        lists,
        block_table,
        max_loop_iterations,
    };
    run_strand(run, instructions, Duration::ZERO);
}

/// Runs `instructions` in order, sending each block to its category's file.
/// Returns early on a return or loop-control block; `Err` is a hard failure
/// (too deep, or a bad return value). The stop flag is checked between blocks.
fn run_block(
    instructions: &[Instruction],
    ctx: &mut ExecCtx,
    depth: u32,
    start: Instant,
) -> Result<Flow, String> {
    if depth > MAX_CALL_DEPTH {
        return Err("custom block call depth exceeded (possible infinite recursion)".to_string());
    }

    // Each block list keeps its own wait schedule. Only a strand's top
    // level starts backdated (`RunOptions::offset`).
    let mut deadline = start;

    for ins in instructions {
        if let Some(flag) = &ctx.stop_flag
            && stop_requested(flag)
        {
            break;
        }
        if let Some(trace) = &ctx.trace
            && !ins.kind.is_header()
        {
            trace.at(&ins.id);
        }
        if !ins.kind.is_header() && !matches!(ins.kind, InstructionKind::Comment(_)) {
            let stopped = || ctx.stop_flag.as_ref().is_some_and(stop_requested);
            let paused = crate::macros::debug::checkpoint(
                &ctx.source.macro_id,
                &ins.id,
                &stopped,
                &ctx.variables,
                &ctx.lists,
                &ctx.param_env,
            );
            if !paused.is_zero() {
                // Waits after the pause keep their spacing instead of catching up.
                deadline += paused;
                if stopped() {
                    break;
                }
            }
        }

        if let Some(Err(e)) = crate::capabilities::checked_when_run(&ins.kind)
            .map(crate::permissions::require)
            .find(Result::is_err)
        {
            soft_fail(ctx, &ins.id, "Permission", e)?;
            continue;
        }

        // `Header` and `Trigger` blocks are never reached: `run_with` leaves
        // their strands out of `entry_strands`, and a watcher starts from the
        // block after the header.
        match ins.kind.info().role {
            Role::Header | Role::Trigger | Role::Comment => {}
            Role::Dialogs => {
                if let Some(flow) = dialogs::run_dialog_block(ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
            Role::System => system::run_system_block(ins, ctx, depth)?,
            Role::Control => match control::run_control_block(ins, ctx, depth, &mut deadline)? {
                control::Next::Go => {}
                control::Next::Stop => break,
                control::Next::Exit(flow) => return Ok(flow),
            },
            Role::Vision => {
                if let Some(flow) = vision::run_screen_block(ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
            Role::Browser => {
                if let Some(flow) = browser::run_browser_block(ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
            Role::Input => {
                if let Some(reason) = &ctx.input_unavailable {
                    if !ctx.input_unavailable_reported {
                        ctx.input_unavailable_reported = true;
                        let reason = reason.clone();
                        soft_fail(ctx, &ins.id, "Input", reason)?;
                    }
                } else if let InstructionKind::Token(token) = &ins.kind
                    && let Some(flow) = input::run_token(token, ins, ctx, depth)?
                {
                    return Ok(flow);
                } else if !matches!(ins.kind, InstructionKind::Token(_))
                    && let Err(error) = input::run_extended(ins, ctx, depth)
                {
                    ctx.gamepad = None;
                    soft_fail(ctx, &ins.id, "Input", error)?;
                }
            }
            Role::Data => {
                if let Some(flow) = data::run_data_block(ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
        }
    }

    Ok(Flow::Normal)
}

/// `make_backend`, or an `UnavailableBackend` when there is none, so macros
/// still run and their input blocks say why they can't.
pub fn make_backend_or_unavailable() -> Arc<Mutex<dyn InputBackend>> {
    make_backend()
        .unwrap_or_else(|| Arc::new(Mutex::new(crate::macros::backend::UnavailableBackend::new())))
}

pub fn make_backend() -> Option<Arc<Mutex<dyn InputBackend>>> {
    match create_backend() {
        Some(b) => Some(b),
        None => {
            EMULATOR_FAILED.store(true, Ordering::Relaxed);
            None
        }
    }
}

#[cfg(test)]
mod tests;
