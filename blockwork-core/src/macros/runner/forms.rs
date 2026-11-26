use super::*;
use crate::prompt::{Control, Kind, Reply, Request};
use serde_json::{Map, Value as Json, json};
use std::collections::BTreeMap;

#[derive(Default)]
struct WindowState {
    title: Option<String>,
    size: Option<(u32, u32)>,
    closing: bool,
    components: BTreeMap<String, ComponentState>,
}

#[derive(Clone, Default)]
struct ComponentState {
    visible: Option<bool>,
    enabled: Option<bool>,
}

type WindowHandle = Arc<Mutex<WindowState>>;

fn windows() -> &'static Mutex<HashMap<String, WindowHandle>> {
    static WINDOWS: OnceLock<Mutex<HashMap<String, WindowHandle>>> = OnceLock::new();
    WINDOWS.get_or_init(Mutex::default)
}

fn lookup(id: &str) -> Option<WindowHandle> {
    windows().lock().unwrap().get(id).cloned()
}

pub(crate) fn is_open(id: &str) -> bool {
    lookup(id).is_some()
}

struct Registration(String);

impl Drop for Registration {
    fn drop(&mut self) {
        windows().lock().unwrap().remove(&self.0);
    }
}

fn register(id: String, form: &Form) -> Result<(Registration, WindowHandle), String> {
    let mut windows = windows().lock().unwrap();
    if id.trim().is_empty() {
        return Err("the window ID is empty".into());
    }
    if windows.contains_key(&id) {
        return Err(format!("window \"{id}\" is already open"));
    }
    let state = WindowState {
        components: form
            .components
            .keys()
            .map(|key| (key.clone(), ComponentState::default()))
            .collect(),
        ..WindowState::default()
    };
    let handle = Arc::new(Mutex::new(state));
    windows.insert(id.clone(), Arc::clone(&handle));
    Ok((Registration(id), handle))
}

#[derive(Clone, Copy)]
enum Input {
    Text,
    Check,
    Slider,
}

impl Input {
    fn json(self, value: Option<&Evaluated>) -> Json {
        match self {
            Self::Text => json!(value.map(Evaluated::as_text).unwrap_or_default()),
            Self::Check => json!(value.is_some_and(Evaluated::as_bool)),
            Self::Slider => json!(
                value
                    .and_then(|value| value.as_number().ok())
                    .filter(|value| value.is_finite())
                    .unwrap_or_default()
                    .clamp(0.0, 100.0)
            ),
        }
    }

    fn read(self, value: &Json) -> Option<Evaluated> {
        match self {
            Self::Text => Some(Evaluated::Text(value.as_str()?.to_owned())),
            Self::Check => Some(Evaluated::Bool(value.as_bool()?)),
            Self::Slider => Some(Evaluated::Number(value.as_f64()?.clamp(0.0, 100.0))),
        }
    }
}

enum Source {
    Variable(String, Input, Option<Evaluated>),
    List(String),
}

struct Binding {
    control: String,
    field: &'static str,
    source: Source,
    initial: Json,
}

struct Button {
    id: String,
    label: String,
    close: bool,
    body: Vec<Instruction>,
}

#[derive(Default)]
struct Form {
    bindings: Vec<Binding>,
    buttons: Vec<Button>,
    inputs: HashMap<String, (String, Input)>,
    changes: HashMap<String, Vec<Vec<Instruction>>>,
    components: HashMap<String, String>,
    has_layout: bool,
    next_control: usize,
}

fn choices(lists: &ListStore, name: &str) -> Vec<String> {
    lists
        .lock()
        .ok()
        .and_then(|lists| {
            lists.get(name).map(|items| {
                items
                    .iter()
                    .map(|item| item.evaluated().as_text())
                    .collect()
            })
        })
        .unwrap_or_default()
}

impl Form {
    fn content(
        &mut self,
        ctx: &mut ExecCtx,
        value: &Value,
        depth: u32,
        id: &str,
        field: &'static str,
    ) -> Result<String, String> {
        // Only a direct variable slot is live. Other reporters can have side effects.
        let initial = ctx.resolve(value, depth)?.eval_text()?;
        if let Value::Var { name } = value {
            self.bindings.push(Binding {
                control: id.into(),
                field,
                source: Source::Variable(name.clone(), Input::Text, Some(Evaluated::Number(0.0))),
                initial: json!(initial),
            });
        }
        Ok(initial)
    }

