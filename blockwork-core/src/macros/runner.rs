use crate::input::types::{Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::{Evaluated, Op, Value};
use crate::macros::backend::{create_backend, InputBackend};
use crate::macros::host_clock;
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{BlockKind, ImageMatch, ImagePick, Instruction, InstructionKind, LaunchOptions, Macro, Strand};
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
        Value::Call { block_id, args, branches, .. } => {
            // Args evaluate in the caller's scope, fully resolved to
            // concrete values before call_block swaps in the callee's scope.
            let mut evaluated_args = Vec::with_capacity(args.len());
            for a in args {
                let resolved = resolve_calls_and_params(a, ctx, depth)?;
                evaluated_args.push(resolved.eval()?);
            }
            let branch_values = branches.iter().map(|body| {
                crate::frontend::reporter_branch_to_instructions(body)
            }).collect::<Result<Vec<_>, _>>()?;
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
    if !runtime.variable_defaults.is_empty() && let Ok(mut vars) = ctx.variables.lock() {
        for (key, value) in &runtime.variable_defaults { vars.entry(key.clone()).or_insert_with(|| value.clone()); }
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
fn soft_fail(ctx: &mut ExecCtx, block_id: &str, block: &str, error: impl std::fmt::Display) -> Result<(), String> {
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
    if error_timeout > 0.0 && (timeout <= 0.0 || error_timeout <= timeout) && elapsed >= error_timeout {
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
    soft_fail(ctx, block_id, "Loop", format!("stopped after {limit} iterations (this macro's loop limit)"))?;
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
        let modifier = if cfg!(target_os = "macos") { MacroKey::Meta } else { MacroKey::LControl };
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
            let Some(InstructionKind::WhenSignal(listens)) = body.first().map(|i| &i.kind) else { return None };
            let listens = ctx.resolve(&listens.value, depth).and_then(|v| v.eval_text()).ok()?;
            listens.trim().eq_ignore_ascii_case(name.trim()).then(|| body[1..].to_vec())
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
fn ask(ctx: &mut ExecCtx, block_id: &str, block: &str, request: crate::prompt::Request) -> Result<Option<crate::prompt::Reply>, String> {
    let flag = ctx.stop_flag.clone();
    let stopped = || flag.as_ref().is_some_and(stop_requested);
    match crate::prompt::ask(&request, &stopped) {
        Ok(reply) if reply.answer == "cancel" => {
            if ctx.try_depth > 0 {
                ctx.error_block = block_id.to_string();
                return Err(format!("{block}: cancelled"));
            }
            crate::run_log::info(&ctx.source, format!("{block}: cancelled, so the run stopped"));
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

mod vision;
mod apps;
use vision::*;
use apps::*;

impl Macro {
    /// Builds the custom block table from this macro's `BlockHeader` strands.
    pub fn block_table(&self) -> BlockTable {
        let mut table = HashMap::new();
        // Signal strands ride along under reserved keys, header included,
        // so every run entry point can reach them without another argument.
        for (index, strand) in self.graph.strands.iter().enumerate() {
            if matches!(strand.instructions.first().map(|i| &i.kind), Some(InstructionKind::WhenSignal(_))) {
                let runtime = BlockRuntime { input_names: Vec::new(), branch_names: Vec::new(), body: strand.instructions.clone(), variable_defaults: Vec::new() };
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
            let defaults: Vec<_> = pack.variables.iter().map(|v| (pack.variable_key(&v.name), v.value.clone())).collect();
            for id in &pack.blocks {
                if crate::block_packs::owner(self, id).is_some_and(|owner| owner.id != pack.id) { continue; }
                if let Some(runtime) = table.get_mut(id) {
                    crate::block_packs::scope_body(&mut runtime.body, pack);
                    runtime.variable_defaults = defaults.clone();
                }
            }
            for var in pack.variables.iter().filter(|v| v.role == crate::block_packs::VariableRole::Output) {
                table.insert(pack.output_id(&var.name), BlockRuntime {
                    input_names: Vec::new(), branch_names: Vec::new(),
                    body: vec![Instruction::new(InstructionKind::Return(Value::Var { name: pack.variable_key(&var.name) }))],
                    variable_defaults: defaults.clone(),
                });
            }
        }
        Arc::new(table)
    }

    /// Runs every "When Ran" entry-point strand concurrently on its own
    /// thread, sharing one `stop_flag`. A custom block's header strand isn't
    /// run directly - its body is extracted into `block_table` and only
    /// executes via `CallBlock`/`Value::Call`. Blocks until every strand
    /// finishes or is stopped.
    pub fn run(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
    ) {
        let lists: ListStore = Arc::new(Mutex::new(
            self.lists
                .iter()
                .map(|list| (list.name.clone(), list.items.clone()))
                .collect(),
        ));
        self.run_with_lists(emulator, stop_flag, speed_multiplier, variables, lists);
    }

    /// `run` with an externally-owned list store, used by the desktop host to
    /// persist list mutations after a macro finishes.
    pub fn run_with_lists(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
        lists: ListStore,
    ) {
        self.run_with_lists_offset(
            emulator,
            stop_flag,
            speed_multiplier,
            variables,
            lists,
            Duration::ZERO,
        )
    }

    /// Backward-compatible offset variant for callers that do not need to
    /// retain a live list store after the run.
    pub fn run_with_offset(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
        initial_offset: Duration,
    ) {
        let lists: ListStore = Arc::new(Mutex::new(
            self.lists
                .iter()
                .map(|list| (list.name.clone(), list.items.clone()))
                .collect(),
        ));
        self.run_with_lists_offset(
            emulator,
            stop_flag,
            speed_multiplier,
            variables,
            lists,
            initial_offset,
        )
    }

    /// Same as `run_with_offset`, but with an externally-owned list store,
    /// used by the desktop host to persist list mutations after a macro
    /// finishes. Backdates every entry strand's `Wait` deadline anchor by
    /// `initial_offset` - pretends this run started `initial_offset` ago
    /// rather than right now.
    pub fn run_with_lists_offset(
        self,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
        lists: ListStore,
        initial_offset: Duration,
    ) {
        // Trigger strands are fired by the app's watchers via
        // `run_fired_strand`, never by a manual Run.
        self.run_entries(
            |s| matches!(s.instructions.first().map(|i| &i.kind), Some(InstructionKind::WhenRan)),
            emulator,
            stop_flag,
            speed_multiplier,
            variables,
            lists,
            initial_offset,
        )
    }

    /// Runs just the strand `strand_id`, from its top block, whatever event
    /// heads it. False when there's no such strand or it's a custom block's
    /// definition, which only runs when called.
    pub fn run_strand_with_lists(
        self,
        strand_id: &str,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
        lists: ListStore,
    ) -> bool {
        if !self.can_run_strand(strand_id) {
            return false;
        }
        self.run_entries(|s| s.id == strand_id, emulator, stop_flag, speed_multiplier, variables, lists, Duration::ZERO);
        true
    }

    /// Whether `run_strand_with_lists` would run `strand_id`.
    pub fn can_run_strand(&self, strand_id: &str) -> bool {
        self.graph.strand(strand_id).is_some_and(|s| {
            !s.instructions.is_empty() && !matches!(s.instructions[0].kind, InstructionKind::BlockHeader(_))
        })
    }

    /// Runs every strand `pick` accepts side by side, each on its own thread.
    fn run_entries(
        self,
        pick: impl Fn(&Strand) -> bool,
        emulator: Arc<Mutex<dyn InputBackend>>,
        stop_flag: Option<Arc<Mutex<bool>>>,
        speed_multiplier: f64,
        variables: VariableStore,
        lists: ListStore,
        initial_offset: Duration,
    ) {
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

/// Convenience entry point equivalent to `run_strand` with an empty
/// `block_table`, for callers/tests that don't need custom blocks.
pub fn run_instructions(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
) {
    run_instructions_with_lists(
        instructions,
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        Arc::new(Mutex::new(HashMap::new())),
    );
}

/// `run_instructions` with an externally-owned list store for watcher runs.
pub fn run_instructions_with_lists(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
) {
    run_fired_strand(
        crate::run_log::Source::default(),
        instructions,
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        lists,
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
    // `initial_offset` - see `Macro::run_with_offset`.
    let mut deadline = start;

    let normalize_modifier_key = |key: MacroKey| -> MacroKey {
        match key {
            MacroKey::Shift => MacroKey::LShift,
            _ => key,
        }
    };

    let shift_is_pressed = |keys: &[MacroKey]| -> bool {
        keys.iter()
            .any(|k| matches!(k, MacroKey::Shift | MacroKey::LShift | MacroKey::RShift))
    };

    for ins in instructions {
        if let Some(flag) = &ctx.stop_flag {
            if stop_requested(flag) {
                break;
            }
        }
        if let Some(trace) = &ctx.trace
            && !ins.kind.is_header()
        {
            trace.at(&ins.id);
        }
        if !ins.kind.is_header() && !matches!(ins.kind, InstructionKind::Comment(_)) {
            let stopped = || ctx.stop_flag.as_ref().is_some_and(stop_requested);
            let paused = crate::macros::debug::checkpoint(&ctx.source.macro_id, &ins.id, &stopped, &ctx.variables, &ctx.lists, &ctx.param_env);
            if !paused.is_zero() {
                // Waits after the pause keep their spacing instead of catching up.
                deadline += paused;
                if stopped() {
                    break;
                }
            }
        }

        if let Some(Err(e)) = crate::capabilities::checked_when_run(&ins.kind).map(crate::permissions::require).find(Result::is_err) {
            soft_fail(ctx, &ins.id, "Permission", e)?;
            continue;
        }

        match &ins.kind {
            InstructionKind::Comment(_) => {}
            InstructionKind::WhenRan => {}
            InstructionKind::BlockHeader(_) => {}
            // Header-only markers, same as `WhenRan`/`BlockHeader` - never
            // actually reached (`run_with_offset` excludes these strands
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
            InstructionKind::AskText { prompt, default, name } => {
                let request = (|| {
                    let message = ctx.resolve(prompt, depth)?.eval_text()?;
                    let mut request = crate::prompt::Request::new(crate::prompt::Kind::Text, &ctx.source.name, message);
                    request.default = ctx.resolve(default, depth)?.eval_text()?;
                    Ok::<_, String>(request)
                })();
                match request {
                    Ok(request) => {
                        if let Some(reply) = ask(ctx, &ins.id, "Ask", request)? {
                            set_variable(ctx, name, Evaluated::Text(reply.text));
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Ask", e)?,
                }
            }
            InstructionKind::AskYesNo { prompt, name } => match ctx.resolve(prompt, depth).and_then(|v| v.eval_text()) {
                Ok(message) => {
                    let request = crate::prompt::Request::new(crate::prompt::Kind::Confirm, &ctx.source.name, message);
                    if let Some(reply) = ask(ctx, &ins.id, "Ask yes or no", request)? {
                        set_variable(ctx, name, Evaluated::Bool(reply.answer == "yes"));
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Ask yes or no", e)?,
            },
            InstructionKind::AskChoice { prompt, list, name } => {
                let options: Vec<String> = ctx
                    .lists
                    .lock()
                    .ok()
                    .and_then(|lists| lists.get(list).map(|items| items.iter().map(|i| i.evaluated().as_text()).collect()))
                    .unwrap_or_default();
                let message = ctx.resolve(prompt, depth).and_then(|v| v.eval_text());
                match message {
                    Ok(_) if options.is_empty() => soft_fail(ctx, &ins.id, "Choose", format!("the list \"{list}\" is empty"))?,
                    Ok(message) => {
                        let mut request = crate::prompt::Request::new(crate::prompt::Kind::Choice, &ctx.source.name, message);
                        request.options = options;
                        if let Some(reply) = ask(ctx, &ins.id, "Choose", request)? {
                            set_variable(ctx, name, Evaluated::Text(reply.text));
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Choose", e)?,
                }
            }
            InstructionKind::ShowMessage(message) => match ctx.resolve(message, depth).and_then(|v| v.eval_text()) {
                Ok(message) => {
                    let request = crate::prompt::Request::new(crate::prompt::Kind::Message, &ctx.source.name, message);
                    // Closing a message is the same as OK.
                    let flag = ctx.stop_flag.clone();
                    let stopped = || flag.as_ref().is_some_and(stop_requested);
                    if let Err(e) = crate::prompt::ask(&request, &stopped)
                        && !stopped()
                    {
                        soft_fail(ctx, &ins.id, "Show message", e)?;
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Show message", e)?,
            },
            InstructionKind::SendSignal { name, wait } => {
                let name = match ctx.resolve(&name.value, depth).and_then(|v| v.eval_text()) {
                    Ok(name) => name,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Send signal", e)?;
                        continue;
                    }
                };
                let mut threads = Vec::new();
                for body in signal_bodies(ctx, &name, depth) {
                    let (source, emulator, stop_flag) = (Arc::clone(&ctx.source), Arc::clone(&ctx.emulator), ctx.stop_flag.clone());
                    let (variables, lists, table) = (Arc::clone(&ctx.variables), Arc::clone(&ctx.lists), Arc::clone(&ctx.block_table));
                    let (speed, loops) = (ctx.speed_multiplier, u32::try_from(ctx.max_loop_iterations).unwrap_or(u32::MAX));
                    let spawned = std::thread::Builder::new().name(format!("signal_{name}")).stack_size(STRAND_STACK_SIZE).spawn(move || {
                        run_strand(source, body, emulator, stop_flag, speed, variables, lists, table, Duration::ZERO, loops)
                    });
                    match spawned {
                        Ok(thread) => threads.push(thread),
                        Err(e) => warn!("Failed to start a signal strand: {e}"),
                    }
                }
                if *wait {
                    for thread in threads {
                        let _ = thread.join();
                    }
                } else {
                    ctx.signal_threads.extend(threads);
                }
            }
            InstructionKind::SplitIntoList { text, separator, name } => {
                let parts = ctx
                    .resolve(text, depth)
                    .and_then(|v| v.eval_text())
                    .and_then(|text| Ok(crate::text::split(&text, &ctx.resolve(separator, depth)?.eval_text()?)));
                match parts {
                    Ok(parts) => {
                        if let Ok(mut lists) = ctx.lists.lock() {
                            lists.insert(name.clone(), parts.into_iter().map(ListItem::Text).collect());
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Split", e)?,
                }
            }
            InstructionKind::PasteText(text) => {
                let pasted = ctx.resolve(text, depth).and_then(|v| v.eval_text()).and_then(|text| paste_text(ctx, text));
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
                let Some(_guard) = crate::macros::locks::acquire(&name, Duration::from_millis(50), || stop.as_ref().is_some_and(stop_requested)) else {
                    break;
                };
                let flow = run_block(body, ctx, depth, Instant::now())?;
                if flow != Flow::Normal {
                    return Ok(flow);
                }
            }
            InstructionKind::OpenApp { command, launch, .. } => {
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
                return Err(if message.is_empty() { "error".to_string() } else { message });
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
                        soft_fail(ctx, &ins.id, "Retry", format!("gave up after {attempts} attempts: {message}"))?;
                        break;
                    }
                    crate::run_log::info(&ctx.source, format!("Attempt {attempt} of {attempts} failed: {message}"));
                    if sleep_unless_stopped(ctx, delay_ms) {
                        break;
                    }
                }
            }
            InstructionKind::WindowAction { action, pattern, by } => {
                let done = (|| {
                    let pattern = ctx.resolve(pattern, depth)?.eval_text()?;
                    crate::window::act(action.parse()?, &pattern, by.parse()?)
                })();
                if let Err(e) = done {
                    soft_fail(ctx, &ins.id, "Window", e)?;
                }
            }
            InstructionKind::MoveWindow { pattern, by, resize, x, y } => {
                let done = (|| {
                    let pattern = ctx.resolve(pattern, depth)?.eval_text()?;
                    let x = ctx.resolve(x, depth)?.eval_number()?.round() as i32;
                    let y = ctx.resolve(y, depth)?.eval_number()?.round() as i32;
                    crate::window::place(&pattern, by.parse()?, *resize, x, y)
                })();
                if let Err(e) = done {
                    soft_fail(ctx, &ins.id, if *resize { "Resize window" } else { "Move window" }, e)?;
                }
            }
            InstructionKind::WaitUntil { condition, timeout, error_timeout } => {
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
                    match ctx.resolve(condition, depth).and_then(|v| v.eval()).map(|v| v.as_bool()) {
                        Ok(true) => break,
                        Ok(false) => {}
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Wait until", e)?;
                            break;
                        }
                    }
                    if wait_limit_reached(ctx, &ins.id, started, timeout, error_timeout, "Wait until: still false")? {
                        break;
                    }
                    if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                        break;
                    }
                }
            }
            InstructionKind::FindImage { image, tolerance, x_variable, y_variable, pick, region, count_variable } => {
                let search = ImageSearch { image, tolerance: tolerance.as_ref(), pick: pick.as_ref(), region: region.as_deref() };
                let count_variable = count_variable.as_deref().unwrap_or("");
                match find_picked_image(ctx, depth, &search, !count_variable.is_empty()) {
                    Ok((at, count)) => {
                        let (x, y) = at.map_or((-1.0, -1.0), |(x, y)| (x.round(), y.round()));
                        set_variable(ctx, x_variable, Evaluated::Number(x));
                        set_variable(ctx, y_variable, Evaluated::Number(y));
                        set_variable(ctx, count_variable, Evaluated::Number(count as f64));
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Find image", e)?,
                }
            }
            InstructionKind::ClickImage { image, tolerance, button, pick, region } => {
                let clicked = (|| {
                    let search = ImageSearch { image, tolerance: tolerance.as_ref(), pick: pick.as_ref(), region: region.as_deref() };
                    let mode = pick.as_ref().map(|m| m.mode);
                    let (at, count) = find_picked_image(ctx, depth, &search, mode == Some(ImagePick::Nth))?;
                    let (x, y) = at.ok_or_else(|| match (mode, count) {
                        (_, 0) => "the image isn't on screen".to_string(),
                        (Some(ImagePick::Nth), 1) => "the image is on screen only once".to_string(),
                        (Some(ImagePick::Nth), n) => format!("the image is on screen only {n} times"),
                        _ => "the image isn't on screen".to_string(),
                    })?;
                    let mut em = ctx.emulator.lock().map_err(|e| e.to_string())?;
                    em.click_mouse_abs(x.round() as i32, y.round() as i32, button.clone())
                })();
                if let Err(e) = clicked {
                    soft_fail(ctx, &ins.id, "Click image", e)?;
                }
            }
            InstructionKind::WaitForImage { image, tolerance, appear, timeout, region, error_timeout } => {
                let setup = (|| {
                    let tolerance = optional_number(ctx, depth, tolerance.as_ref(), 10.0)?;
                    let timeout = optional_number(ctx, depth, timeout.as_ref(), 0.0)?;
                    let error_timeout = optional_number(ctx, depth, error_timeout.as_ref(), 0.0)?;
                    let region = crate::detect::parse_region(region.as_deref().unwrap_or(""))?;
                    Ok::<_, String>((crate::screen::image::load_cached(image)?, tolerance, timeout, region, error_timeout))
                })();
                let (needle, tolerance, timeout, region, error_timeout) = match setup {
                    Ok(setup) => setup,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait for image", e)?;
                        return Ok(Flow::Normal);
                    }
                };
                let started = Instant::now();
                loop {
                    match crate::screen::find_image_in(&needle, tolerance, region) {
                        Ok(at) if at.is_some() == *appear => break,
                        Ok(_) => {}
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Wait for image", e)?;
                            break;
                        }
                    }
                    let what = if *appear { "Wait for image: the image didn't appear" } else { "Wait for image: the image didn't go away" };
                    if wait_limit_reached(ctx, &ins.id, started, timeout, error_timeout, what)? {
                        break;
                    }
                    if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                        break;
                    }
                }
            }
            InstructionKind::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => {
                let found = detect_query(ctx, depth, pattern, by, detector, label, confidence, sort, region)
                    .and_then(|query| detect_into_list(ctx, &query, results_list, &crate::detect::Screen));
                match found {
                    Ok(frame) => set_variable(ctx, count_variable, Evaluated::Number(frame.detections.len() as f64)),
                    Err(e) => soft_fail(ctx, &ins.id, "Detect objects", e)?,
                }
            }
            InstructionKind::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region } => {
                let setup = detect_query(ctx, depth, pattern, by, detector, label, confidence, "Score", region)
                    .and_then(|query| Ok((query, ctx.resolve(timeout, depth)?.eval_number()?.max(0.0))));
                let (query, timeout) = match setup {
                    Ok(setup) => setup,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait for detection", e)?;
                        continue;
                    }
                };
                let started = Instant::now();
                loop {
                    match crate::detect::detect_on_screen(&query) {
                        Ok(frame) if frame.detections.is_empty() != *appear => break,
                        Ok(_) => {}
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Wait for detection", e)?;
                            break;
                        }
                    }
                    if timeout > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout {
                        let what = if *appear { "didn't appear" } else { "didn't go away" };
                        soft_fail(ctx, &ins.id, "Wait for detection", format!("it {what} within {timeout} ms"))?;
                        break;
                    }
                    if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                        break;
                    }
                }
            }
            InstructionKind::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone } => {
                if !confirm_aim(ctx, &ins.id)? {
                    continue;
                }
                let done = (|| {
                    let pattern = ctx.resolve(pattern, depth)?.eval_text()?;
                    let target = (ctx.resolve(target_x, depth)?.eval_number()?, ctx.resolve(target_y, depth)?.eval_number()?);
                    let aim = aim_controller(ctx, depth, gain, max_step, deadzone)?;
                    let window = crate::window::bounds(&pattern, by.parse()?)?;
                    let crosshair = (window.x + window.width / 2.0, window.y + window.height / 2.0);
                    if let Some((dx, dy)) = aim.for_window(window.height).step(target, crosshair) {
                        ctx.emulator.lock().map_err(|e| e.to_string())?.move_mouse_rel(dx, dy)?;
                    }
                    Ok::<_, String>(())
                })();
                if let Err(e) = done {
                    soft_fail(ctx, &ins.id, "Aim", e)?;
                }
            }
            InstructionKind::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => {
                if !confirm_aim(ctx, &ins.id)? {
                    continue;
                }
                let setup = (|| {
                    let query = crate::detect::track::query_for(&ctx.source.macro_id, results_list)
                        .ok_or_else(|| format!("detect objects into the list \"{results_list}\" first"))?;
                    let n = ctx.resolve(index, depth)?.eval_number()?;
                    let item = ctx
                        .lists
                        .lock()
                        .ok()
                        .and_then(|lists| Some(lists.get(results_list)?.get((n.round() as usize).wrapping_sub(1))?.evaluated().as_text()))
                        .ok_or_else(|| format!("the list \"{results_list}\" has no item {n}"))?;
                    let target = crate::detect::Detection::from_item(&item)?
                        .id
                        .ok_or("that item has no tracking id; fill the list with a detect block")?;
                    let aim = aim_controller(ctx, depth, gain, max_step, deadzone)?;
                    let timeout = ctx.resolve(timeout, depth)?.eval_number()?.max(0.0);
                    let settle = ctx.resolve(settle_ms, depth)?.eval_number()?.max(0.0);
                    Ok::<_, String>((query, target, aim, timeout, settle))
                })();
                let result = setup.and_then(|(query, target, aim, timeout, settle)| aim_at(ctx, &query, results_list, target, aim, timeout, settle));
                match result {
                    Ok(status) => set_variable(ctx, status_variable, Evaluated::Text(status.to_string())),
                    Err(e) => soft_fail(ctx, &ins.id, "Aim at detection", e)?,
                }
            }
            InstructionKind::RunMacro { target, wait } => {
                match ctx.resolve(target, depth).and_then(|v| v.eval_text()).and_then(|name| crate::macros::calls::start(&name)) {
                    Ok((id, done)) if *wait => {
                        // Stopping this run stops the one it's waiting on too.
                        while !done.load(Ordering::Relaxed) {
                            if ctx.stop_flag.as_ref().is_some_and(stop_requested) {
                                crate::macros::run_registry::stop_macro(&id);
                                break;
                            }
                            std::thread::sleep(STOP_POLL_INTERVAL);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => soft_fail(ctx, &ins.id, "Run macro", e)?,
                }
            }
            InstructionKind::StopMacroNamed(target) => {
                if let Err(e) = ctx.resolve(target, depth).and_then(|v| v.eval_text()).and_then(|name| crate::macros::calls::stop(&name)) {
                    soft_fail(ctx, &ins.id, "Stop macro", e)?;
                }
            }
            InstructionKind::SetGlobal { name, value } => {
                let set = ctx.resolve(name, depth).and_then(|n| n.eval_text()).and_then(|name| {
                    crate::globals::set(&name, ctx.resolve(value, depth)?.eval()?)
                });
                if let Err(e) = set {
                    soft_fail(ctx, &ins.id, "Set global", e)?;
                }
            }
            InstructionKind::ChangeGlobal { name, by } => {
                let changed = ctx.resolve(name, depth).and_then(|n| n.eval_text()).and_then(|name| {
                    crate::globals::change(&name, ctx.resolve(by, depth)?.eval_number()?)
                });
                if let Err(e) = changed {
                    soft_fail(ctx, &ins.id, "Change global", e)?;
                }
            }
            InstructionKind::Log(value) => match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                Ok(text) => crate::run_log::info(&ctx.source, text),
                Err(e) => soft_fail(ctx, &ins.id, "Log", e)?,
            },
            InstructionKind::Notify { title, message } => {
                let title = ctx.resolve(title, depth).and_then(|v| v.eval_text());
                let message = ctx.resolve(message, depth).and_then(|v| v.eval_text());
                let shown = title.and_then(|t| message.and_then(|m| crate::notify::show(&t, &m)));
                if let Err(e) = shown {
                    soft_fail(ctx, &ins.id, "Notify", e)?;
                }
            }
            InstructionKind::WriteFile { path, text, append } => {
                let path = ctx.resolve(path, depth).and_then(|v| v.eval_text());
                let text = ctx.resolve(text, depth).and_then(|v| v.eval_text());
                let written = path.and_then(|p| text.and_then(|t| crate::files::write_text(&p, &t, *append)));
                if let Err(e) = written {
                    soft_fail(ctx, &ins.id, if *append { "Append to file" } else { "Write file" }, e)?;
                }
            }
            InstructionKind::BrowserOpen { url, new_tab } => {
                let opened = ctx
                    .resolve(url, depth)
                    .and_then(|u| u.eval_text())
                    .and_then(|url| crate::browser::request("open", serde_json::json!({ "url": url, "newTab": new_tab }), BROWSER_LOAD_LIMIT));
                if let Err(e) = opened {
                    soft_fail(ctx, &ins.id, "Open in browser", e)?;
                }
            }
            InstructionKind::BrowserClick { selector, nth } => {
                let clicked = (|| {
                    let selector = ctx.resolve(selector, depth)?.eval_text()?;
                    let nth = match nth {
                        Some(nth) => ctx.resolve(nth, depth)?.eval_number()?,
                        None => 1.0,
                    };
                    crate::browser::request("click", serde_json::json!({ "selector": selector, "nth": nth }), BROWSER_LIMIT)
                })();
                if let Err(e) = clicked {
                    soft_fail(ctx, &ins.id, "Click in browser", e)?;
                }
            }
            InstructionKind::BrowserType { text, selector, nth } => {
                let typed = (|| {
                    let text = ctx.resolve(text, depth)?.eval_text()?;
                    let selector = ctx.resolve(selector, depth)?.eval_text()?;
                    let nth = match nth {
                        Some(nth) => ctx.resolve(nth, depth)?.eval_number()?,
                        None => 1.0,
                    };
                    crate::browser::request("type", serde_json::json!({ "selector": selector, "nth": nth, "text": text }), BROWSER_LIMIT)
                })();
                if let Err(e) = typed {
                    soft_fail(ctx, &ins.id, "Type in browser", e)?;
                }
            }
            InstructionKind::BrowserWaitFor { selector, appear, timeout } => {
                let setup = (|| {
                    let selector = ctx.resolve(selector, depth)?.eval_text()?;
                    let timeout = ctx.resolve(timeout, depth)?.eval_number()?.max(0.0);
                    Ok::<_, String>((selector, timeout))
                })();
                let (selector, timeout) = match setup {
                    Ok(setup) => setup,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait for element", e)?;
                        return Ok(Flow::Normal);
                    }
                };
                let started = Instant::now();
                loop {
                    match crate::browser::request("count", serde_json::json!({ "selector": selector }), BROWSER_LIMIT) {
                        Ok(found) if (found["count"].as_u64().unwrap_or(0) > 0) == *appear => break,
                        Ok(_) => {}
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Wait for element", e)?;
                            break;
                        }
                    }
                    if timeout > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout {
                        let what = if *appear { "didn't appear" } else { "didn't go away" };
                        soft_fail(ctx, &ins.id, "Wait for element", format!("{selector} {what} within {timeout} ms"))?;
                        break;
                    }
                    if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                        break;
                    }
                }
            }
            InstructionKind::BrowserScript { code, variable } => {
                let ran = ctx
                    .resolve(code, depth)
                    .and_then(|c| c.eval_text())
                    .and_then(|code| {
                        let stopped = || ctx.stop_flag.as_ref().is_some_and(stop_requested);
                        crate::browser::request_until("script", serde_json::json!({ "code": code }), BROWSER_SCRIPT_LIMIT, stopped)
                    });
                match ran {
                    Ok(result) => set_variable(ctx, variable, Evaluated::Text(crate::browser::text_field(&result, "value"))),
                    Err(e) => soft_fail(ctx, &ins.id, "Run JavaScript", e)?,
                }
            }
            InstructionKind::HttpRequest {
                method,
                url,
                body,
                headers,
                variable,
            } => {
                let response = (|| {
                    let url = ctx.resolve(url, depth)?.eval_text()?;
                    let body = ctx.resolve(body, depth)?.eval_text()?;
                    let headers = ctx.resolve(headers, depth)?.eval_text()?;
                    crate::http::request(method, &url, &body, &headers)
                })();
                match response {
                    Ok(text) => {
                        if !variable.is_empty()
                            && let Ok(mut vars) = ctx.variables.lock()
                        {
                            vars.insert(variable.clone(), Evaluated::Text(text));
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Web request", e)?,
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
                    if let Some(flag) = &ctx.stop_flag {
                        if stop_requested(flag) {
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
            InstructionKind::Forever { body } => {
                let mut iterations = 0;
                loop {
                    if let Some(flag) = &ctx.stop_flag {
                        if stop_requested(flag) {
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
            InstructionKind::While { condition, body } => {
                let mut iterations = 0;
                loop {
                    if let Some(flag) = &ctx.stop_flag {
                        if stop_requested(flag) {
                            break;
                        }
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
                    let should_stop = move || flag.as_ref().is_some_and(|f| stop_requested(f));
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
            InstructionKind::Command(command) => {
                println!("Starting command: {command}");
                match shell_process(command).spawn() {
                    // Reap it when it exits so it doesn't linger as a zombie.
                    Ok(mut child) => {
                        std::thread::spawn(move || child.wait());
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Command", e)?,
                }
            }
            InstructionKind::CommandWait(command) => {
                println!("Running command: {command}");
                if let Err(e) = shell_process(command).status() {
                    soft_fail(ctx, &ins.id, "Command", e)?;
                }
            }
            InstructionKind::Token(_) if ctx.input_unavailable.is_some() => {
                if !ctx.input_unavailable_reported {
                    ctx.input_unavailable_reported = true;
                    let reason = ctx.input_unavailable.clone().unwrap_or_default();
                    soft_fail(ctx, &ins.id, "Input", reason)?;
                }
            }
            InstructionKind::Token(token) => match token {
                InputToken::Text(value) => {
                    let text = match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                        Ok(t) => t,
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Type text", e)?;
                            continue;
                        }
                    };
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Flow::Normal);
                        }
                    };
                    if let Err(err) = em.text(&text) {
                        warn!("Failed to type text '{}': {}", text, err);
                    }
                }
                InputToken::Key(key, direction) => {
                    let text = match ctx.resolve(key, depth).and_then(|v| v.eval_text()) {
                        Ok(t) => t,
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Key", e)?;
                            continue;
                        }
                    };
                    let mut keys: Vec<MacroKey> = crate::input::key_names::parse_key_combo(&text)
                        .into_iter()
                        .map(normalize_modifier_key)
                        .collect();
                    // In a combo, `Control+A` means the A key; Shift is
                    // added only when the combo names it.
                    if keys.len() > 1 {
                        for key in &mut keys {
                            if let MacroKey::Unicode(c) = key {
                                *c = c.to_ascii_lowercase();
                            }
                        }
                    }
                    if keys.is_empty() {
                        soft_fail(ctx, &ins.id, "Key", format!("no key in '{text}'"))?;
                        continue;
                    }
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Flow::Normal);
                        }
                    };
                    // Press in order, release in reverse, like a typed chord.
                    let press = !matches!(direction, Direction::Release);
                    let release = !matches!(direction, Direction::Press);
                    let mut errors = Vec::new();
                    let mut emit = |key: &MacroKey, dir: Direction, pressed_keys: &mut Vec<MacroKey>| {
                        let key_for_event = match (key, &dir) {
                            (MacroKey::Unicode(c), Direction::Click)
                                if shift_is_pressed(pressed_keys) && c.is_ascii_lowercase() =>
                            {
                                MacroKey::Unicode(c.to_ascii_uppercase())
                            }
                            _ => key.clone(),
                        };
                        match em.key(key_for_event, dir.clone()) {
                            Ok(()) => match dir {
                                Direction::Press => {
                                    if !pressed_keys.contains(key) {
                                        pressed_keys.push(key.clone());
                                    }
                                }
                                Direction::Release => pressed_keys.retain(|k| k != key),
                                Direction::Click => {}
                            },
                            Err(err) => errors.push(format!("{key:?} ({dir:?}): {err}")),
                        }
                    };
                    if keys.len() == 1 {
                        emit(&keys[0], direction.clone(), ctx.pressed_keys);
                    } else {
                        if press {
                            for key in &keys {
                                emit(key, Direction::Press, ctx.pressed_keys);
                            }
                        }
                        if release {
                            for key in keys.iter().rev() {
                                emit(key, Direction::Release, ctx.pressed_keys);
                            }
                        }
                    }
                    // Finish the release sequence before reporting a failed
                    // key; Try/Catch may leave this block immediately.
                    drop(em);
                    if !errors.is_empty() {
                        soft_fail(ctx, &ins.id, "Key", errors.join("; "))?;
                    }
                }
                InputToken::Raw(keycode, direction) => {
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Flow::Normal);
                        }
                    };
                    if let Err(err) = em.raw_keycode(*keycode, direction.clone()) {
                        warn!("Failed to emit raw keycode {}: {}", keycode, err);
                    }
                }
                InputToken::Button(button, direction) => {
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Flow::Normal);
                        }
                    };
                    match em.button(button.clone(), direction.clone()) {
                        Ok(()) => match direction {
                            Direction::Press => {
                                if !ctx.pressed_buttons.contains(button) {
                                    ctx.pressed_buttons.push(button.clone());
                                }
                            }
                            Direction::Release => ctx.pressed_buttons.retain(|b| b != button),
                            // Press+release in one call - nothing left held.
                            Direction::Click => {}
                        },
                        Err(err) => {
                            warn!(
                                "Failed to click button {:?} ({:?}): {}",
                                button, direction, err
                            );
                        }
                    }
                }
                InputToken::MoveMouse(x, y, coordinate) => {
                    let x = match ctx.resolve(x, depth).and_then(|v| v.eval_number()) {
                        Ok(v) => v.round() as i32,
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Move mouse", e)?;
                            continue;
                        }
                    };
                    let y = match ctx.resolve(y, depth).and_then(|v| v.eval_number()) {
                        Ok(v) => v.round() as i32,
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Move mouse", e)?;
                            continue;
                        }
                    };
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Flow::Normal);
                        }
                    };
                    let result = match coordinate {
                        Coordinate::Rel => em.move_mouse_rel(x, y),
                        Coordinate::Abs => em.move_mouse_abs(x, y),
                    };
                    if let Err(err) = result {
                        warn!("Failed to move mouse ({}, {}): {}", x, y, err);
                    }
                }
                InputToken::Scroll(amount, axis) => {
                    let amount = match ctx.resolve(amount, depth).and_then(|v| v.eval_number()) {
                        Ok(v) => v.round() as i32,
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Scroll", e)?;
                            continue;
                        }
                    };
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Flow::Normal);
                        }
                    };
                    if let Err(err) = em.scroll(amount, axis.clone()) {
                        warn!("Failed to scroll by {}: {}", amount, err);
                    }
                }
            },
            InstructionKind::SetVariable(name, value) => {
                match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                    Ok(result) => {
                        if let Ok(mut vars) = ctx.variables.lock() {
                            vars.insert(name.clone(), result);
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Set variable", e)?,
                }
            }
            InstructionKind::ChangeVariable(name, value) => {
                match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                    // The delta must be numeric - text/bool are a deliberate no-op.
                    Ok(Evaluated::Text(_) | Evaluated::Bool(_)) => {}
                    Ok(Evaluated::Number(delta)) => {
                        if let Ok(mut vars) = ctx.variables.lock() {
                            // Non-numeric (or missing) current value coerces
                            // to 0 before adding, same leniency Scratch uses.
                            let current = vars
                                .get(name)
                                .and_then(|e| e.as_number().ok())
                                .unwrap_or(0.0);
                            vars.insert(name.clone(), Evaluated::Number(current + delta));
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Change variable", e)?,
                }
            }
            InstructionKind::SetClipboard(value) => {
                match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                    Ok(text) => {
                        if let Err(e) = crate::clipboard::set_text(text) {
                            soft_fail(ctx, &ins.id, "Set clipboard", e)?;
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Set clipboard", e)?,
                }
            }
            InstructionKind::AddToList { value, name } => {
                match ctx.resolve(value, depth).and_then(|v| v.eval()) {
                    Ok(value) => match ListItem::from_evaluated(value) {
                        Some(item) => {
                            if let Ok(mut lists) = ctx.lists.lock() {
                                if let Some(list) = lists.get_mut(name) {
                                    list.push(item);
                                }
                            }
                        }
                        None => soft_fail(ctx, &ins.id, "Add to list", "list items must be number or text")?,
                    },
                    Err(e) => soft_fail(ctx, &ins.id, "Add to list", e)?,
                }
            }
            InstructionKind::DeleteOfList { index, name } => {
                match ctx.resolve(index, depth).and_then(|v| v.eval_number()) {
                    Ok(index) => {
                        if let Ok(mut lists) = ctx.lists.lock() {
                            if let Some(list) = lists.get_mut(name) {
                                if let Some(index) = list_index(index, list.len(), false) {
                                    list.remove(index);
                                }
                            }
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Delete list item", e)?,
                }
            }
            InstructionKind::DeleteAllOfList { name } => {
                if let Ok(mut lists) = ctx.lists.lock() {
                    if let Some(list) = lists.get_mut(name) {
                        list.clear();
                    }
                }
            }
            InstructionKind::ShiftList { name, amount } => {
                match ctx.resolve(amount, depth).and_then(|v| v.eval_number()) {
                    Ok(amount) => {
                        if let Ok(mut lists) = ctx.lists.lock() {
                            if let Some(list) = lists.get_mut(name) {
                                if !list.is_empty() {
                                    let len = list.len();
                                    let distance = amount.round() as isize;
                                    if distance >= 0 {
                                        list.rotate_right(distance as usize % len);
                                    } else {
                                        list.rotate_left(distance.unsigned_abs() % len);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Shift list", e)?,
                }
            }
            InstructionKind::InsertIntoList { value, index, name } => {
                let value = ctx.resolve(value, depth).and_then(|v| v.eval());
                let index = ctx.resolve(index, depth).and_then(|v| v.eval_number());
                match (value, index) {
                    (Ok(value), Ok(index)) => match ListItem::from_evaluated(value) {
                        Some(item) => {
                            if let Ok(mut lists) = ctx.lists.lock() {
                                if let Some(list) = lists.get_mut(name) {
                                    if let Some(index) = list_index(index, list.len(), true) {
                                        list.insert(index, item);
                                    }
                                }
                            }
                        }
                        None => {
                            soft_fail(ctx, &ins.id, "Insert into list", "list items must be number or text")?
                        }
                    },
                    (Err(e), _) | (_, Err(e)) => soft_fail(ctx, &ins.id, "Insert into list", e)?,
                }
            }
            InstructionKind::ReplaceItemOfList { index, name, value } => {
                let index = ctx.resolve(index, depth).and_then(|v| v.eval_number());
                let value = ctx.resolve(value, depth).and_then(|v| v.eval());
                match (index, value) {
                    (Ok(index), Ok(value)) => match ListItem::from_evaluated(value) {
                        Some(item) => {
                            if let Ok(mut lists) = ctx.lists.lock() {
                                if let Some(list) = lists.get_mut(name) {
                                    if let Some(index) = list_index(index, list.len(), false) {
                                        list[index] = item;
                                    }
                                }
                            }
                        }
                        None => {
                            soft_fail(ctx, &ins.id, "Replace list item", "list items must be number or text")?
                        }
                    },
                    (Err(e), _) | (_, Err(e)) => soft_fail(ctx, &ins.id, "Replace list item", e)?,
                }
            }
            InstructionKind::ReverseList { name } => {
                if let Ok(mut lists) = ctx.lists.lock() {
                    if let Some(list) = lists.get_mut(name) {
                        list.reverse();
                    }
                }
            }
        }
    }

    Ok(Flow::Normal)
}

/// Launches `command` - the already-resolved, platform-specific launch
/// string an `InstructionKind::OpenApp` carries - with its launch options.
/// `make_backend`, or an `UnavailableBackend` when there is none, so macros
/// still run and their input blocks say why they can't.
pub fn make_backend_or_unavailable() -> Arc<Mutex<dyn InputBackend>> {
    make_backend().unwrap_or_else(|| Arc::new(Mutex::new(crate::macros::backend::UnavailableBackend::new())))
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
#[cfg(test)]
mod tests;
