use crate::input::types::{Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::{Evaluated, Op, Value};
use crate::macros::backend::{create_backend, InputBackend};
use crate::macros::host_clock;
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{BlockKind, Instruction, InstructionKind, Macro};
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
        Value::Call { block_id, args, .. } => {
            // Args evaluate in the caller's scope, fully resolved to
            // concrete values before call_block swaps in the callee's scope.
            // Reporter calls carry no branch bodies (blockstitch's `Value`
            // has nowhere to put them), so the branch env stays empty.
            let mut evaluated_args = Vec::with_capacity(args.len());
            for a in args {
                let resolved = resolve_calls_and_params(a, ctx, depth)?;
                evaluated_args.push(resolved.eval()?);
            }
            match call_block(block_id, evaluated_args, Vec::new(), ctx, depth + 1)? {
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

/// The abilities a block needs permission for.
fn guarded_abilities(kind: &InstructionKind) -> &'static [&'static str] {
    match kind {
        InstructionKind::Command(_)
        | InstructionKind::CommandWait(_)
        | InstructionKind::OpenApp { .. }
        | InstructionKind::CloseApp { .. } => &["commands"],
        InstructionKind::HttpRequest { .. } => &["network"],
        InstructionKind::WriteFile { .. } => &["files_write"],
        InstructionKind::FindImage { .. } | InstructionKind::WaitForImage { .. } => &["screen"],
        InstructionKind::DetectObjectsInWindow { .. } | InstructionKind::WaitForDetection { .. } => &["screen"],
        InstructionKind::ClickImage { .. } | InstructionKind::AimAtDetection { .. } => &["screen", "input"],
        InstructionKind::Token(_) | InstructionKind::PasteText(_) | InstructionKind::AimStep { .. } => &["input"],
        InstructionKind::RunMacro { .. } | InstructionKind::StopMacroNamed(_) => &["macros"],
        InstructionKind::SetGlobal { .. } | InstructionKind::ChangeGlobal { .. } => &["globals"],
        _ => &[],
    }
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

/// A detection query from a detect block's slots.
#[allow(clippy::too_many_arguments)]
fn detect_query(
    ctx: &mut ExecCtx,
    depth: u32,
    pattern: &Value,
    by: &str,
    detector: &Value,
    label: &Value,
    confidence: &Value,
    sort: &str,
    region: &str,
) -> Result<crate::detect::Query, String> {
    Ok(crate::detect::Query {
        pattern: ctx.resolve(pattern, depth)?.eval_text()?,
        by: by.parse()?,
        detector: ctx.resolve(detector, depth)?.eval_text()?,
        label: ctx.resolve(label, depth)?.eval_text()?,
        min_score: (ctx.resolve(confidence, depth)?.eval_number()? / 100.0).clamp(0.0, 1.0),
        sort: sort.parse()?,
        region: crate::detect::parse_region(region)?,
    })
}

/// One tracked detection pass, written to `list` in the query's order.
fn detect_into_list(ctx: &ExecCtx, query: &crate::detect::Query, list: &str, capture: &dyn crate::detect::WindowCapture) -> Result<crate::detect::Frame, String> {
    let frame = crate::detect::detect_with(query, capture)?;
    let kept = crate::detect::overlay::filter(&overlay_scope(ctx, list), frame.detections, frame.crosshair);
    let mut found = crate::detect::track::update(&ctx.source.macro_id, list, query, kept);
    crate::detect::sort(&mut found, query.sort, frame.crosshair);
    if !list.is_empty()
        && let Ok(mut lists) = ctx.lists.lock()
    {
        lists.insert(list.to_string(), found.iter().map(|d| ListItem::Text(d.to_item())).collect());
    }
    Ok(crate::detect::Frame { detections: found, ..frame })
}

/// Overlay spots are per macro and list, like tracks.
fn overlay_scope(ctx: &ExecCtx, list: &str) -> String {
    format!("{}\n{list}", ctx.source.macro_id)
}

fn aim_controller(ctx: &mut ExecCtx, depth: u32, gain: &Value, max_step: &Value, deadzone: &Value) -> Result<crate::detect::AimController, String> {
    Ok(crate::detect::AimController {
        gain: ctx.resolve(gain, depth)?.eval_number()?,
        max_step: ctx.resolve(max_step, depth)?.eval_number()?,
        deadzone: ctx.resolve(deadzone, depth)?.eval_number()?,
    })
}

/// Macros allowed to aim this session, by id.
static AIM_ALLOWED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Asks once per macro per session before aim blocks move the mouse.
/// `Ok(false)` means the user said no (already logged as the block's error).
fn confirm_aim(ctx: &mut ExecCtx, block_id: &str) -> Result<bool, String> {
    let id = ctx.source.macro_id.clone();
    if AIM_ALLOWED.lock().unwrap_or_else(|e| e.into_inner()).contains(&id) {
        return Ok(true);
    }
    let message = "This macro moves the mouse toward what it sees on screen. \
        Only allow it where automation is allowed, never against other players. Let it aim?";
    let request = crate::prompt::Request::new(crate::prompt::Kind::Confirm, &ctx.source.name, message);
    match ask(ctx, block_id, "Aim", request)? {
        Some(reply) if reply.answer == "yes" => {
            AIM_ALLOWED.lock().unwrap_or_else(|e| e.into_inner()).push(id);
            Ok(true)
        }
        Some(_) => {
            soft_fail(ctx, block_id, "Aim", "you didn't allow aiming")?;
            Ok(false)
        }
        None => Ok(false),
    }
}

/// Follows track `target` until it's held centered for `settle_ms`: frames
/// are detected back to back while a mover thread glides the pointer at a
/// high rate. Returns `hit`, `timeout`, `lost` or `stopped`.
fn aim_at(
    ctx: &ExecCtx,
    query: &crate::detect::Query,
    list: &str,
    target: u64,
    aim: crate::detect::AimController,
    timeout_ms: f64,
    settle_ms: f64,
) -> Result<&'static str, String> {
    let started = Instant::now();
    let glide = Arc::new(Mutex::new(crate::detect::Glide::new(aim, rand::random())));
    let done = Arc::new(AtomicBool::new(false));
    let mover = {
        let (glide, done, emulator, stop_flag) = (Arc::clone(&glide), Arc::clone(&done), Arc::clone(&ctx.emulator), ctx.stop_flag.clone());
        std::thread::Builder::new()
            .name("aim-glide".into())
            .spawn(move || {
                raise_current_thread_priority();
                let tick = Duration::from_secs_f64(GLIDE_TICK);
                let mut next = Instant::now();
                while !done.load(Ordering::Relaxed) && !stop_flag.as_ref().is_some_and(stop_requested) {
                    let (dx, dy) = glide.lock().unwrap_or_else(|e| e.into_inner()).tick(started.elapsed().as_secs_f64(), GLIDE_TICK);
                    if (dx, dy) != (0, 0) && emulator.lock().map_err(|e| e.to_string()).and_then(|mut em| em.move_mouse_rel(dx, dy)).is_err() {
                        break;
                    }
                    next += tick;
                    let now = Instant::now();
                    if next > now {
                        spin_sleeper().sleep(next - now);
                    } else {
                        next = now;
                    }
                }
            })
            .map_err(|e| e.to_string())?
    };
    let result = follow(ctx, query, list, target, &glide, started, aim, timeout_ms, settle_ms);
    done.store(true, Ordering::Relaxed);
    let _ = mover.join();
    result
}

/// The detection side of [`aim_at`].
#[allow(clippy::too_many_arguments)]
fn follow(
    ctx: &ExecCtx,
    query: &crate::detect::Query,
    list: &str,
    target: u64,
    glide: &Mutex<crate::detect::Glide>,
    started: Instant,
    aim: crate::detect::AimController,
    timeout_ms: f64,
    settle_ms: f64,
) -> Result<&'static str, String> {
    let mut centered_since: Option<Instant> = None;
    let mut stats = AimStats::default();
    let prefetch = crate::detect::Prefetch::start(&query.pattern, query.by);
    let mut target = target;
    let mut overlay = crate::detect::overlay::OverlayCheck::default();
    loop {
        if ctx.stop_flag.as_ref().is_some_and(stop_requested) {
            stats.report(&ctx.source);
            return Ok("stopped");
        }
        let frame = detect_into_list(ctx, query, list, &prefetch)?;
        stats.add(&frame);
        glide.lock().unwrap_or_else(|e| e.into_inner()).set_aim(aim.for_window(frame.window_height));
        // The frame shows the game as it was a little before the capture.
        let captured = prefetch.captured_at().map_or(0.0, |at| at.duration_since(started).as_secs_f64()) - FRAME_LAG;
        // Turning the view moves everything on screen, so the tracker can
        // give the same target a new id; take over the box where it should be.
        let expected = glide.lock().unwrap_or_else(|e| e.into_inner()).expected_px(started.elapsed().as_secs_f64());
        let found = frame.detections.iter().find(|d| d.id == Some(target)).or_else(|| {
            let (ex, ey) = expected?;
            let off = |d: &&crate::detect::Detection| {
                let (x, y) = d.center();
                (x - frame.crosshair.0 - ex).hypot(y - frame.crosshair.1 - ey)
            };
            frame.detections.iter().filter(|d| off(d) <= d.h.max(RELOCK_MIN_PX)).min_by(|a, b| off(a).total_cmp(&off(b)))
        });
        if let Some(id) = found.and_then(|d| d.id) {
            if id != target {
                overlay.reset();
            }
            target = id;
        }
        match found {
            // Missed this frame; give up once the tracker drops it.
            None => {
                centered_since = None;
                glide.lock().unwrap_or_else(|e| e.into_inner()).hold(started.elapsed().as_secs_f64());
                if !crate::detect::track::alive(&ctx.source.macro_id, list, target) {
                    stats.report(&ctx.source);
                    return Ok("lost");
                }
            }
            Some(d) => {
                let (x, y) = d.center();
                let error = (x - frame.crosshair.0, y - frame.crosshair.1);
                let (turned, now_turned) = {
                    let g = glide.lock().unwrap_or_else(|e| e.into_inner());
                    (g.turned_px(captured), g.turned_px(captured + FRAME_LAG))
                };
                tracing::debug!(
                    "aim frame at {:.3} s: {:.1},{:.1} px off, turned {:.1},{:.1} px",
                    captured + FRAME_LAG,
                    error.0,
                    error.1,
                    now_turned.0,
                    now_turned.1
                );
                if overlay.is_overlay(captured, error, turned) {
                    // Part of the game's HUD: chasing it would only spin the view.
                    tracing::debug!("aim: skipping a box fixed to the screen at {:.0},{:.0} px", error.0, error.1);
                    crate::detect::overlay::remember(&overlay_scope(ctx, list), d, frame.crosshair);
                    stats.report(&ctx.source);
                    return Ok("lost");
                }
                glide.lock().unwrap_or_else(|e| e.into_inner()).observe(captured, error);
                if error.0.hypot(error.1) <= aim.deadzone.max(0.0) {
                    if centered_since.get_or_insert_with(Instant::now).elapsed().as_secs_f64() * 1000.0 >= settle_ms {
                        stats.report(&ctx.source);
                        return Ok("hit");
                    }
                } else {
                    centered_since = None;
                }
            }
        }
        if timeout_ms > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout_ms {
            stats.report(&ctx.source);
            return Ok("timeout");
        }
    }
}

/// Frame timings for one aim, logged when it ends.
#[derive(Default)]
struct AimStats {
    frames: u32,
    capture_ms: f64,
    detect_ms: f64,
    started: Option<Instant>,
}

impl AimStats {
    fn add(&mut self, frame: &crate::detect::Frame) {
        self.started.get_or_insert_with(Instant::now);
        self.frames += 1;
        self.capture_ms += frame.capture_ms;
        self.detect_ms += frame.detect_ms;
    }

    fn report(&self, source: &crate::run_log::Source) {
        let (Some(started), n) = (self.started, self.frames.max(1) as f64) else { return };
        tracing::info!(
            "{}: aimed for {:.1} s at {:.0} frames/s (waited {:.1} ms for capture, model {:.1} ms)",
            source.name,
            started.elapsed().as_secs_f64(),
            self.frames as f64 / started.elapsed().as_secs_f64().max(1e-3),
            self.capture_ms / n,
            self.detect_ms / n
        );
    }
}

impl Macro {
    /// Builds the custom block table from this macro's `BlockHeader` strands.
    pub fn block_table(&self) -> BlockTable {
        let mut table = HashMap::new();
        // Signal strands ride along under reserved keys, header included,
        // so every run entry point can reach them without another argument.
        for (index, strand) in self.graph.strands.iter().enumerate() {
            if matches!(strand.instructions.first().map(|i| &i.kind), Some(InstructionKind::WhenSignal(_))) {
                let runtime = BlockRuntime { input_names: Vec::new(), branch_names: Vec::new(), body: strand.instructions.clone() };
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
                    },
                );
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
        let source = Arc::new(crate::run_log::Source::for_macro(&self.id, &self.name));
        let max_loop_iterations = self.settings.max_loop_iterations;
        // StopMacro needs a flag every strand of this run shares.
        let stop_flag = stop_flag.or_else(|| Some(Arc::new(Mutex::new(true))));
        let block_table = self.block_table();
        // Trigger strands are fired by the app's watchers via
        // `run_fired_strand`, never by a manual Run.
        let entry_strands: Vec<Vec<Instruction>> = self
            .graph
            .strands
            .into_iter()
            .filter(|s| matches!(s.instructions.first().map(|i| &i.kind), Some(InstructionKind::WhenRan)))
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

        if let Some(Err(e)) = guarded_abilities(&ins.kind).iter().map(|a| crate::permissions::require(a)).find(Result::is_err) {
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
            InstructionKind::OpenApp { command, .. } => {
                println!("Opening app: {command}");
                if let Err(e) = open_app(command) {
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
            InstructionKind::WaitUntil { condition, timeout } => {
                let timeout = match ctx.resolve(timeout, depth).and_then(|v| v.eval_number()) {
                    Ok(ms) => ms.max(0.0),
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
                    if timeout > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout {
                        soft_fail(ctx, &ins.id, "Wait until", format!("still false after {timeout} ms"))?;
                        break;
                    }
                    if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                        break;
                    }
                }
            }
            InstructionKind::FindImage { image, tolerance, x_variable, y_variable } => {
                let found = ctx
                    .resolve(tolerance, depth)
                    .and_then(|t| t.eval_number())
                    .and_then(|t| crate::screen::find_image(&*crate::screen::image::load_cached(image)?, t));
                match found {
                    Ok(at) => {
                        let (x, y) = at.map_or((-1.0, -1.0), |(x, y)| (x.round(), y.round()));
                        if let Ok(mut vars) = ctx.variables.lock() {
                            for (name, value) in [(x_variable, x), (y_variable, y)] {
                                if !name.is_empty() {
                                    vars.insert(name.clone(), Evaluated::Number(value));
                                }
                            }
                        }
                    }
                    Err(e) => soft_fail(ctx, &ins.id, "Find image", e)?,
                }
            }
            InstructionKind::ClickImage { image, tolerance, button } => {
                let clicked = (|| {
                    let tolerance = ctx.resolve(tolerance, depth)?.eval_number()?;
                    let (x, y) = crate::screen::find_image(&*crate::screen::image::load_cached(image)?, tolerance)?
                        .ok_or("the image isn't on screen")?;
                    let mut em = ctx.emulator.lock().map_err(|e| e.to_string())?;
                    em.move_mouse_abs(x.round() as i32, y.round() as i32)?;
                    em.button(button.clone(), Direction::Click)
                })();
                if let Err(e) = clicked {
                    soft_fail(ctx, &ins.id, "Click image", e)?;
                }
            }
            InstructionKind::WaitForImage { image, tolerance, appear, timeout } => {
                let setup = (|| {
                    let tolerance = ctx.resolve(tolerance, depth)?.eval_number()?;
                    let timeout = ctx.resolve(timeout, depth)?.eval_number()?.max(0.0);
                    Ok::<_, String>((crate::screen::image::load_cached(image)?, tolerance, timeout))
                })();
                let (needle, tolerance, timeout) = match setup {
                    Ok(setup) => setup,
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait for image", e)?;
                        return Ok(Flow::Normal);
                    }
                };
                let started = Instant::now();
                loop {
                    match crate::screen::find_image(&needle, tolerance) {
                        Ok(at) if at.is_some() == *appear => break,
                        Ok(_) => {}
                        Err(e) => {
                            soft_fail(ctx, &ins.id, "Wait for image", e)?;
                            break;
                        }
                    }
                    if timeout > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout {
                        let what = if *appear { "didn't appear" } else { "didn't go away" };
                        soft_fail(ctx, &ins.id, "Wait for image", format!("the image {what} within {timeout} ms"))?;
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
                            Err(err) => warn!("Failed to press key {:?} ({:?}): {}", key, dir, err),
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
/// string an `InstructionKind::OpenApp` carries. Each platform needs a
/// different launcher: Windows' `start` handles a `.lnk` path directly;
/// macOS' `open` handles an `.app` bundle path; a plain `sh -c` covers
/// Linux's cleaned `Exec=` line (may still carry shell syntax), run on the
/// host when sandboxed since the app lives outside the Flatpak.
#[cfg(target_os = "windows")]
fn open_app(command: &str) -> std::io::Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "", command])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "macos")]
fn open_app(command: &str) -> std::io::Result<()> {
    Command::new("open").arg(command).spawn().map(|_| ())
}

#[cfg(target_os = "linux")]
fn open_app(command: &str) -> std::io::Result<()> {
    crate::flatpak::host_command("sh")
        .arg("-c")
        .arg(command)
        .spawn()
        .map(|_| ())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn open_app(_command: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening apps is not supported on this platform",
    ))
}

/// Terminates the app an `InstructionKind::CloseApp` names. No cross-platform
/// "close this launch string" API exists, so each platform derives a
/// best-effort process matcher from whatever `OpenApp`'s picker captured:
/// the executable basename from a Linux `Exec=` line or a Windows `.lnk`
/// path, or (macOS) the app's display name, since Launch Services addresses
/// running apps by name.
#[cfg(target_os = "linux")]
fn close_app(command: &str, _name: &str) -> std::io::Result<()> {
    let proc_name = command
        .split_whitespace()
        .next()
        .and_then(|c| c.rsplit('/').next())
        .unwrap_or("");
    if proc_name.is_empty() {
        return Ok(());
    }
    crate::flatpak::host_command("killall").arg(proc_name).status().map(|_| ())
}

#[cfg(target_os = "windows")]
fn close_app(command: &str, _name: &str) -> std::io::Result<()> {
    let Some(stem) = std::path::Path::new(command)
        .file_stem()
        .and_then(|s| s.to_str())
    else {
        return Ok(());
    };
    if stem.is_empty() {
        return Ok(());
    }
    Command::new("taskkill")
        .args(["/IM", &format!("{stem}.exe"), "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|_| ())
}

#[cfg(target_os = "macos")]
fn close_app(_command: &str, name: &str) -> std::io::Result<()> {
    if name.is_empty() {
        return Ok(());
    }
    // AppleScript string literal - strip quotes/backslashes rather than
    // escaping them, since `name` only ever comes from the picker's own app
    // list (never freeform user text) and doesn't need real escaping support.
    let sanitized: String = name.chars().filter(|c| *c != '"' && *c != '\\').collect();
    let script = format!("tell application \"{sanitized}\" to quit");
    Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn close_app(_command: &str, _name: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "closing apps is not supported on this platform",
    ))
}

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
mod tests {
    use super::*;
    use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
    use crate::macros::{
        BlockDef, BlockPiece, BlockShape, InputValueType, MacroGraph, Strand,
        default_block_color,
    };
    use blockstitch_core::graph::resolve_list_reporters;

    struct NoopBackend;
    impl InputBackend for NoopBackend {
        fn key(&mut self, _key: MacroKey, _dir: Direction) -> Result<(), String> {
            Ok(())
        }
        fn raw_keycode(&mut self, _keycode: u16, _dir: Direction) -> Result<(), String> {
            Ok(())
        }
        fn button(&mut self, _button: MacroButton, _dir: Direction) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_rel(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_abs(&mut self, _x: i32, _y: i32) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _amount: i32, _axis: Axis) -> Result<(), String> {
            Ok(())
        }
        fn text(&mut self, _s: &str) -> Result<(), String> {
            Ok(())
        }
        fn cursor_pos(&self) -> Option<(i32, i32)> {
            None
        }
    }

    fn when_ran_strand(id: &str, wait_ms: f64) -> Strand {
        Strand {
            id: id.to_string(),
            x: 0,
            y: 0,
            instructions: vec![
                Instruction::new(InstructionKind::WhenRan),
                Instruction::new(InstructionKind::Wait(Value::number(wait_ms))),
            ],
        }
    }

    fn empty_vars() -> VariableStore {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn noop_emulator() -> Arc<Mutex<dyn InputBackend>> {
        Arc::new(Mutex::new(NoopBackend))
    }

    #[test]
    fn list_commands_and_reporters_share_the_same_literal_store() {
        use crate::input::value::Op;

        let text = |value: &str| Value::Text {
            value: value.to_string(),
        };
        let reporter = |op, args| Value::Op {
            op,
            args,
            saved: Box::new(Value::number(0.0)),
        };
        let variables: VariableStore = Arc::new(Mutex::new(HashMap::new()));
        let lists: ListStore = Arc::new(Mutex::new(HashMap::from([(
            "items".to_string(),
            vec![ListItem::Text("one".into()), ListItem::Number(2.0)],
        )])));

        run_instructions_with_lists(
            vec![
                Instruction::new(InstructionKind::AddToList {
                    value: Value::number(3.0),
                    name: "items".into(),
                }),
                Instruction::new(InstructionKind::InsertIntoList {
                    value: text("z"),
                    index: Value::number(2.0),
                    name: "items".into(),
                }),
                Instruction::new(InstructionKind::ReplaceItemOfList {
                    index: Value::number(3.0),
                    name: "items".into(),
                    value: text("y"),
                }),
                Instruction::new(InstructionKind::ShiftList {
                    name: "items".into(),
                    amount: Value::number(1.0),
                }),
                Instruction::new(InstructionKind::DeleteOfList {
                    index: Value::number(2.0),
                    name: "items".into(),
                }),
                Instruction::new(InstructionKind::ReverseList {
                    name: "items".into(),
                }),
                Instruction::new(InstructionKind::SetVariable(
                    "item".into(),
                    reporter(Op::from_name("ListItem"), vec![Value::number(1.0), text("items")]),
                )),
                Instruction::new(InstructionKind::SetVariable(
                    "position".into(),
                    reporter(Op::from_name("ListItemNumber"), vec![text("z"), text("items")]),
                )),
                Instruction::new(InstructionKind::SetVariable(
                    "amount".into(),
                    reporter(Op::from_name("ListAmount"), vec![text("y"), text("items")]),
                )),
                Instruction::new(InstructionKind::SetVariable(
                    "length".into(),
                    reporter(Op::from_name("ListLength"), vec![text("items")]),
                )),
                Instruction::new(InstructionKind::SetVariable(
                    "contains".into(),
                    reporter(Op::from_name("ListContains"), vec![text("items"), text("z")]),
                )),
                Instruction::new(InstructionKind::SetVariable(
                    "exists".into(),
                    reporter(Op::from_name("ListItemExists"), vec![Value::number(3.0), text("items")]),
                )),
                Instruction::new(InstructionKind::SetVariable(
                    "empty".into(),
                    reporter(Op::from_name("ListIsEmpty"), vec![text("items")]),
                )),
                Instruction::new(InstructionKind::DeleteAllOfList {
                    name: "items".into(),
                }),
                Instruction::new(InstructionKind::SetVariable(
                    "now_empty".into(),
                    reporter(Op::from_name("ListIsEmpty"), vec![text("items")]),
                )),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&variables),
            Arc::clone(&lists),
        );

        assert!(lists.lock().unwrap().get("items").unwrap().is_empty());
        let variables = variables.lock().unwrap();
        assert_eq!(variables.get("item"), Some(&Evaluated::Text("y".into())));
        assert_eq!(variables.get("position"), Some(&Evaluated::Number(2.0)));
        assert_eq!(variables.get("amount"), Some(&Evaluated::Number(1.0)));
        assert_eq!(variables.get("length"), Some(&Evaluated::Number(3.0)));
        assert_eq!(variables.get("contains"), Some(&Evaluated::Bool(true)));
        assert_eq!(variables.get("exists"), Some(&Evaluated::Bool(true)));
        assert_eq!(variables.get("empty"), Some(&Evaluated::Bool(false)));
        assert_eq!(variables.get("now_empty"), Some(&Evaluated::Bool(true)));
    }

    #[test]
    fn resolve_list_reporters_supports_canvas_preview_without_running_a_macro() {
        use crate::input::value::Op;

        let lists = HashMap::from([(
            "items".to_string(),
            vec![ListItem::Text("first".into()), ListItem::Number(7.0)],
        )]);
        let list_item = Value::Op {
            op: Op::from_name("ListItem"),
            args: vec![
                Value::number(2.0),
                Value::Text {
                    value: "items".into(),
                },
            ],
            saved: Box::new(Value::number(0.0)),
        };
        let preview =
            resolve_list_reporters(&list_item, &lists).and_then(|value| value.eval_text());
        assert_eq!(preview, Ok("7".to_string()));

        let nested_length = Value::Op {
            op: Op::Add,
            args: vec![
                Value::Op {
                    op: Op::from_name("ListLength"),
                    args: vec![Value::Text {
                        value: "items".into(),
                    }],
                    saved: Box::new(Value::number(0.0)),
                },
                Value::number(1.0),
            ],
            saved: Box::new(Value::number(0.0)),
        };
        let preview =
            resolve_list_reporters(&nested_length, &lists).and_then(|value| value.eval_text());
        assert_eq!(preview, Ok("3".to_string()));
    }

    /// Three entry strands each waiting 150ms should finish in well under
    /// 3×150ms if they're actually running concurrently rather than one
    /// after another.
    #[test]
    fn run_executes_all_when_ran_strands_concurrently() {
        let mac = Macro {
            id: "m".into(),
            name: "Concurrent".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    when_ran_strand("a", 150.0),
                    when_ran_strand("b", 150.0),
                    when_ran_strand("c", 150.0),
                    Strand {
                        id: "inert".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![Instruction::new(InstructionKind::Wait(Value::number(
                            150.0,
                        )))],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        let start = Instant::now();
        mac.run(noop_emulator(), None, 1.0, empty_vars());
        assert!(
            start.elapsed() < Duration::from_millis(400),
            "entry strands ran sequentially instead of concurrently"
        );
    }

    /// `run_with_offset` should backdate the first `Wait`'s deadline anchor
    /// by `initial_offset` - a macro whose only strand waits 200ms should
    /// return in roughly (200ms - offset) wall-clock time, proving the
    /// anchor is `Instant::now() - initial_offset`, not just `Instant::now()`
    /// with the offset silently ignored.
    #[test]
    fn run_with_offset_backdates_the_first_wait_deadline() {
        let mac = Macro {
            id: "m".into(),
            name: "Offset".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![when_ran_strand("a", 200.0)],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        let start = Instant::now();
        mac.run_with_offset(
            noop_emulator(),
            None,
            1.0,
            empty_vars(),
            Duration::from_millis(80),
        );
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(170),
            "offset didn't shorten the wait as expected: {elapsed:?}"
        );
        assert!(
            elapsed >= Duration::from_millis(90),
            "returned suspiciously fast, offset may have overshot: {elapsed:?}"
        );
    }

    /// A stop flag flipped false mid-run should cut every concurrently
    /// running entry strand short, not just one of them.
    #[test]
    fn stop_flag_stops_every_concurrent_strand() {
        let long_wait = 5_000.0;
        let mac = Macro {
            id: "m".into(),
            name: "Stoppable".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    when_ran_strand("a", long_wait),
                    when_ran_strand("b", long_wait),
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        let stop_flag = Arc::new(Mutex::new(true));
        let flag_clone = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            *flag_clone.lock().unwrap() = false;
        });
        let start = Instant::now();
        mac.run(noop_emulator(), Some(stop_flag), 1.0, empty_vars());
        assert!(
            start.elapsed() < Duration::from_millis(1000),
            "stop flag didn't stop both concurrent strands promptly"
        );
    }

    #[test]
    fn set_variable_writes_evaluated_value() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(5.0),
            ))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
    }

    #[test]
    fn change_variable_adds_to_existing_numeric_value() {
        let vars = empty_vars();
        vars.lock()
            .unwrap()
            .insert("x".to_string(), Evaluated::Number(10.0));
        run_instructions(
            vec![Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::number(5.0),
            ))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(
            vars.lock().unwrap().get("x"),
            Some(&Evaluated::Number(15.0))
        );
    }

    #[test]
    fn change_variable_coerces_non_numeric_current_value_to_zero() {
        let vars = empty_vars();
        vars.lock()
            .unwrap()
            .insert("x".to_string(), Evaluated::Text("hello".to_string()));
        run_instructions(
            vec![Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::number(5.0),
            ))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
    }

    #[test]
    fn change_variable_with_text_delta_is_a_no_op() {
        let vars = empty_vars();
        vars.lock()
            .unwrap()
            .insert("x".to_string(), Evaluated::Number(10.0));
        run_instructions(
            vec![Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::Text {
                    value: "abc".to_string(),
                },
            ))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(
            vars.lock().unwrap().get("x"),
            Some(&Evaluated::Number(10.0))
        );
    }

    #[test]
    fn set_variable_value_can_read_other_variables() {
        let vars = empty_vars();
        vars.lock()
            .unwrap()
            .insert("y".to_string(), Evaluated::Number(7.0));
        run_instructions(
            vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Var {
                    name: "y".to_string(),
                },
            ))],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(7.0)));
    }

    /// A macro with one custom "double" block (`Return param*2`) called via
    /// `Value::Call` should write the doubled result.
    fn macro_with_double_block(caller_instructions: Vec<Instruction>) -> Macro {
        let block_id = "double".to_string();
        Macro {
            id: "m".into(),
            name: "Double".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: {
                            let mut ins = vec![Instruction::new(InstructionKind::WhenRan)];
                            ins.extend(caller_instructions);
                            ins
                        },
                    },
                    Strand {
                        id: "double_body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                            Instruction::new(InstructionKind::Return(Value::Op {
                                op: crate::input::value::Op::Mul,
                                args: vec![Value::Param { name: "n".into() }, Value::number(2.0)],
                                saved: Box::new(Value::number(0.0)),
                            })),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![
                        BlockPiece::Label {
                            id: "p1".into(),
                            text: "double".into(),
                        },
                        BlockPiece::Input {
                            id: "p2".into(),
                            name: "n".into(),
                            value_type: InputValueType::Any,
                            default_value: None,
                        },
                    ],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        }
    }

    #[test]
    fn custom_block_runs_the_callback_supplied_to_its_branch() {
        let vars = empty_vars();
        let block_id = "with_branch".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Branches".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenRan),
                            Instruction::new(InstructionKind::CallBlock {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![vec![Instruction::new(
                                    InstructionKind::SetVariable(
                                        "ran".into(),
                                        Value::number(1.0),
                                    ),
                                )]],
                            }),
                        ],
                    },
                    Strand {
                        id: "body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                            Instruction::new(InstructionKind::RunBranch("callback".into())),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![BlockPiece::Branch {
                        id: "b1".into(),
                        name: "callback".into(),
                    }],
                    shape: BlockShape::Normal,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(
            vars.lock().unwrap().get("ran"),
            Some(&Evaluated::Number(1.0))
        );
    }

    #[test]
    fn shipped_text_helpers_pack_runs() {
        crate::input::value::register_blockwork_operators();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../blockwork-core/tests/fixtures/blocks/packs/text-helpers.json");
        let pack = crate::block_packs::read_pack(&std::fs::read_to_string(path).unwrap()).unwrap();
        let id = |name: &str| {
            pack.content.block_defs.iter().find(|b| crate::block_packs::block_name(b) == name).unwrap().id.clone()
        };
        let call = |name: &str, text: &str| Value::Call {
            block_id: id(name),
            args: vec![Value::text(text)],
            branches: vec![],
            saved: Box::new(Value::number(0.0)),
        };
        let set = |var: &str, value: Value| Instruction::new(InstructionKind::SetVariable(var.to_string(), value));
        let mut mac = Macro::new("t".into(), String::new(), vec![
            set("cap", call("capitalize (text)", "hello")),
            set("empty", call("capitalize (text)", "")),
            set("last", call("last letter of (text)", "abc")),
            set("blank", call("(text) is empty?", "  ")),
        ]);
        crate::block_packs::merge_into(&mut mac, &pack, "");
        let vars = empty_vars();
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("cap"), Some(&Evaluated::Text("Hello".into())));
        assert_eq!(vars.get("empty"), Some(&Evaluated::Text(String::new())));
        assert_eq!(vars.get("last"), Some(&Evaluated::Text("c".into())));
        assert_eq!(vars.get("blank"), Some(&Evaluated::Bool(true)));
    }

    #[test]
    fn value_call_to_reporter_block_resolves_and_runs() {
        let vars = empty_vars();
        let mac = macro_with_double_block(vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::Call {
                block_id: "double".to_string(),
                args: vec![Value::number(21.0)],
                branches: vec![],
                saved: Box::new(Value::number(0.0)),
            },
        ))]);
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(
            vars.lock().unwrap().get("x"),
            Some(&Evaluated::Number(42.0))
        );
    }

    /// A reporter block whose body never hits `Return` should leave
    /// the caller's `SetVariable` un-applied rather than panicking or
    /// defaulting to some value.
    #[test]
    fn value_call_to_block_without_return_leaves_variable_unset() {
        let vars = empty_vars();
        let block_id = "empty".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Empty".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenRan),
                            Instruction::new(InstructionKind::SetVariable(
                                "x".to_string(),
                                Value::Call {
                                    block_id: block_id.clone(),
                                    args: vec![],
                                    branches: vec![],
                                    saved: Box::new(Value::number(0.0)),
                                },
                            )),
                        ],
                    },
                    Strand {
                        id: "empty_body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![Instruction::new(InstructionKind::BlockHeader(
                            block_id.clone(),
                        ))],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }

    /// A `CallBlock` with real `Wait`s inside its body should take real
    /// time, proving the callee's instructions genuinely execute inline.
    #[test]
    fn call_block_runs_real_instructions_including_wait() {
        let block_id = "waiter".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Waiter".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenRan),
                            Instruction::new(InstructionKind::CallBlock {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                            }),
                        ],
                    },
                    Strand {
                        id: "waiter_body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                            Instruction::new(InstructionKind::Wait(Value::number(150.0))),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![],
                    shape: BlockShape::Normal,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        let start = Instant::now();
        mac.run(noop_emulator(), None, 1.0, empty_vars());
        assert!(
            start.elapsed() >= Duration::from_millis(140),
            "CallBlock's nested Wait didn't actually take real time"
        );
    }

    /// A watcher-fired trigger body can call the macro's custom blocks.
    #[test]
    fn fired_strand_can_call_custom_block() {
        let block_id = "setter".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Fired".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "trigger".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenClipboardChanged),
                            Instruction::new(InstructionKind::CallBlock {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                            }),
                        ],
                    },
                    Strand {
                        id: "setter_body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                            Instruction::new(InstructionKind::SetVariable(
                                "x".to_string(),
                                Value::number(1.0),
                            )),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![],
                    shape: BlockShape::Normal,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        let vars = empty_vars();
        let body = mac.strands[0].instructions[1..].to_vec();
        run_fired_strand(
            crate::run_log::Source::app("Fired"),
            body,
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
            Arc::new(Mutex::new(HashMap::new())),
            mac.block_table(),
            0,
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
    }

    /// A block that calls itself should error past `MAX_CALL_DEPTH` instead
    /// of overflowing the stack.
    #[test]
    fn self_recursive_reporter_block_errs_past_max_depth() {
        let vars = empty_vars();
        let block_id = "loop".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Loop".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenRan),
                            Instruction::new(InstructionKind::SetVariable(
                                "x".to_string(),
                                Value::Call {
                                    block_id: block_id.clone(),
                                    args: vec![],
                                    branches: vec![],
                                    saved: Box::new(Value::number(0.0)),
                                },
                            )),
                        ],
                    },
                    Strand {
                        id: "loop_body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                            Instruction::new(InstructionKind::Return(Value::Call {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            })),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        // Should return promptly (erroring out at MAX_CALL_DEPTH) rather than
        // hanging or crashing the test process via a stack overflow.
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }

    /// One reporter block calling another in its own `Return` expression
    /// should compose correctly, proving multi-level `Value::Call`
    /// resolution nests rather than working one level deep.
    #[test]
    fn reporter_block_composes_with_another_reporter_block() {
        let vars = empty_vars();
        let double_id = "double".to_string();
        let triple_id = "triple".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "Compose".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenRan),
                            Instruction::new(InstructionKind::SetVariable(
                                "x".to_string(),
                                Value::Call {
                                    block_id: triple_id.clone(),
                                    args: vec![Value::number(2.0)],
                                    branches: vec![],
                                    saved: Box::new(Value::number(0.0)),
                                },
                            )),
                        ],
                    },
                    Strand {
                        id: "double_body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(double_id.clone())),
                            Instruction::new(InstructionKind::Return(Value::Op {
                                op: crate::input::value::Op::Mul,
                                args: vec![Value::Param { name: "n".into() }, Value::number(2.0)],
                                saved: Box::new(Value::number(0.0)),
                            })),
                        ],
                    },
                    Strand {
                        id: "triple_body".into(),
                        x: 0,
                        y: 0,
                        // triple(n) = double(n) + n  =>  triple(2) = 4 + 2 = 6
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(triple_id.clone())),
                            Instruction::new(InstructionKind::Return(Value::Op {
                                op: crate::input::value::Op::Add,
                                args: vec![
                                    Value::Call {
                                        block_id: double_id.clone(),
                                        args: vec![Value::Param { name: "n".into() }],
                                        branches: vec![],
                                        saved: Box::new(Value::number(0.0)),
                                    },
                                    Value::Param { name: "n".into() },
                                ],
                                saved: Box::new(Value::number(0.0)),
                            })),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![
                    BlockDef {
                        id: double_id,
                        pieces: vec![BlockPiece::Input {
                            id: "p1".into(),
                            name: "n".into(),
                            value_type: InputValueType::Any,
                            default_value: None,
                        }],
                        shape: BlockShape::ReturnsValue,
                        color: default_block_color(),
                        icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                    },
                    BlockDef {
                        id: triple_id,
                        pieces: vec![BlockPiece::Input {
                            id: "p1".into(),
                            name: "n".into(),
                            value_type: InputValueType::Any,
                            default_value: None,
                        }],
                        shape: BlockShape::ReturnsValue,
                        color: default_block_color(),
                        icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                    },
                ],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(6.0)));
    }

    fn true_cond() -> Value {
        Value::Op {
            op: crate::input::value::Op::True,
            args: vec![],
            saved: Box::new(Value::number(0.0)),
        }
    }
    fn false_cond() -> Value {
        Value::Op {
            op: crate::input::value::Op::False,
            args: vec![],
            saved: Box::new(Value::number(0.0)),
        }
    }

    fn set(name: &str, value: Value) -> Instruction {
        Instruction::new(InstructionKind::SetVariable(name.to_string(), value))
    }

    /// Fails to evaluate: text isn't a number.
    fn bad_number() -> Value {
        Value::op(Op::Add, vec![Value::text("abc"), Value::number(1.0)])
    }

    #[test]
    fn try_catch_runs_catch_body_with_the_error_message() {
        crate::input::value::register_blockwork_operators();
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::TryCatch {
                body: vec![
                    set("a", Value::number(1.0)),
                    Instruction::new(InstructionKind::ThrowError(Value::text("boom"))),
                    set("a", Value::number(2.0)),
                ],
                catch_body: vec![set("e", Value::op(Op::from_name("LastError"), vec![]))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("a"), Some(&Evaluated::Number(1.0)));
        assert_eq!(vars.get("e"), Some(&Evaluated::Text("boom".into())));
    }

    #[test]
    fn failing_block_is_caught_inside_try_and_skipped_outside() {
        let vars = empty_vars();
        run_instructions(
            vec![
                set("y", bad_number()),
                set("after_skip", Value::number(1.0)),
                Instruction::new(InstructionKind::TryCatch {
                    body: vec![set("y", bad_number()), set("after_error", Value::number(1.0))],
                    catch_body: vec![set("caught", Value::number(1.0))],
                }),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("after_skip"), Some(&Evaluated::Number(1.0)));
        assert_eq!(vars.get("after_error"), None);
        assert_eq!(vars.get("caught"), Some(&Evaluated::Number(1.0)));
    }

    fn bump(name: &str) -> Instruction {
        Instruction::new(InstructionKind::ChangeVariable(name.to_string(), Value::number(1.0)))
    }

    #[test]
    fn retry_runs_the_body_again_until_attempts_run_out() {
        let vars = empty_vars();
        run_instructions(
            vec![
                Instruction::new(InstructionKind::TryCatch {
                    body: vec![Instruction::new(InstructionKind::Retry {
                        count: Value::number(3.0),
                        delay: Value::number(0.0),
                        body: vec![bump("tries"), Instruction::new(InstructionKind::ThrowError(Value::text("nope")))],
                    })],
                    catch_body: vec![set("caught", Value::number(1.0))],
                }),
                Instruction::new(InstructionKind::Retry {
                    count: Value::number(5.0),
                    delay: Value::number(0.0),
                    body: vec![bump("ok")],
                }),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("tries"), Some(&Evaluated::Number(3.0)));
        assert_eq!(vars.get("caught"), Some(&Evaluated::Number(1.0)), "giving up is an error a try catches");
        assert_eq!(vars.get("ok"), Some(&Evaluated::Number(1.0)), "a body that works runs once");
    }

    #[test]
    fn loop_limit_stops_a_runaway_loop() {
        let vars = empty_vars();
        run_fired_strand(
            crate::run_log::Source::default(),
            vec![
                Instruction::new(InstructionKind::Forever { body: vec![bump("n")] }),
                set("after", Value::number(1.0)),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(HashMap::new()),
            10,
        );
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("n"), Some(&Evaluated::Number(10.0)));
        assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)), "outside a try the strand carries on");
    }

    #[test]
    fn run_macro_waits_for_the_other_run_to_end() {
        struct Host;
        impl crate::macros::calls::MacroHost for Host {
            fn find(&self, name: &str) -> Option<String> {
                (name == "Helper").then(|| "helper-id".to_string())
            }
            fn start(&self, _id: &str) -> Result<Arc<AtomicBool>, String> {
                let done = Arc::new(AtomicBool::new(false));
                let flag = Arc::clone(&done);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(60));
                    flag.store(true, Ordering::Relaxed);
                });
                Ok(done)
            }
        }
        crate::macros::calls::set_host(Arc::new(Host));
        let vars = empty_vars();
        let started = Instant::now();
        run_instructions(
            vec![
                Instruction::new(InstructionKind::RunMacro { target: Value::text("Nope"), wait: true }),
                Instruction::new(InstructionKind::RunMacro { target: Value::text("Helper"), wait: true }),
                set("after", Value::number(1.0)),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert!(started.elapsed() >= Duration::from_millis(60), "waited for the helper");
        assert_eq!(vars.lock().unwrap().get("after"), Some(&Evaluated::Number(1.0)), "an unknown macro is a soft error");
    }

    /// Answers dialogs by their message: "cancel" cancels, permissions are denied.
    fn install_test_dialogs() {
        use crate::prompt::{Kind, Reply};
        crate::prompt::set_handler(Arc::new(|request: &crate::prompt::Request, _stopped: &dyn Fn() -> bool| {
            let reply = |answer: &str, text: &str| Ok(Reply { answer: answer.into(), text: text.into() });
            match request.kind {
                _ if request.message == "cancel" => reply("cancel", ""),
                Kind::Text => reply("ok", &format!("{}!", request.default)),
                Kind::Confirm => reply("yes", ""),
                Kind::Choice => reply("ok", &request.options[1]),
                Kind::Message => reply("ok", ""),
                Kind::Permission => reply("deny", ""),
            }
        }));
    }

    #[test]
    fn ask_blocks_put_answers_in_variables_and_cancel_stops() {
        install_test_dialogs();
        let vars = empty_vars();
        let lists: ListStore = Arc::new(Mutex::new(HashMap::from([(
            "colors".to_string(),
            vec![ListItem::Text("red".into()), ListItem::Text("blue".into())],
        )])));
        run_instructions_with_lists(
            vec![
                Instruction::new(InstructionKind::AskText { prompt: Value::text("Name?"), default: Value::text("Ada"), name: "name".into() }),
                Instruction::new(InstructionKind::AskYesNo { prompt: Value::text("Sure?"), name: "sure".into() }),
                Instruction::new(InstructionKind::AskChoice { prompt: Value::text("Pick"), list: "colors".into(), name: "pick".into() }),
                Instruction::new(InstructionKind::ShowMessage(Value::text("Done"))),
                Instruction::new(InstructionKind::AskText { prompt: Value::text("cancel"), default: Value::text(""), name: "never".into() }),
                set("after", Value::number(1.0)),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
            lists,
        );
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("name"), Some(&Evaluated::Text("Ada!".into())));
        assert_eq!(vars.get("sure"), Some(&Evaluated::Bool(true)));
        assert_eq!(vars.get("pick"), Some(&Evaluated::Text("blue".into())));
        assert_eq!(vars.get("never"), None);
        assert_eq!(vars.get("after"), None, "cancel stopped the run");
    }

    #[test]
    fn signal_expressions_can_use_unregistered_computed_names() {
        let mut mac = Macro::new(
            "Dynamic signals".into(),
            String::new(),
            vec![
                set("target", Value::text("task:42")),
                Instruction::new(InstructionKind::SendSignal {
                    name: Value::op(Op::Join, vec![Value::text("task:"), Value::text("42")]).into(),
                    wait: true,
                }),
                set("seen", Value::Var { name: "received".into() }),
            ],
        );
        mac.graph.strands.push(Strand::with_instructions(0, 200, vec![
            Instruction::new(InstructionKind::WhenSignal(Value::Var { name: "target".into() }.into())),
            set("received", Value::number(1.0)),
        ]));
        mac.collect_named_signals();
        assert!(mac.signals.is_empty());
        let vars = empty_vars();
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("seen"), Some(&Evaluated::Number(1.0)));
    }

    #[test]
    fn send_signal_and_wait_runs_the_receivers_first() {
        let mut mac = Macro::new(
            "Signals".into(),
            String::new(),
            vec![
                Instruction::new(InstructionKind::SendSignal { name: Value::text("Go").into(), wait: true }),
                set("seen", Value::Var { name: "x".into() }),
            ],
        );
        mac.graph.strands.push(Strand::with_instructions(0, 200, vec![
            Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
            Instruction::new(InstructionKind::Wait(Value::number(40.0))),
            set("x", Value::number(5.0)),
        ]));
        mac.graph.strands.push(Strand::with_instructions(0, 400, vec![
            Instruction::new(InstructionKind::WhenSignal(Value::text("other").into())),
            set("wrong", Value::number(1.0)),
        ]));
        let vars = empty_vars();
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("seen"), Some(&Evaluated::Number(5.0)));
        assert_eq!(vars.get("wrong"), None);
    }

    #[test]
    fn globals_accept_variable_and_computed_names() {
        crate::input::value::register_blockwork_operators();
        let name = "runner-dynamic-global-picker";
        crate::globals::remove(name);
        crate::globals::create(name).unwrap();
        let dynamic = Value::Var { name: "target".into() };
        let computed = Value::op(Op::from_name("Join"), vec![Value::text("runner-dynamic-"), Value::text("global-picker")]);
        let mac = Macro::new(
            "Dynamic globals".into(),
            String::new(),
            vec![
                set("target", Value::text(name)),
                set("initial", Value::op(Op::from_name("GlobalVariable"), vec![dynamic.clone()])),
                Instruction::new(InstructionKind::SetGlobal { name: dynamic.clone(), value: Value::number(4.0) }),
                Instruction::new(InstructionKind::ChangeGlobal { name: computed, by: Value::number(3.0) }),
                set("result", Value::op(Op::from_name("GlobalVariable"), vec![dynamic])),
            ],
        );
        let vars = empty_vars();
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("initial"), Some(&Evaluated::Text(String::new())));
        assert_eq!(vars.get("result"), Some(&Evaluated::Number(7.0)));
        crate::globals::remove(name);
    }

    #[cfg(unix)]
    #[test]
    fn run_command_moves_on_and_run_command_and_wait_waits() {
        let dir = std::env::temp_dir().join(format!("bw-command-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = |kind: InstructionKind| {
            Macro::new("Commands".into(), String::new(), vec![Instruction::new(kind)]).run(noop_emulator(), None, 1.0, empty_vars());
        };
        let (started, waited) = (dir.join("started"), dir.join("waited"));
        run(InstructionKind::Command(format!("sleep 1; touch '{}'", started.display())));
        assert!(!started.exists(), "run command didn't wait");
        run(InstructionKind::CommandWait(format!("sleep 1; touch '{}'", waited.display())));
        assert!(waited.exists(), "run command and wait waited");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !started.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(started.exists(), "the started command still ran");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restricted_macros_skip_blocks_they_may_not_run() {
        install_test_dialogs();
        let marker = std::env::temp_dir().join(format!("bw-perm-{}", std::process::id()));
        let mut mac = Macro::new(
            "Restricted".into(),
            String::new(),
            vec![
                Instruction::new(InstructionKind::CommandWait(format!("touch '{}'", marker.display()))),
                set("after", Value::number(1.0)),
            ],
        );
        mac.id = "runner-restricted".into();
        crate::permissions::restrict(&mac.id, Vec::new());
        let vars = empty_vars();
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        crate::permissions::trust("runner-restricted");
        assert!(!marker.exists(), "the denied command didn't run");
        assert_eq!(vars.lock().unwrap().get("after"), Some(&Evaluated::Number(1.0)));
    }

    #[test]
    fn restricted_macros_ask_before_globals_files_and_input() {
        install_test_dialogs();
        let read = |op: &str, arg: &str| Value::op(Op::from_name(op), vec![Value::text(arg)]);
        let mut mac = Macro::new(
            "Restricted reads".into(),
            String::new(),
            vec![
                Instruction::new(InstructionKind::SetGlobal { name: Value::text("runner-restricted-global"), value: Value::number(1.0) }),
                set("file", read("ReadFile", "/etc/hostname")),
                set("global", read("GlobalVariable", "runner-restricted-global")),
                Instruction::new(InstructionKind::RunMacro { target: Value::text("anything"), wait: true }),
                set("after", Value::number(1.0)),
            ],
        );
        mac.id = "runner-restricted-reads".into();
        crate::permissions::restrict(&mac.id, Vec::new());
        let vars = empty_vars();
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        crate::permissions::trust("runner-restricted-reads");
        assert_eq!(crate::globals::get("runner-restricted-global"), Evaluated::Number(0.0));
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("file"), None);
        assert_eq!(vars.get("global"), None);
        assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)));
    }

    #[test]
    fn wait_until_returns_when_true_and_fails_after_its_timeout() {
        let vars = empty_vars();
        let started = Instant::now();
        let gt = |a: Value, b: Value| Value::op(Op::from_name("Gt"), vec![a, b]);
        run_instructions(
            vec![
                Instruction::new(InstructionKind::WaitUntil { condition: gt(Value::number(2.0), Value::number(1.0)), timeout: Value::number(0.0) }),
                Instruction::new(InstructionKind::TryCatch {
                    body: vec![
                        Instruction::new(InstructionKind::WaitUntil { condition: gt(Value::number(1.0), Value::number(2.0)), timeout: Value::number(150.0) }),
                        set("missed", Value::number(1.0)),
                    ],
                    catch_body: vec![set("timed_out", Value::number(1.0))],
                }),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("timed_out"), Some(&Evaluated::Number(1.0)));
        assert_eq!(vars.get("missed"), None);
        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_millis(150) && elapsed < Duration::from_secs(2), "{elapsed:?}");
    }

    #[test]
    fn lock_blocks_run_their_body_and_give_up_when_stopped() {
        let lock = |name: &str, body| Instruction::new(InstructionKind::WithLock { name: Value::text(name), body });
        let vars = empty_vars();
        run_instructions(vec![lock("runner-free", vec![set("ran", Value::number(1.0))])], noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("ran"), Some(&Evaluated::Number(1.0)));
        assert!(!crate::macros::locks::is_held("runner-free"));

        // Held elsewhere: the strand waits until it's stopped, skipping the body.
        let held = std::thread::spawn(|| {
            let guard = crate::macros::locks::acquire("runner-busy", Duration::from_millis(5), || false);
            std::thread::sleep(Duration::from_millis(400));
            drop(guard);
        });
        std::thread::sleep(Duration::from_millis(50));
        let stop = Arc::new(Mutex::new(true));
        let stopper = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            *stopper.lock().unwrap() = false;
        });
        let vars = empty_vars();
        let started = Instant::now();
        run_instructions(vec![lock("runner-busy", vec![set("ran", Value::number(1.0))])], noop_emulator(), Some(stop), 1.0, Arc::clone(&vars));
        assert!(started.elapsed() < Duration::from_millis(300));
        assert_eq!(vars.lock().unwrap().get("ran"), None);
        held.join().unwrap();
    }

    #[test]
    fn image_blocks_without_an_image_fail_softly() {
        let vars = empty_vars();
        run_instructions(
            vec![
                Instruction::new(InstructionKind::FindImage {
                    image: String::new(),
                    tolerance: Value::number(5.0),
                    x_variable: "x".into(),
                    y_variable: "y".into(),
                }),
                set("after", Value::number(1.0)),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("x"), None);
        assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)));
    }

    #[test]
    fn uncaught_throw_ends_the_strand() {
        let vars = empty_vars();
        run_instructions(
            vec![
                Instruction::new(InstructionKind::ThrowError(Value::text("stop here"))),
                set("after", Value::number(1.0)),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("after"), None);
    }

    #[test]
    fn run_log_errors_name_the_failing_block() {
        let _log = crate::run_log::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let throw = Instruction::new(InstructionKind::ThrowError(Value::text("boom")));
        let source = crate::run_log::Source::for_macro("blame-test", "Blame");
        run_fired_strand(
            source,
            vec![Instruction::new(InstructionKind::Log(Value::text("hi"))), throw.clone()],
            noop_emulator(),
            None,
            1.0,
            empty_vars(),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(HashMap::new()),
            0,
        );
        let errors: Vec<_> = crate::run_log::snapshot()
            .into_iter()
            .filter(|e| e.macro_id == "blame-test" && e.level == crate::run_log::Level::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].block_id, throw.id);
        assert_eq!(errors[0].message, "boom");
    }

    #[test]
    fn without_input_emulation_other_blocks_run_and_input_reports_once() {
        let _log = crate::run_log::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let text = || Instruction::new(InstructionKind::Token(InputToken::Text(Value::text("hi"))));
        let first = text();
        let vars = empty_vars();
        run_fired_strand(
            crate::run_log::Source::for_macro("no-input-test", "No input"),
            vec![first.clone(), text(), set("after", Value::number(1.0))],
            Arc::new(Mutex::new(crate::macros::backend::UnavailableBackend::new())),
            None,
            1.0,
            Arc::clone(&vars),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(HashMap::new()),
            0,
        );
        assert_eq!(vars.lock().unwrap().get("after"), Some(&Evaluated::Number(1.0)));
        let errors: Vec<_> = crate::run_log::snapshot()
            .into_iter()
            .filter(|e| e.macro_id == "no-input-test" && e.level == crate::run_log::Level::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].block_id, first.id);
        assert!(errors[0].message.contains("input emulation isn't available"), "{}", errors[0].message);
    }

    #[test]
    fn combo_letters_dont_add_shift() {
        #[derive(Default)]
        struct Keys(Arc<Mutex<Vec<(MacroKey, Direction)>>>);
        impl InputBackend for Keys {
            fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
                self.0.lock().unwrap().push((key, dir));
                Ok(())
            }
            fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> { Ok(()) }
            fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> { Ok(()) }
            fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
            fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
            fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> { Ok(()) }
            fn text(&mut self, _: &str) -> Result<(), String> { Ok(()) }
            fn cursor_pos(&self) -> Option<(i32, i32)> { None }
        }
        let keys = Keys::default();
        let seen = Arc::clone(&keys.0);
        run_instructions(
            vec![Instruction::new(InstructionKind::Token(InputToken::Key(Value::text("Control+A"), Direction::Click)))],
            Arc::new(Mutex::new(keys)),
            None,
            1.0,
            empty_vars(),
        );
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[1], (MacroKey::Unicode('a'), Direction::Press));
    }

    #[test]
    fn stop_macro_stops_every_strand_of_the_run() {
        let mut stopper = when_ran_strand("b", 0.0);
        stopper.instructions.push(Instruction::new(InstructionKind::StopMacro));
        let mac = Macro {
            id: "m".into(),
            name: "Stops itself".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![when_ran_strand("a", 5_000.0), stopper],
                ..MacroGraph::new()
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        let start = Instant::now();
        mac.run(noop_emulator(), None, 1.0, empty_vars());
        assert!(start.elapsed() < Duration::from_millis(1000), "stop macro didn't end the waiting strand");
    }

    #[test]
    fn if_runs_body_when_condition_true() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::If {
                condition: true_cond(),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
    }

    #[test]
    fn if_skips_body_when_condition_false() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::If {
                condition: false_cond(),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }

    #[test]
    fn if_else_runs_the_matching_branch() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::IfElse {
                condition: true_cond(),
                then_body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
                else_body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(2.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));

        let vars2 = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::IfElse {
                condition: false_cond(),
                then_body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
                else_body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(2.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars2),
        );
        assert_eq!(
            vars2.lock().unwrap().get("x"),
            Some(&Evaluated::Number(2.0))
        );
    }

    #[test]
    fn nested_if_inside_if_runs_only_when_both_true() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::If {
                condition: true_cond(),
                body: vec![Instruction::new(InstructionKind::If {
                    condition: true_cond(),
                    body: vec![Instruction::new(InstructionKind::SetVariable(
                        "x".to_string(),
                        Value::number(1.0),
                    ))],
                })],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));

        let vars2 = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::If {
                condition: true_cond(),
                body: vec![Instruction::new(InstructionKind::If {
                    condition: false_cond(),
                    body: vec![Instruction::new(InstructionKind::SetVariable(
                        "x".to_string(),
                        Value::number(1.0),
                    ))],
                })],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars2),
        );
        assert_eq!(vars2.lock().unwrap().get("x"), None);
    }

    /// A `Return` inside an `If` branch, within a custom block called via
    /// `Value::Call`, should halt the block's body and hand its value back
    /// to the caller - proving `run_block`'s `Ok(Some(_))` bubbles up
    /// through nested `If` the same way it already does through `CallBlock`.
    #[test]
    fn return_inside_if_branch_bubbles_up_through_reporter_block() {
        let vars = empty_vars();
        let block_id = "cond_return".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "CondReturn".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenRan),
                            Instruction::new(InstructionKind::SetVariable(
                                "x".to_string(),
                                Value::Call {
                                    block_id: block_id.clone(),
                                    args: vec![],
                                    branches: vec![],
                                    saved: Box::new(Value::number(0.0)),
                                },
                            )),
                        ],
                    },
                    Strand {
                        id: "body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                            Instruction::new(InstructionKind::If {
                                condition: true_cond(),
                                body: vec![Instruction::new(InstructionKind::Return(Value::number(
                                    42.0,
                                )))],
                            }),
                            // Never reached if the branch's Return correctly halted the body.
                            Instruction::new(InstructionKind::Return(Value::number(0.0))),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(
            vars.lock().unwrap().get("x"),
            Some(&Evaluated::Number(42.0))
        );
    }

    #[test]
    fn repeat_runs_body_n_times() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::Repeat {
                count: Value::number(5.0),
                body: vec![Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
    }

    #[test]
    fn repeat_with_zero_or_negative_count_never_runs_body() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::Repeat {
                count: Value::number(-3.0),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }

    #[test]
    fn escape_loop_stops_repeat_early() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::Repeat {
                count: Value::number(10.0),
                body: vec![
                    Instruction::new(InstructionKind::ChangeVariable(
                        "x".to_string(),
                        Value::number(1.0),
                    )),
                    Instruction::new(InstructionKind::If {
                        condition: true_cond(),
                        body: vec![Instruction::new(InstructionKind::EscapeLoop)],
                    }),
                ],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
    }

    #[test]
    fn continue_loop_skips_rest_of_iteration_but_keeps_looping() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::Repeat {
                count: Value::number(3.0),
                body: vec![
                    Instruction::new(InstructionKind::ChangeVariable(
                        "x".to_string(),
                        Value::number(1.0),
                    )),
                    Instruction::new(InstructionKind::ContinueLoop),
                    // Never reached - proves ContinueLoop halted this iteration.
                    Instruction::new(InstructionKind::ChangeVariable(
                        "x".to_string(),
                        Value::number(100.0),
                    )),
                ],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(3.0)));
    }

    #[test]
    fn while_loop_runs_until_condition_goes_false() {
        let vars = empty_vars();
        vars.lock()
            .unwrap()
            .insert("x".to_string(), Evaluated::Number(0.0));
        run_instructions(
            vec![Instruction::new(InstructionKind::While {
                condition: Value::Op {
                    op: crate::input::value::Op::Lt,
                    args: vec![
                        Value::Var {
                            name: "x".to_string(),
                        },
                        Value::number(5.0),
                    ],
                    saved: Box::new(Value::Bool),
                },
                body: vec![Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
    }

    #[test]
    fn while_loop_with_false_condition_never_runs_body() {
        let vars = empty_vars();
        run_instructions(
            vec![Instruction::new(InstructionKind::While {
                condition: false_cond(),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }

    #[test]
    fn forever_loop_runs_until_escape_loop() {
        let vars = empty_vars();
        vars.lock()
            .unwrap()
            .insert("x".to_string(), Evaluated::Number(0.0));
        run_instructions(
            vec![Instruction::new(InstructionKind::Forever {
                body: vec![
                    Instruction::new(InstructionKind::ChangeVariable(
                        "x".to_string(),
                        Value::number(1.0),
                    )),
                    Instruction::new(InstructionKind::IfElse {
                        condition: Value::Op {
                            op: crate::input::value::Op::Gte,
                            args: vec![
                                Value::Var {
                                    name: "x".to_string(),
                                },
                                Value::number(3.0),
                            ],
                            saved: Box::new(Value::Bool),
                        },
                        then_body: vec![Instruction::new(InstructionKind::EscapeLoop)],
                        else_body: vec![],
                    }),
                ],
            })],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(3.0)));
    }

    /// A `Forever` loop with no `Break`/`Return` must still be interruptible
    /// via the external stop flag rather than spinning the thread forever.
    #[test]
    fn forever_loop_is_interrupted_by_stop_flag() {
        let stop_flag = Arc::new(Mutex::new(true));
        let flag_clone = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            *flag_clone.lock().unwrap() = false;
        });
        let start = Instant::now();
        run_instructions(
            vec![Instruction::new(InstructionKind::Forever { body: vec![] })],
            noop_emulator(),
            Some(stop_flag),
            1.0,
            empty_vars(),
        );
        assert!(
            start.elapsed() < Duration::from_millis(1000),
            "forever loop wasn't stopped by the stop flag"
        );
    }

    /// `Return` inside a `Repeat` must unwind straight through the loop,
    /// same as it already does through `If` - proving loop instructions
    /// forward `Flow::Return` rather than swallowing it like `Break`.
    #[test]
    fn return_inside_repeat_bubbles_up_through_reporter_block() {
        let vars = empty_vars();
        let block_id = "loop_return".to_string();
        let mac = Macro {
            id: "m".into(),
            name: "LoopReturn".into(),
            description: "".into(),
            graph: MacroGraph {
                strands: vec![
                    Strand {
                        id: "caller".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::WhenRan),
                            Instruction::new(InstructionKind::SetVariable(
                                "x".to_string(),
                                Value::Call {
                                    block_id: block_id.clone(),
                                    args: vec![],
                                    branches: vec![],
                                    saved: Box::new(Value::number(0.0)),
                                },
                            )),
                        ],
                    },
                    Strand {
                        id: "body".into(),
                        x: 0,
                        y: 0,
                        instructions: vec![
                            Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                            Instruction::new(InstructionKind::Repeat {
                                count: Value::number(10.0),
                                body: vec![Instruction::new(InstructionKind::Return(Value::number(
                                    7.0,
                                )))],
                            }),
                            // Never reached if Return correctly halted the loop and the body.
                            Instruction::new(InstructionKind::Return(Value::number(0.0))),
                        ],
                    },
                ],
                floating_values: vec![],
                comments: vec![],
                variables: vec![],
                block_defs: vec![BlockDef {
                    id: block_id,
                    pieces: vec![],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                }],
                lists: vec![],
                dicts: vec![],
            },
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(7.0)));
    }

    /// `EscapeLoop`/`ContinueLoop` with no enclosing loop at all (a malformed
    /// path, since the UI itself refuses to place one there) should be a
    /// harmless no-op, mirroring how a stray `Return` at strand level today
    /// is silently discarded.
    #[test]
    fn escape_loop_with_no_enclosing_loop_is_a_harmless_no_op() {
        let vars = empty_vars();
        run_instructions(
            vec![
                Instruction::new(InstructionKind::EscapeLoop),
                Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
            ],
            noop_emulator(),
            None,
            1.0,
            Arc::clone(&vars),
        );
        // EscapeLoop with nothing to catch it halts the whole strand, same as
        // Return does today - the SetVariable after it never runs.
        assert_eq!(vars.lock().unwrap().get("x"), None);
    }
}