    /// Gives a part the ID blocks can address it by while the window is open.
    fn component(&mut self, key: &str, id: &str) -> Result<(), String> {
        if key.is_empty() {
            return Ok(());
        }
        if self
            .components
            .insert(key.to_string(), id.to_string())
            .is_some()
        {
            return Err(format!("duplicate component ID \"{key}\""));
        }
        Ok(())
    }

    fn input(&mut self, ctx: &ExecCtx, control: &mut Control, name: &str, input: Input) {
        let vars = ctx.variables.lock().unwrap();
        control.value = input.json(vars.get(name));
        if !name.is_empty() {
            control.name = name.into();
            self.inputs.insert(control.id.clone(), (name.into(), input));
            self.bindings.push(Binding {
                control: control.id.clone(),
                field: "value",
                source: Source::Variable(name.into(), input, None),
                initial: control.value.clone(),
            });
        }
    }

    fn build(
        &mut self,
        ctx: &mut ExecCtx,
        body: &[Instruction],
        depth: u32,
        nesting: u32,
    ) -> Result<Vec<Control>, String> {
        if nesting >= MAX_CALL_DEPTH {
            return Err("window layouts are nested too deeply".into());
        }
        let mut controls = Vec::new();
        for ins in body {
            self.next_control += 1;
            let id = format!("c{}", self.next_control);
            let mut control = Control {
                id: id.clone(),
                ..Control::default()
            };
            let mut label = |this: &mut Self, value| this.content(ctx, value, depth, &id, "label");
            control.kind = match &ins.kind {
                InstructionKind::WindowNote(text) => {
                    control.label = label(self, text)?;
                    "note"
                }
                InstructionKind::WindowButton {
                    label: text,
                    close,
                    body,
                    key,
                    size,
                    icon,
                    variant,
                } => {
                    control.label = label(self, text)?;
                    control.close = *close;
                    control.size = self.content(ctx, size, depth, &id, "size")?;
                    control.icon = self.content(ctx, icon, depth, &id, "icon")?;
                    control.variant = variant.clone();
                    self.component(key, &id)?;
                    self.buttons.push(Button {
                        id,
                        label: control.label.clone(),
                        close: *close,
                        body: body.clone(),
                    });
                    "button"
                }
                InstructionKind::WindowTextBox {
                    label: text,
                    name,
                    key,
                    size,
                } => {
                    control.label = label(self, text)?;
                    control.size = self.content(ctx, size, depth, &id, "size")?;
                    self.component(key, &id)?;
                    self.input(ctx, &mut control, name, Input::Text);
                    "text"
                }
                InstructionKind::WindowCheckbox {
                    label: text,
                    name,
                    key,
                    size,
                } => {
                    control.label = label(self, text)?;
                    control.size = self.content(ctx, size, depth, &id, "size")?;
                    self.component(key, &id)?;
                    self.input(ctx, &mut control, name, Input::Check);
                    "check"
                }
                InstructionKind::WindowChoice {
                    label: text,
                    list,
                    name,
                    key,
                    size,
                } => {
                    control.label = label(self, text)?;
                    control.size = self.content(ctx, size, depth, &id, "size")?;
                    self.component(key, &id)?;
                    self.input(ctx, &mut control, name, Input::Text);
                    control.options = choices(&ctx.lists, list);
                    self.bindings.push(Binding {
                        control: id,
                        field: "options",
                        source: Source::List(list.clone()),
                        initial: json!(control.options),
                    });
                    "choice"
                }
                InstructionKind::WindowLayout {
                    layout,
                    label: text,
                    key,
                    columns,
                    body,
                    size,
                    icon,
                    style,
                } => {
                    if !matches!(
                        layout.as_str(),
                        "row"
                            | "column"
                            | "grid"
                            | "scroll"
                            | "group"
                            | "spacer"
                            | "tabs"
                            | "split"
                    ) {
                        return Err(format!("unknown window layout \"{layout}\""));
                    }
                    self.has_layout = true;
                    control.label = label(self, text)?;
                    control.size = self.content(ctx, size, depth, &id, "size")?;
                    control.icon = self.content(ctx, icon, depth, &id, "icon")?;
                    control.style = style.clone();
                    self.component(key, &id)?;
                    if layout == "grid" {
                        control.columns = Some(
                            ctx.resolve(columns, depth)?.eval_number()?.clamp(1.0, 12.0) as u32,
                        );
                    }
                    control.children = self.build(ctx, body, depth, nesting + 1)?;
                    layout
                }
                InstructionKind::WindowDisplay {
                    display,
                    label: text,
                    data,
                    key,
                    size,
                    tone,
                    icon,
                } => {
                    if !matches!(
                        display.as_str(),
                        "heading"
                            | "subheading"
                            | "body"
                            | "caption"
                            | "divider"
                            | "icon"
                            | "table"
                            | "progress"
                            | "chart"
                            | "image"
                    ) {
                        return Err(format!("unknown window display \"{display}\""));
                    }
                    control.label = label(self, text)?;
                    control.size = self.content(ctx, size, depth, &id, "size")?;
                    control.icon = self.content(ctx, icon, depth, &id, "icon")?;
                    control.tone = tone.clone();
                    self.component(key, &id)?;
                    if display != "heading" && display != "divider" {
                        control.value = json!(self.content(ctx, data, depth, &id, "value")?);
                    }
                    display
                }
                InstructionKind::WindowInput {
                    input,
                    label: text,
                    name,
                    options,
                    key,
                    size,
                    icon,
                } => {
                    let kind = match input.as_str() {
                        "slider" => Input::Slider,
                        "multiline" | "list" | "table" | "file" | "folder" => Input::Text,
                        _ => return Err(format!("unknown window input \"{input}\"")),
                    };
                    control.label = label(self, text)?;
                    control.size = self.content(ctx, size, depth, &id, "size")?;
                    control.icon = self.content(ctx, icon, depth, &id, "icon")?;
                    self.component(key, &id)?;
                    self.input(ctx, &mut control, name, kind);
                    if input == "list" || input == "table" {
                        control.data = self.content(ctx, options, depth, &id, "data")?;
                    }
                    input
                }
                InstructionKind::WindowOnChange { name, body } => {
                    self.changes
                        .entry(name.clone())
                        .or_default()
                        .push(body.clone());
                    continue;
                }
                _ => return Err("only window parts work inside a window".into()),
            }
            .into();
            controls.push(control);
        }
        Ok(controls)
    }

