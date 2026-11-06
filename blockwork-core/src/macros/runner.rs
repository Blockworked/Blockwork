use crate::input::types::{Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::{Evaluated, Op, Value};
use crate::macros::backend::{InputBackend, create_backend};
use crate::macros::host_clock;
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{
    BlockKind, ImageMatch, ImagePick, Instruction, InstructionKind, LaunchOptions, Macro, Strand,
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

/// What a `run_block` invocation is telling its caller to do next: the
/// "did a Return happen" signal, generalized to also carry loop control.
/// `If`/`IfElse` forward every non-`Normal` variant unchanged (not loops,
/// nothing to catch); `Repeat`/`Forever`/`While` catch `Break`/`Continue`
/// but let `Return` keep unwinding. `call_block` absorbs `Break`/`Continue`
/// at its own boundary - a custom block's body is a separate context, so
/// loop control can't cross into or out of it.
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
    stop_flag: Option<Arc<Mutex<bool>>>,
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

/// `true` once a stop has been requested on `flag` (the flag itself holds
/// "should keep running", so this is just its negation behind the lock).
fn stop_requested(flag: &Arc<Mutex<bool>>) -> bool {
    !flag.lock().map(|g| *g).unwrap_or(false)
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
            if let Some(flag) = &ctx.stop_flag
                && let Ok(mut running) = flag.lock()
            {
                *running = false;
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
mod data;
mod dialogs;
mod input;
mod system;
mod vision;
use apps::*;

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

impl Macro {
    /// A fresh list store holding the macro's saved lists.
    fn list_store(&self) -> ListStore {
        Arc::new(Mutex::new(
            self.lists
                .iter()
                .map(|list| (list.name.clone(), list.items.clone()))
                .collect(),
        ))
    }

    /// Builds the custom block table from this macro's `BlockHeader` strands.
    pub fn block_table(&self) -> BlockTable {
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

    /// Runs every "When Ran" entry-point strand concurrently on its own
    /// thread, sharing one `stop_flag`, from the macro's saved lists. A custom
    /// block's header strand isn't run directly - its body is extracted into
    /// `block_table` and only executes via `CallBlock`/`Value::Call`. Blocks
    /// until every strand finishes or is stopped. [`Macro::run_with`] takes
    /// the rest of the options.
    pub fn run(
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

    /// [`Macro::run`] with every option. Trigger strands are fired by the
    /// app's watchers via `run_fired_strand`, never by a manual Run.
    pub fn run_with(self, options: RunOptions) {
        self.run_entries(
            |s| {
                matches!(
                    s.instructions.first().map(|i| &i.kind),
                    Some(InstructionKind::WhenRan)
                )
            },
            options,
        )
    }

    /// Runs just the strand `strand_id`, from its top block, whatever event
    /// heads it. False when there's no such strand or it's a custom block's
    /// definition, which only runs when called.
    pub fn run_one_strand(self, strand_id: &str, options: RunOptions) -> bool {
        if !self.can_run_strand(strand_id) {
            return false;
        }
        self.run_entries(|s| s.id == strand_id, options);
        true
    }

    /// Whether `run_one_strand` would run `strand_id`.
    pub fn can_run_strand(&self, strand_id: &str) -> bool {
        self.graph.strand(strand_id).is_some_and(|s| {
            !s.instructions.is_empty()
                && !matches!(s.instructions[0].kind, InstructionKind::BlockHeader(_))
        })
    }

    /// Runs every strand `pick` accepts side by side, each on its own thread.
    fn run_entries(self, pick: impl Fn(&Strand) -> bool, options: RunOptions) {
        let RunOptions {
            emulator,
            stop_flag,
            speed_multiplier,
            variables,
            lists,
            offset: initial_offset,
        } = options;
        let lists = lists.unwrap_or_else(|| self.list_store());
        let source = Arc::new(crate::run_log::Source::for_macro(&self.id, &self.name));
        let max_loop_iterations = self.settings.max_loop_iterations;
        // StopMacro needs a flag every strand of this run shares.
        let stop_flag = stop_flag.or_else(|| Some(Arc::new(Mutex::new(true))));
        let block_table = self.block_table();
        let entry_strands: Vec<Vec<Instruction>> = self
            .graph
            .strands
            .into_iter()
            .filter(|s| pick(s))
            .map(|s| s.instructions)
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
            for instructions in entry_strands {
                let emulator = Arc::clone(&emulator);
                let stop_flag = stop_flag.clone();
                let variables = Arc::clone(&variables);
                let lists = Arc::clone(&lists);
                let block_table = Arc::clone(&block_table);
                let source = Arc::clone(&source);
                let spawned = std::thread::Builder::new()
                    .stack_size(STRAND_STACK_SIZE)
                    .spawn_scoped(scope, move || {
                        run_strand(
                            source,
                            instructions,
                            emulator,
                            stop_flag,
                            speed_multiplier,
                            variables,
                            lists,
                            block_table,
                            initial_offset,
                            max_loop_iterations,
                        )
                    });
                if let Err(e) = spawned {
                    warn!("Failed to start a strand thread: {e}");
                }
            }
        });
    }
}

/// Per-thread entry point for one strand: runs it via `run_block`, then
/// releases any keys/buttons still held, whether the strand finished,
/// stopped, or errored. The returned `Result`/`Option` is discarded - a
/// `Return` reaching strand level has nowhere to hand its value, and resolve
/// errors are already `warn!`-logged at their source.
/// Runs a function when dropped.
struct Defer(Option<fn()>);

impl Drop for Defer {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

fn run_strand(
    source: Arc<crate::run_log::Source>,
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    block_table: BlockTable,
    initial_offset: Duration,
    max_loop_iterations: u32,
) {
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
    let _permissions = crate::permissions::enter(Arc::clone(&source), stop_flag.clone());
    {
        let mut ctx = ExecCtx {
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
    run_strand(
        Arc::new(source),
        instructions,
        emulator,
        stop_flag.or_else(|| Some(Arc::new(Mutex::new(true)))),
        speed_multiplier,
        variables,
        lists,
        block_table,
        Duration::ZERO,
        max_loop_iterations,
    );
}

/// Runs `instructions` top to bottom, returning `Ok(Flow::Return(v))` the
/// instant a `Return` is hit, `Ok(Flow::Break)`/`Ok(Flow::Continue)` the
/// instant an `EscapeLoop`/`ContinueLoop` is hit, `Ok(Flow::Normal)` on
/// completion, or `Err` on a hard failure (call-depth overrun, or a
/// `Return`'s value failing to resolve). Most per-instruction resolve errors
/// are non-fatal and just `warn!`-log + skip.
///
/// `stop_flag`, when present, is checked between every instruction (and
/// during a `Wait`, `Repeat`, `Forever`, or `While`) so the run can be
/// aborted early.
///
/// `speed_multiplier` scales every `Wait` inversely (2.0 = half-length,
/// twice as fast), combining the macro's own multiplier with the global
/// runtime override.
fn run_block(
    instructions: &[Instruction],
    ctx: &mut ExecCtx,
    depth: u32,
    start: Instant,
) -> Result<Flow, String> {
    if depth > MAX_CALL_DEPTH {
        return Err("custom block call depth exceeded (possible infinite recursion)".to_string());
    }

    // Local to this invocation (not in `ctx`) so a nested call's waits pace
    // independently; the caller's deadline re-anchors on its next Wait.
    // `start` is `Instant::now()` at every call site except the outermost
    // one (`run_strand`'s top-level call), backdated by that strand's
    // `initial_offset` - see `RunOptions::offset`.
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

        match &ins.kind {
            InstructionKind::Comment(_) => {}
            InstructionKind::WhenRan => {}
            InstructionKind::BlockHeader(_) => {}
            // Header-only markers, same as `WhenRan`/`BlockHeader` - never
            // actually reached (`run_with` excludes these strands
            // from `entry_strands`, and the background watcher that fires
            // them starts from `strand.instructions[1..]`). Handled here
            // defensively so the match stays total.
            InstructionKind::WhenBatteryDischargedTo(_) => {}
            InstructionKind::WhenBatteryChargedTo(_) => {}
            InstructionKind::WhenTime(..) => {}
            InstructionKind::WhenPowerPluggedIn => {}
            InstructionKind::WhenPowerUnplugged => {}
            InstructionKind::WhenClipboardChanged => {}
            InstructionKind::WhenKey(..) => {}
            InstructionKind::WhenWindow { .. } => {}
            InstructionKind::WhenFileChanged(_) => {}
            InstructionKind::WhenTyped { .. } => {}
            InstructionKind::WhenApp { .. } => {}
            InstructionKind::WhenIdle(_) => {}
            InstructionKind::WhenScreenLock { .. } => {}
            InstructionKind::WhenCron(_) => {}
            InstructionKind::WhenNetwork { .. } => {}
            InstructionKind::WhenUsb { .. } => {}
            InstructionKind::WhenMouseButton { .. } => {}
            InstructionKind::WhenAt(_) => {}
            InstructionKind::WhenSignal(_) => {}
            InstructionKind::WhenWebhook { .. } => {}
            InstructionKind::AskText { .. }
            | InstructionKind::AskYesNo { .. }
            | InstructionKind::AskChoice { .. }
            | InstructionKind::ShowMessage { .. } => {
                if let Some(flow) = dialogs::run_dialog_block(ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
            InstructionKind::SendSignal { .. }
            | InstructionKind::WindowAction { .. }
            | InstructionKind::MoveWindow { .. }
            | InstructionKind::RunMacro { .. }
            | InstructionKind::StopMacroNamed(_)
            | InstructionKind::SetGlobal { .. }
            | InstructionKind::ChangeGlobal { .. }
            | InstructionKind::Log(_)
            | InstructionKind::Notify { .. }
            | InstructionKind::WriteFile { .. }
            | InstructionKind::HttpRequest { .. }
            | InstructionKind::Command(_)
            | InstructionKind::CommandWait(_) => system::run_system_block(ins, ctx, depth)?,
            InstructionKind::SplitIntoList {
                text,
                separator,
                name,
            } => {
                let parts = ctx
                    .resolve(text, depth)
                    .and_then(|v| v.eval_text())
                    .and_then(|text| {
                        Ok(crate::text::split(
                            &text,
                            &ctx.resolve(separator, depth)?.eval_text()?,
                        ))
                    });
                match parts {
                    Ok(parts) => {
                        if let Ok(mut lists) = ctx.lists.lock() {
                            lists.insert(
                                name.clone(),
                                parts.into_iter().map(ListItem::Text).collect(),
                            );
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Split", e)?,
                }
            }
            InstructionKind::PasteText(text) => {
                let pasted = ctx
                    .resolve(text, depth)
                    .and_then(|v| v.eval_text())
                    .and_then(|text| paste_text(ctx, text));
                if let Err(e) = pasted {
                    soft_fail(ctx, &ins.id, "Paste", e)?;
                }
            }
            InstructionKind::WithLock { name, body } => {
                let name = match ctx.resolve(name, depth).and_then(|v| v.eval_text()) {
                    Ok(name) => name,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Lock", e)?;
                        continue;
                    }
                };
                let stop = ctx.stop_flag.clone();
                let Some(_guard) =
                    crate::macros::locks::acquire(&name, Duration::from_millis(50), || {
                        stop.as_ref().is_some_and(stop_requested)
                    })
                else {
                    break;
                };
                let flow = run_block(body, ctx, depth, Instant::now())?;
                if flow != Flow::Normal {
                    return Ok(flow);
                }
            }
            InstructionKind::OpenApp {
                command, launch, ..
            } => {
                println!("Opening app: {command}");
                if let Err(e) = open_app(command, launch) {
                    soft_fail(ctx, &ins.id, "Open app", e)?;
                }
            }
            InstructionKind::CloseApp { command, name, .. } => {
                println!("Closing app: {name}");
                if let Err(e) = close_app(command, name) {
                    soft_fail(ctx, &ins.id, "Close app", e)?;
                }
            }
            InstructionKind::Return(value) => {
                let resolved = ctx.resolve(value, depth)?;
                return Ok(Flow::Return(resolved.eval()?));
            }
            InstructionKind::EscapeLoop => return Ok(Flow::Break),
            InstructionKind::ContinueLoop => return Ok(Flow::Continue),
            InstructionKind::StopMacro => {
                crate::run_log::info(&ctx.source, "Stopped by a stop macro block");
                if let Some(flag) = &ctx.stop_flag
                    && let Ok(mut running) = flag.lock()
                {
                    *running = false;
                }
                break;
            }
            InstructionKind::ThrowError(message) => {
                let message = ctx
                    .resolve(message, depth)
                    .and_then(|v| v.eval_text())
                    .unwrap_or_else(|e| e);
                ctx.error_block = ins.id.clone();
                return Err(if message.is_empty() {
                    "error".to_string()
                } else {
                    message
                });
            }
            InstructionKind::TryCatch { body, catch_body } => {
                ctx.try_depth += 1;
                let result = run_block(body, ctx, depth, Instant::now());
                ctx.try_depth -= 1;
                match result {
                    Ok(Flow::Normal) => {}
                    Ok(flow) => return Ok(flow),
                    Err(message) => {
                        crate::run_log::info(&ctx.source, format!("Handled error: {message}"));
                        ctx.error_block.clear();
                        LAST_ERROR.with(|e| *e.borrow_mut() = message);
                        let flow = run_block(catch_body, ctx, depth, Instant::now())?;
                        if flow != Flow::Normal {
                            return Ok(flow);
                        }
                    }
                }
            }
            InstructionKind::Retry { count, delay, body } => {
                let attempts = ctx.resolve(count, depth).and_then(|v| v.eval_number());
                let delay_ms = ctx.resolve(delay, depth).and_then(|v| v.eval_number());
                let (attempts, delay_ms) = match (attempts, delay_ms) {
                    (Ok(a), Ok(d)) => (a.max(1.0).round() as u64, d),
                    (Err(e), _) | (_, Err(e)) => {
                        soft_fail(ctx, &ins.id, "Retry", e)?;
                        continue;
                    }
                };
                for attempt in 1..=attempts {
                    ctx.try_depth += 1;
                    let result = run_block(body, ctx, depth, Instant::now());
                    ctx.try_depth -= 1;
                    let message = match result {
                        Ok(Flow::Normal) => break,
                        Ok(flow) => return Ok(flow),
                        Err(message) => message,
                    };
                    ctx.error_block.clear();
                    LAST_ERROR.with(|e| *e.borrow_mut() = message.clone());
                    if attempt == attempts {
                        soft_fail(
                            ctx,
                            &ins.id,
                            "Retry",
                            format!("gave up after {attempts} attempts: {message}"),
                        )?;
                        break;
                    }
                    crate::run_log::info(
                        &ctx.source,
                        format!("Attempt {attempt} of {attempts} failed: {message}"),
                    );
                    if sleep_unless_stopped(ctx, delay_ms) {
                        break;
                    }
                }
            }
            InstructionKind::WaitUntil {
                condition,
                timeout,
                error_timeout,
            } => {
                let (timeout, error_timeout) = match (|| {
                    Ok::<_, String>((
                        optional_number(ctx, depth, timeout.as_ref(), 0.0)?,
                        optional_number(ctx, depth, error_timeout.as_ref(), 0.0)?,
                    ))
                })() {
                    Ok(limits) => limits,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait until", e)?;
                        continue;
                    }
                };
                let started = Instant::now();
                loop {
                    match ctx
                        .resolve(condition, depth)
                        .and_then(|v| v.eval())
                        .map(|v| v.as_bool())
                    {
                        Ok(true) => break,
                        Ok(false) => {}
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Wait until", e)?;
                            break;
                        }
                    }
                    if wait_limit_reached(
                        ctx,
                        &ins.id,
                        started,
                        timeout,
                        error_timeout,
                        "Wait until: still false",
                    )? {
                        break;
                    }
                    if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                        break;
                    }
                }
            }
            InstructionKind::FindImage { .. }
            | InstructionKind::ClickImage { .. }
            | InstructionKind::WaitForImage { .. }
            | InstructionKind::DetectObjectsInWindow { .. }
            | InstructionKind::WaitForDetection { .. }
            | InstructionKind::AimStep { .. }
            | InstructionKind::AimAtDetection { .. } => {
                if let Some(flow) = vision::run_screen_block(ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
            InstructionKind::BrowserOpen { .. }
            | InstructionKind::BrowserClick { .. }
            | InstructionKind::BrowserType { .. }
            | InstructionKind::BrowserWaitFor { .. }
            | InstructionKind::BrowserScript { .. } => {
                if let Some(flow) = browser::run_browser_block(ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
            InstructionKind::CallBlock {
                block_id,
                args,
                branches,
            } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    match ctx.resolve(a, depth).and_then(|v| v.eval()) {
                        Ok(v) => evaluated_args.push(v),
                        Err(e) => {
                            soft_fail(ctx, &ins.id, &format!("Call to '{block_id}'"), e)?;
                            continue;
                        }
                    }
                }
                // A non-reporter (`Normal`/`Ending`) block's body normally
                // has no `Return`; if one sneaks in, it just ends the call
                // early.
                let _ = call_block(block_id, evaluated_args, branches.clone(), ctx, depth + 1)?;
            }
            InstructionKind::RunBranch(name) => {
                if let Some(body) = ctx.branch_env.get(name).cloned() {
                    let flow = run_block(&body, ctx, depth, Instant::now())?;
                    if flow != Flow::Normal {
                        return Ok(flow);
                    }
                }
            }
            InstructionKind::If { condition, body } => {
                match ctx.resolve(condition, depth).and_then(|v| v.eval()) {
                    Ok(cond) => {
                        if cond.as_bool() {
                            let flow = run_block(body, ctx, depth, Instant::now())?;
                            if flow != Flow::Normal {
                                return Ok(flow);
                            }
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "If", e)?,
                }
            }
            InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            } => match ctx.resolve(condition, depth).and_then(|v| v.eval()) {
                Ok(cond) => {
                    let branch = if cond.as_bool() { then_body } else { else_body };
                    let flow = run_block(branch, ctx, depth, Instant::now())?;
                    if flow != Flow::Normal {
                        return Ok(flow);
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "If else", e)?,
            },
            InstructionKind::Repeat { count, body } => {
                let n = match ctx.resolve(count, depth).and_then(|v| v.eval_number()) {
                    Ok(v) => v,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Repeat", e)?;
                        continue;
                    }
                };
                let n = n.max(0.0).round() as u64;
                let mut iterations = 0;
                for _ in 0..n {
                    if let Some(flag) = &ctx.stop_flag
                        && stop_requested(flag)
                    {
                        break;
                    }
                    if loop_limit_hit(ctx, &ins.id, &mut iterations)? {
                        break;
                    }
                    match run_block(body, ctx, depth, Instant::now())? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
            }
            InstructionKind::Forever { body } => {
                let mut iterations = 0;
                loop {
                    if let Some(flag) = &ctx.stop_flag
                        && stop_requested(flag)
                    {
                        break;
                    }
                    if loop_limit_hit(ctx, &ins.id, &mut iterations)? {
                        break;
                    }
                    match run_block(body, ctx, depth, Instant::now())? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
            }
            InstructionKind::While { condition, body } => {
                let mut iterations = 0;
                loop {
                    if let Some(flag) = &ctx.stop_flag
                        && stop_requested(flag)
                    {
                        break;
                    }
                    match ctx.resolve(condition, depth).and_then(|v| v.eval()) {
                        Ok(cond) => {
                            if !cond.as_bool() {
                                break;
                            }
                        }
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "While", e)?;
                            break;
                        }
                    }
                    if loop_limit_hit(ctx, &ins.id, &mut iterations)? {
                        break;
                    }
                    match run_block(body, ctx, depth, Instant::now())? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
            }
            InstructionKind::Wait(duration) => {
                let duration = match ctx.resolve(duration, depth).and_then(|v| v.eval_number()) {
                    Ok(v) => v,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait", e)?;
                        continue;
                    }
                };
                // Clamped since duration may be an Op::Random node with a
                // negative lower bound; from_secs_f64 panics on negative input.
                let actual = (duration / ctx.speed_multiplier).max(0.0);
                if host_clock::enabled() {
                    ctx.host_time_ms += actual;
                    let flag = ctx.stop_flag.clone();
                    let should_stop = move || flag.as_ref().is_some_and(stop_requested);
                    if !host_clock::wait_until(ctx.host_time_ms, &should_stop) {
                        break;
                    }
                    continue;
                }
                deadline += Duration::from_secs_f64(actual / 1000.0);

                if let Some(flag) = &ctx.stop_flag {
                    let mut stopped = false;
                    loop {
                        if stop_requested(flag) {
                            stopped = true;
                            break;
                        }
                        let now = Instant::now();
                        if now >= deadline {
                            break;
                        }
                        let remaining = deadline - now;
                        if remaining > STOP_POLL_INTERVAL {
                            spin_sleeper().sleep(STOP_POLL_INTERVAL);
                        } else {
                            spin_sleeper().sleep_until(deadline);
                            break;
                        }
                    }
                    if stopped {
                        break;
                    }
                } else {
                    let now = Instant::now();
                    if now >= deadline {
                        deadline = now; // fell behind; re-anchor instead of catching up
                    } else {
                        spin_sleeper().sleep_until(deadline);
                    }
                }
            }
            InstructionKind::Token(_) if ctx.input_unavailable.is_some() => {
                if !ctx.input_unavailable_reported {
                    ctx.input_unavailable_reported = true;
                    let reason = ctx.input_unavailable.clone().unwrap_or_default();
                    soft_fail(ctx, &ins.id, "Input", reason)?;
                }
            }
            InstructionKind::Token(token) => {
                if let Some(flow) = input::run_token(token, ins, ctx, depth)? {
                    return Ok(flow);
                }
            }
            InstructionKind::SetVariable { .. }
            | InstructionKind::ChangeVariable { .. }
            | InstructionKind::SetClipboard { .. }
            | InstructionKind::AddToList { .. }
            | InstructionKind::DeleteOfList { .. }
            | InstructionKind::DeleteAllOfList { .. }
            | InstructionKind::ShiftList { .. }
            | InstructionKind::InsertIntoList { .. }
            | InstructionKind::ReplaceItemOfList { .. }
            | InstructionKind::ReverseList { .. } => {
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
