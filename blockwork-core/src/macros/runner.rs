use crate::input::types::{Coordinate, Direction, InputToken, MacroButton, MacroKey};
use crate::input::value::{Evaluated, Op, Value};
use crate::macros::backend::{create_backend, InputBackend};
use crate::macros::priority::raise_current_thread_priority;
use crate::macros::{Instruction, InstructionKind, ListItem, Macro};
use spin_sleep::{SpinSleeper, SpinStrategy};
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;

/// Shared, macro-wide variable store, so a `Set`/`Change` in one strand is
/// visible to others running concurrently.
pub type VariableStore = Arc<Mutex<HashMap<String, Evaluated>>>;
/// Shared, macro-wide list contents, parallel to `VariableStore`.
pub type ListStore = Arc<Mutex<HashMap<String, Vec<ListItem>>>>;

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
    block_table: Arc<HashMap<String, BlockRuntime>>,
    pressed_keys: &'a mut Vec<MacroKey>,
    /// Mouse buttons held by this strand, tracked and cleaned up like
    /// `pressed_keys`. Without this, stopping between a press and release
    /// leaves the button physically stuck down for the rest of the session.
    pressed_buttons: &'a mut Vec<MacroButton>,
    param_env: HashMap<String, Evaluated>,
    branch_env: HashMap<String, Vec<Instruction>>,
}

use crate::macros::is_list_reporter;

/// Resolves one list reporter whose arguments have already been reduced to
/// ordinary values. This is shared by the macro runner and the editor's
/// click-to-preview evaluator so the two always agree about list semantics.
fn resolve_list_reporter(
    op_name: &str,
    args: Vec<Value>,
    lists: &HashMap<String, Vec<ListItem>>,
) -> Result<Value, String> {
    let text = |index: usize| {
        args.get(index)
            .ok_or_else(|| "missing list reporter argument".to_string())
            .and_then(Value::eval_text)
    };
    let number = |index: usize| {
        args.get(index)
            .ok_or_else(|| "missing list reporter argument".to_string())
            .and_then(Value::eval_number)
    };
    let list_name = match op_name {
        "ListItem" | "ListItemNumber" | "ListAmount" | "ListItemExists" => text(1)?,
        "ListContains" => text(0)?,
        "ListLength" | "ListIsEmpty" => text(0)?,
        _ => return Err("not a list reporter".to_string()),
    };
    let list = lists.get(&list_name).cloned().unwrap_or_default();
    let evaluated = match op_name {
        "ListItem" => list_index(number(0)?, list.len(), false)
            .and_then(|index| list.get(index))
            .map(ListItem::evaluated)
            .unwrap_or(Evaluated::Text(String::new())),
        "ListItemNumber" => {
            let needle = ListItem::from_evaluated(args[0].eval()?)
                .ok_or_else(|| "list items must be number or text".to_string())?;
            Evaluated::Number(
                list.iter()
                    .position(|item| item == &needle)
                    .map_or(0, |index| index + 1) as f64,
            )
        }
        "ListAmount" => {
            let needle = ListItem::from_evaluated(args[0].eval()?)
                .ok_or_else(|| "list items must be number or text".to_string())?;
            Evaluated::Number(list.iter().filter(|item| *item == &needle).count() as f64)
        }
        "ListLength" => Evaluated::Number(list.len() as f64),
        "ListContains" => {
            let needle = ListItem::from_evaluated(args[1].eval()?)
                .ok_or_else(|| "list items must be number or text".to_string())?;
            Evaluated::Bool(list.contains(&needle))
        }
        "ListItemExists" => Evaluated::Bool(list_index(number(0)?, list.len(), false).is_some()),
        "ListIsEmpty" => Evaluated::Bool(list.is_empty()),
        _ => unreachable!("validated by is_list_reporter"),
    };
    Ok(evaluated.into_value())
}