    fn button(&self, reply: &Reply, close: bool) -> Option<&Button> {
        self.buttons
            .iter()
            .find(|button| button.id == reply.text)
            .or_else(|| {
                self.buttons
                    .iter()
                    .find(|button| button.label == reply.text)
            })
            .filter(|button| button.close == close)
    }

    fn save(&self, variables: &VariableStore, reply: &Reply) -> Vec<String> {
        let mut vars = variables.lock().unwrap();
        let mut changed = Vec::new();
        for (name, input) in self.inputs.values() {
            let Some(value) = reply.values.get(name).and_then(|v| input.read(v)) else {
                continue;
            };
            if vars.get(name) != Some(&value) {
                vars.insert(name.clone(), value);
                changed.push(name.clone());
            }
        }
        changed
    }

    fn updates(
        &self,
        spawner: &Spawner,
        state: &WindowHandle,
        previous: &Mutex<Json>,
        acknowledged: &Mutex<String>,
    ) -> Option<Json> {
        let mut controls = BTreeMap::<&str, Map<String, Json>>::new();
        {
            let vars = spawner.variables.lock().unwrap();
            for binding in &self.bindings {
                let value = match &binding.source {
                    Source::Variable(name, input, default) => {
                        input.json(vars.get(name).or(default.as_ref()))
                    }
                    Source::List(name) => json!(choices(&spawner.lists, name)),
                };
                controls
                    .entry(&binding.control)
                    .or_default()
                    .insert(binding.field.into(), value);
            }
        }
        let state = state.lock().unwrap();
        for (key, settings) in &state.components {
            let properties = controls.entry(&self.components[key]).or_default();
            if let Some(visible) = settings.visible {
                properties.insert("visible".into(), visible.into());
            }
            if let Some(enabled) = settings.enabled {
                properties.insert("enabled".into(), enabled.into());
            }
        }
        let mut next = json!({
            "controls": controls,
            "title": state.title,
            "size": state.size,
            "close": state.closing,
            "ack": *acknowledged.lock().unwrap(),
        });
        let mut previous = previous.lock().unwrap();
        let mut delta = next.as_object().unwrap().clone();
        delta.remove("controls");
        delta.retain(|key, value| previous.get(key) != Some(value) && !value.is_null());
        let mut changed = Map::new();
        for (id, fields) in next["controls"].as_object_mut().unwrap() {
            let mut fields = fields.as_object().unwrap().clone();
            fields.retain(|field, value| previous["controls"][id].get(field) != Some(value));
            if !fields.is_empty() {
                changed.insert(id.clone(), Json::Object(fields));
            }
        }
        if !changed.is_empty() {
            delta.insert("controls".into(), Json::Object(changed));
        }
        *previous = next;
        (!delta.is_empty()).then_some(Json::Object(delta))
    }
}

struct FinishedWindow {
    closing_body: Result<Option<Vec<Instruction>>, String>,
    callbacks: Vec<std::thread::JoinHandle<()>>,
}

fn run_window(
    form: Form,
    request: Request,
    spawner: &Spawner,
    state: WindowHandle,
    flag: Option<StrandStop>,
) -> FinishedWindow {
    let threads = Mutex::new(Vec::new());
    let acknowledged = Mutex::new(String::new());
    let event = |reply: &Reply| {
        let changed = form.save(&spawner.variables, reply);
        let mut threads = threads.lock().unwrap();
        threads.retain(|thread: &std::thread::JoinHandle<()>| !thread.is_finished());
        if reply.answer == "change" {
            *acknowledged.lock().unwrap() = reply.text.clone();
            for name in changed {
                for body in form.changes.get(&name).into_iter().flatten() {
                    threads.extend(spawner.spawn("window_change".into(), body.clone()));
                }
            }
        } else if let Some(button) = form.button(reply, false) {
            threads.extend(spawner.spawn("window_button".into(), button.body.clone()));
        }
    };
    let stopped = || flag.as_ref().is_some_and(stop_requested);
    let mut initial = json!({"controls": {}, "close": false, "ack": ""});
    for binding in &form.bindings {
        initial["controls"][&binding.control][binding.field] = binding.initial.clone();
    }
    let previous = Mutex::new(initial);
    let updates = || form.updates(spawner, &state, &previous, &acknowledged);
    let closing_body = match crate::prompt::show_live_window(&request, &stopped, &event, &updates) {
        Ok(reply) => {
            form.save(&spawner.variables, &reply);
            Ok((reply.answer == "ok")
                .then(|| form.button(&reply, true).map(|button| button.body.clone()))
                .flatten())
        }
        Err(_) if stopped() => Ok(None),
        Err(error) => Err(error),
    };
    FinishedWindow {
        closing_body,
        callbacks: threads.into_inner().unwrap(),
    }
}

fn dimensions(
    ctx: &mut ExecCtx,
    width: &Value,
    height: &Value,
    depth: u32,
) -> Result<(u32, u32), String> {
    let width = ctx.resolve(width, depth)?.eval_number()?;
    let height = ctx.resolve(height, depth)?.eval_number()?;
    if !width.is_finite() || !height.is_finite() {
        return Err("window dimensions must be finite numbers".into());
    }
    Ok((
        width.clamp(320.0, 4096.0) as u32,
        height.clamp(120.0, 4096.0) as u32,
    ))
}

fn build_window(
    ctx: &mut ExecCtx,
    title: &Value,
    body: &[Instruction],
    depth: u32,
    inline_buttons: bool,
) -> Result<(Form, Request), String> {
    let title = ctx.resolve(title, depth)?.eval_text()?;
    let mut request = Request::new(Kind::Window, &title, "");
    let mut form = Form::default();
    request.live = true;
    request.controls = form.build(ctx, body, depth, 0)?;
    request.inline_buttons = inline_buttons || form.has_layout;
    Ok((form, request))
}