/// Replaces list reporter nodes with their values from `lists`, without
/// executing custom-block calls. Useful for non-running contexts such as a
/// canvas reporter preview; the full runner adds call/parameter resolution.
pub fn resolve_list_reporters(
    value: &Value,
    lists: &HashMap<String, Vec<ListItem>>,
) -> Result<Value, String> {
    match value {
        Value::Op { op, args, saved } => {
            let args = args
                .iter()
                .map(|arg| resolve_list_reporters(arg, lists))
                .collect::<Result<Vec<_>, _>>()?;
            if is_list_reporter(op) {
                let name: Box<str> = match op {
                    Op::Ext(name) => name.clone(),
                    _ => unreachable!("validated by is_list_reporter"),
                };
                resolve_list_reporter(&name, args, lists)
            } else {
                Ok(Value::Op {
                    op: op.clone(),
                    args,
                    saved: saved.clone(),
                })
            }
        }
        Value::Call {
            block_id,
            args,
            branches,
            saved,
        } => Ok(Value::Call {
            block_id: block_id.clone(),
            args: args
                .iter()
                .map(|arg| resolve_list_reporters(arg, lists))
                .collect::<Result<Vec<_>, _>>()?,
            branches: branches.clone(),
            saved: saved.clone(),
        }),
        _ => Ok(value.clone()),
    }
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

/// `true` once a stop has been requested on `flag` (the flag itself holds
/// "should keep running", so this is just its negation behind the lock).
fn stop_requested(flag: &Arc<Mutex<bool>>) -> bool {
    !flag.lock().map(|g| *g).unwrap_or(false)
}

/// Converts a Scratch-style 1-based numeric index to a vector offset. An
/// insert may target the slot immediately after the final item; other list
/// commands require an existing item.
fn list_index(value: f64, len: usize, allow_end: bool) -> Option<usize> {
    if !value.is_finite() || value < 1.0 {
        return None;
    }
    let index = value.floor() as usize - 1;
    if index < len || (allow_end && index == len) {
        Some(index)
    } else {
        None
    }
}

impl Macro {
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
        let block_defs = self.graph.block_defs;
        let mut block_table: HashMap<String, BlockRuntime> = HashMap::new();
        let mut entry_strands: Vec<Vec<Instruction>> = Vec::new();

        for strand in self.graph.strands {
            match strand.instructions.first().map(|i| &i.kind) {
                Some(InstructionKind::BlockHeader(id)) => {
                    if let Some(def) = block_defs.iter().find(|b| &b.id == id) {
                        let input_names: Vec<String> =
                            def.input_names().map(str::to_string).collect();
                        let branch_names: Vec<String> =
                            def.branch_names().map(str::to_string).collect();
                        let body = strand.instructions[1..].to_vec();
                        block_table.insert(
                            id.clone(),
                            BlockRuntime {
                                input_names,
                                branch_names,
                                body,
                            },
                        );
                    }
                }
                Some(InstructionKind::WhenRan) => entry_strands.push(strand.instructions),
                // Battery/time-triggered strands aren't run here at all -
                // they're driven by the app's own background watchers (e.g.
                // src-tauri's `battery_watch`/`time_watch` modules), which
                // fire just their body directly once their own condition
                // holds, independent of Run/Loop. A manual Run intentionally
                // leaves them alone.
                Some(InstructionKind::WhenBatteryDischargedTo(_))
                | Some(InstructionKind::WhenBatteryChargedTo(_))
                | Some(InstructionKind::WhenTime(_))
                | Some(InstructionKind::WhenPowerPluggedIn)
                | Some(InstructionKind::WhenPowerUnplugged)
                | Some(InstructionKind::WhenClipboardChanged) => {}
                _ => {}
            }
        }
        let block_table = Arc::new(block_table);

        if entry_strands.is_empty() {
            return;
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
                let spawned = std::thread::Builder::new()
                    .stack_size(STRAND_STACK_SIZE)
                    .spawn_scoped(scope, move || {
                        run_strand(
                            instructions,
                            emulator,
                            stop_flag,
                            speed_multiplier,
                            variables,
                            lists,
                            block_table,
                            initial_offset,
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
fn run_strand(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
    block_table: Arc<HashMap<String, BlockRuntime>>,
    initial_offset: Duration,
) {
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
        };
        let _ = run_block(&instructions, &mut ctx, 0, start);
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
    run_strand(
        instructions,
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        lists,
        Arc::new(HashMap::new()),
        Duration::ZERO,
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
            InstructionKind::WhenTime(_) => {}
            InstructionKind::WhenPowerPluggedIn => {}
            InstructionKind::WhenPowerUnplugged => {}
            InstructionKind::WhenClipboardChanged => {}
            InstructionKind::OpenApp { command, .. } => {
                println!("Opening app: {command}");
                if let Err(e) = open_app(command) {
                    warn!("Failed to open app: {}", e);
                }
            }
            InstructionKind::CloseApp { command, name, .. } => {
                println!("Closing app: {name}");
                if let Err(e) = close_app(command, name) {
                    warn!("Failed to close app: {}", e);
                }
            }
            InstructionKind::Return(value) => {
                let resolved = ctx.resolve(value, depth)?;
                return Ok(Flow::Return(resolved.eval()?));
            }
            InstructionKind::EscapeLoop => return Ok(Flow::Break),
            InstructionKind::ContinueLoop => return Ok(Flow::Continue),
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
                            warn!("Skipping call to '{}': arg {}", block_id, e);
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
                    Err(e) => warn!("Skipping If: condition {}", e),
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
                Err(e) => warn!("Skipping IfElse: condition {}", e),
            },
            InstructionKind::Repeat { count, body } => {
                let n = match ctx.resolve(count, depth).and_then(|v| v.eval_number()) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Skipping Repeat: count {}", e);
                        continue;
                    }
                };
                let n = n.max(0.0).round() as u64;
                for _ in 0..n {
                    if let Some(flag) = &ctx.stop_flag {
                        if stop_requested(flag) {
                            break;
                        }
                    }
                    match run_block(body, ctx, depth, Instant::now())? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
            }
            InstructionKind::Forever { body } => loop {
                if let Some(flag) = &ctx.stop_flag {
                    if stop_requested(flag) {
                        break;
                    }
                }
                match run_block(body, ctx, depth, Instant::now())? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Break => break,
                    flow @ Flow::Return(_) => return Ok(flow),
                }
            },
            InstructionKind::While { condition, body } => loop {
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
                        warn!("Skipping While: condition {}", e);
                        break;
                    }
                }
                match run_block(body, ctx, depth, Instant::now())? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Break => break,
                    flow @ Flow::Return(_) => return Ok(flow),
                }
            },
            InstructionKind::Wait(duration) => {
                let duration = match ctx.resolve(duration, depth).and_then(|v| v.eval_number()) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Skipping Wait: duration {}", e);
                        continue;
                    }
                };
                // Clamped since duration may be an Op::Random node with a
                // negative lower bound; from_secs_f64 panics on negative input.
                let actual = (duration / ctx.speed_multiplier).max(0.0);
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
                println!("Running command: {command}");
                if let Err(e) = Command::new("bash").args(["-c", command]).status() {
                    warn!("Command failed: {}", e)
                }
            }
            InstructionKind::Token(token) => match token {
                InputToken::Text(value) => {
                    let text = match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                        Ok(t) => t,
                        Err(e) => {
                            warn!("Skipping Text: {}", e);
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
                    let mut em = match ctx.emulator.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            warn!("Failed to lock emulator mutex: {}", err);
                            return Ok(Flow::Normal);
                        }
                    };
                    let normalized_key = normalize_modifier_key(key.clone());
                    let key_for_event = match (normalized_key.clone(), direction.clone()) {
                        (MacroKey::Unicode(c), Direction::Click)
                            if shift_is_pressed(ctx.pressed_keys) && c.is_ascii_lowercase() =>
                        {
                            MacroKey::Unicode(c.to_ascii_uppercase())
                        }
                        (key, _) => key,
                    };

                    match em.key(key_for_event, direction.clone()) {
                        Ok(()) => match direction {
                            Direction::Press => {
                                if !ctx.pressed_keys.contains(&normalized_key) {
                                    ctx.pressed_keys.push(normalized_key);
                                }
                            }
                            Direction::Release => {
                                ctx.pressed_keys.retain(|k| k != &normalized_key);
                            }
                            Direction::Click => {}
                        },
                        Err(err) => {
                            warn!(
                                "Failed to press key {:?} ({:?}): {}",
                                normalized_key, direction, err
                            );
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
                            warn!("Skipping mouse move: x {}", e);
                            continue;
                        }
                    };
                    let y = match ctx.resolve(y, depth).and_then(|v| v.eval_number()) {
                        Ok(v) => v.round() as i32,
                        Err(e) => {
                            warn!("Skipping mouse move: y {}", e);
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
                            warn!("Skipping scroll: {}", e);
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
                    Err(e) => warn!("Skipping Set Variable: {}", e),
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
                    Err(e) => warn!("Skipping Change Variable: {}", e),
                }
            }
            InstructionKind::SetClipboard(value) => {
                match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
                    Ok(text) => {
                        if let Err(e) = crate::clipboard::set_text(text) {
                            warn!("Failed to set clipboard: {}", e);
                        }
                    }
                    Err(e) => warn!("Skipping Set Clipboard: {}", e),
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
                        None => warn!("Skipping Add to List: list items must be number or text"),
                    },
                    Err(e) => warn!("Skipping Add to List: {e}"),
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
                    Err(e) => warn!("Skipping Delete List Item: {e}"),
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
                    Err(e) => warn!("Skipping Shift List: {e}"),
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
                            warn!("Skipping Insert into List: list items must be number or text")
                        }
                    },
                    (Err(e), _) | (_, Err(e)) => warn!("Skipping Insert into List: {e}"),
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
                            warn!("Skipping Replace List Item: list items must be number or text")
                        }
                    },
                    (Err(e), _) | (_, Err(e)) => warn!("Skipping Replace List Item: {e}"),
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
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
                        },
                    ],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                }],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
            lists: vec![],
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
                }],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            settings: crate::macros::MacroSettings::default(),
            lists: vec![],
        };
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        assert_eq!(
            vars.lock().unwrap().get("ran"),
            Some(&Evaluated::Number(1.0))
        );
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
                }],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
                }],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
            settings: crate::macros::MacroSettings::default(),
        };
        let start = Instant::now();
        mac.run(noop_emulator(), None, 1.0, empty_vars());
        assert!(
            start.elapsed() >= Duration::from_millis(140),
            "CallBlock's nested Wait didn't actually take real time"
        );
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
                }],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
                        }],
                        shape: BlockShape::ReturnsValue,
                        color: default_block_color(),
                    },
                    BlockDef {
                        id: triple_id,
                        pieces: vec![BlockPiece::Input {
                            id: "p1".into(),
                            name: "n".into(),
                            value_type: InputValueType::Any,
                        }],
                        shape: BlockShape::ReturnsValue,
                        color: default_block_color(),
                    },
                ],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
                }],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
                }],
            },
            recording_target: None,
            speed_multiplier: 1.0,
            lists: vec![],
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