fn run(
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Option<Vec<Instruction>>, String> {
    let window_id = |ctx: &mut ExecCtx, value| ctx.resolve(value, depth)?.eval_text();
    let handle = |ctx: &mut ExecCtx, value| {
        let id = window_id(ctx, value)?;
        lookup(&id).ok_or_else(|| format!("window \"{id}\" is not open"))
    };
    match &ins.kind {
        InstructionKind::ShowWindow { title, body } => {
            let (form, request) = build_window(ctx, title, body, depth, false)?;
            let finished = run_window(
                form,
                request,
                &Spawner::of(ctx),
                WindowHandle::default(),
                ctx.stop_flag.clone(),
            );
            ctx.signal_threads.extend(finished.callbacks);
            return finished.closing_body;
        }
        InstructionKind::OpenFormWindow {
            window,
            title,
            width,
            height,
            body,
        } => {
            let id = window_id(ctx, window)?;
            let (width, height) = dimensions(ctx, width, height, depth)?;
            let (form, mut request) = build_window(ctx, title, body, depth, true)?;
            request.width = Some(width);
            request.height = Some(height);
            let (registration, state) = register(id, &form)?;
            let spawner = Spawner::of(ctx);
            let flag = ctx.stop_flag.as_ref().map(StrandStop::fork);
            let block = ins.id.clone();
            // Keep the run cancellable while its named window is open.
            let thread = std::thread::Builder::new()
                .name("form_window".into())
                .spawn(move || {
                    let finished = run_window(form, request, &spawner, state, flag);
                    drop(registration);
                    let closing = match finished.closing_body {
                        Ok(body) => {
                            body.and_then(|body| spawner.spawn("window_closed".into(), body))
                        }
                        Err(error) => {
                            crate::run_log::block_error(&spawner.source, &block, error);
                            None
                        }
                    };
                    for thread in finished.callbacks.into_iter().chain(closing) {
                        let _ = thread.join();
                    }
                })
                .map_err(|error| error.to_string())?;
            ctx.signal_threads.push(thread);
        }
        InstructionKind::CloseFormWindow(window) => {
            if let Some(window) = lookup(&window_id(ctx, window)?) {
                window.lock().unwrap().closing = true;
            }
        }
        InstructionKind::WaitForFormWindow(window) => {
            let id = window_id(ctx, window)?;
            if let Some(original) = lookup(&id) {
                while lookup(&id).is_some_and(|current| Arc::ptr_eq(&current, &original))
                    && !ctx.stop_flag.as_ref().is_some_and(stop_requested)
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
        }
        InstructionKind::SetFormWindowTitle { window, title } => {
            let window = handle(ctx, window)?;
            let title = ctx.resolve(title, depth)?.eval_text()?;
            window.lock().unwrap().title = Some(title);
        }
        InstructionKind::ResizeFormWindow {
            window,
            width,
            height,
        } => {
            let window = handle(ctx, window)?;
            let size = dimensions(ctx, width, height, depth)?;
            window.lock().unwrap().size = Some(size);
        }
        InstructionKind::SetFormComponent {
            window,
            component,
            property,
            state,
        } => {
            let window = handle(ctx, window)?;
            let id = ctx.resolve(component, depth)?.eval_text()?;
            let value = ctx.resolve(state, depth)?.eval()?.as_bool();
            let mut window = window.lock().unwrap();
            let component = window
                .components
                .get_mut(&id)
                .ok_or_else(|| format!("unknown component ID \"{id}\""))?;
            match property.as_str() {
                "visible" => component.visible = Some(value),
                "enabled" => component.enabled = Some(value),
                _ => return Err(format!("unknown component property \"{property}\"")),
            }
        }
        _ => unreachable!("run_form_block got another block"),
    }
    Ok(None)
}

pub(super) fn run_form_block(
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Option<Flow>, String> {
    match run(ins, ctx, depth) {
        Ok(Some(body)) => match run_block(&body, ctx, depth)? {
            Flow::Normal => {}
            flow => return Ok(Some(flow)),
        },
        Ok(None) => {}
        Err(error) => soft_fail(ctx, &ins.id, "Macro window", error)?,
    }
    Ok(None)
}
