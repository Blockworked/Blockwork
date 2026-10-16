import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke
    required property var appBridge
    // Shared import/export flow (MacroFiles in Main.qml).
    property var macroFiles: null
    readonly property var macro: appState.current_macro
    readonly property bool recording: appState.recording_phase && appState.recording_phase.phase === "Active"
    property string sidebarKey: "a"
    property real sidebarWidth: 348
    property bool sidebarCollapsed: false
    property bool sidebarToggling: false
    readonly property real minSidebarWidth: 180
    readonly property real maxSidebarWidth: 600
    readonly property var instructionTypes: ["WhenRan", "WhenBatteryDischargedTo", "WhenBatteryChargedTo", "WhenTime", "WhenPowerPluggedIn", "WhenPowerUnplugged", "WhenClipboardChanged", "WhenKey", "WhenWindow", "WhenFileChanged", "WhenTyped", "WhenApp", "WhenIdle", "WhenScreenLock", "WhenCron", "WhenNetwork", "WhenUsb", "WhenMouseButton", "WhenAt", "WhenSignal", "WhenWebhook", "Wait", "Text", "PasteText", "Key", "Button", "MoveMouse", "Scroll", "Command", "OpenApp", "CloseApp", "WindowAction", "MoveWindow", "WriteFile", "HttpRequest", "Notify", "AskText", "AskYesNo", "AskChoice", "ShowMessage", "Log", "SetVariable", "ChangeVariable", "SetClipboard", "Return", "If", "IfElse", "Repeat", "Forever", "While", "EscapeLoop", "ContinueLoop", "TryCatch", "Retry", "ThrowError", "StopMacro", "WithLock", "SendSignal", "RunMacro", "StopMacroNamed", "SetGlobal", "ChangeGlobal", "WaitUntil", "FindImage", "ClickImage", "WaitForImage", "DetectObjectsInWindow", "WaitForDetection", "AimStep", "AimAtDetection"]
    readonly property var listInstructionTypes: ["AddToList", "DeleteOfList", "DeleteAllOfList", "ShiftList", "InsertIntoList", "ReplaceItemOfList", "ReverseList", "SplitIntoList"]

    function uuid() { return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, c => { const r = Math.random() * 16 | 0; return (c === "x" ? r : (r & 3 | 8)).toString(16); }); }
    function numberValue(n) { return { kind: "Number", value: n }; }
    function textValue(t) { return { kind: "Text", value: t }; }
    function defaultInstruction(type) {
        const registered = BlockRegistry.prefab(type);
        if (registered) { registered.id = uuid(); return registered; }
        const i = { id: uuid(), type: type };
        if (type === "WhenBatteryDischargedTo") i.threshold = numberValue(20);
        else if (type === "WhenBatteryChargedTo") i.threshold = numberValue(100);
        else if (type === "WhenTime") i.schedule = { kind: "Daily", hour: 9, minute: 0 };
        else if (type === "Wait") i.duration = numberValue(1000);
        else if (type === "Text") i.text = textValue("text");
        else if (type === "Key") { i.key = textValue(root.sidebarKey); i.direction = "Click"; }
        else if (type === "WhenKey") { i.key = textValue(root.sidebarKey); i.direction = "Press"; }
        else if (type === "Button") { i.button = "Left"; i.direction = "Click"; }
        else if (type === "MoveMouse") { i.x = numberValue(0); i.y = numberValue(0); i.coordinate = "Relative"; }
        else if (type === "Scroll") { i.amount = numberValue(4); i.axis = "Vertical"; }
        else if (type === "Command") i.command = "";
        else if (type === "OpenApp" || type === "CloseApp") { i.command = ""; i.name = ""; i.icon = null; }
        else if (type === "SetClipboard") i.value = textValue("text");
        else if (type === "AddToList") { i.name = firstListName(); i.value = textValue("thing"); }
        else if (type === "DeleteOfList") { i.name = firstListName(); i.index = numberValue(1); }
        else if (type === "DeleteAllOfList" || type === "ReverseList") i.name = firstListName();
        else if (type === "ShiftList") { i.name = firstListName(); i.amount = numberValue(1); }
        else if (type === "InsertIntoList") { i.name = firstListName(); i.value = textValue("thing"); i.index = numberValue(1); }
        else if (type === "ReplaceItemOfList") { i.name = firstListName(); i.index = numberValue(1); i.value = textValue("thing"); }
        else if (type === "If") { i.condition = { kind: "Bool" }; i.body = []; }
        else if (type === "IfElse") { i.condition = { kind: "Bool" }; i.then_body = []; i.else_body = []; }
        else if (type === "Repeat") { i.count = numberValue(10); i.body = []; }
        else if (type === "Forever") i.body = [];
        else if (type === "While") { i.condition = { kind: "Bool" }; i.body = []; }
        else if (type === "SetVariable") { i.name = root.macro && root.macro.variables.length ? root.macro.variables[0] : "variable"; i.value = numberValue(0); }
        else if (type === "ChangeVariable") { i.name = root.macro && root.macro.variables.length ? root.macro.variables[0] : "variable"; i.value = numberValue(1); }
        else if (type === "Return") i.value = numberValue(0);
        return i;
    }
    function firstListName() { return root.macro && root.macro.lists && root.macro.lists.length ? root.macro.lists[0].name : ""; }
    function addBlock(type, x, y) { root.invoke("add_strand", { x: x, y: y, instruction: defaultInstruction(type) }); }
    function addCustomBlock(definition, x, y) {
        const args = (definition.pieces || []).filter(piece => piece.kind === "Input")
            .map(piece => piece.value_type === "Bool" ? { kind: "Bool" } : numberValue(0));
        const branchCount = (definition.pieces || []).filter(piece => piece.kind === "Branch").length;
        const instruction = { id: uuid(), type: branchCount ? "BranchCallBlock" : "CallBlock", block_id: definition.id, args: args };
        if (branchCount) instruction.branches = Array.from({ length: branchCount }, () => []);
        root.invoke("add_strand", { x: x, y: y, instruction: instruction });
    }
    function recordingLabel() {
        const phase = appState.recording_phase || { phase: "Idle" };
        if (phase.phase === "Countdown") return "Recording in " + phase.countdown + "s…";
        if (phase.phase === "Active") return "Stop recording";
        return "Record";
    }
    function refreshIds(instruction) {
        const copy = JSON.parse(JSON.stringify(instruction)); copy.id = uuid();
        ["body","then_body","else_body"].forEach(k => { if (copy[k]) copy[k] = copy[k].map(refreshIds); });
        if (copy.branches) copy.branches = copy.branches.map(branch => branch.map(refreshIds));
        return copy;
    }
    // In-memory copy/paste buffer (not the OS clipboard).
    property var clipboard: []
    function totalBlocks() { return (root.macro ? root.macro.strands : []).reduce((n, st) => n + st.instructions.length, 0); }
    function nextPath(path) { const p=JSON.parse(JSON.stringify(path));p[p.length-1].index++;return p; }
    function instructionExplainer(type) {
        const x={WhenAt:"Runs once, at a date and time like 2026-12-24 18:00 (local time). Turn on 'catch up on missed schedules' in Macro settings to still run it if Blockwork was closed or the computer was asleep at that moment.",WhenSignal:"Runs when a 'send signal' block in this macro sends the same name. It shares that run's variables, so one strand can hand work to others.",SendSignal:"Starts this macro's 'when I receive' strands with the same name. With 'and wait' this strand pauses until they've all finished: a way to wait for other strands.",WhenWebhook:"Runs when a web request reaches Blockwork's webhook server at this path. Turn the server on in Settings > Integrations, where its address and token are. The request body goes into the variable.",AskText:"Shows a dialog asking for text and puts the answer in the variable. Cancel stops the run, unless it's inside a try block, where it's an error you can catch.",AskYesNo:"Shows a yes/no question and puts true or false in the variable.",AskChoice:"Shows the items of a list and puts the one you pick in the variable. Cancel stops the run, unless it's inside a try block.",ShowMessage:"Shows a message in a dialog and waits until you close it.",WhenNetwork:"Runs when the computer goes online or offline: when a network route to the internet appears or goes away. Checked once a second.",WhenUsb:"Runs when a USB device is plugged in or unplugged. The text matches the device's name or its vendor:product id (like 046d:c52b); leave it empty for any device. Hubs don't count.",WhenMouseButton:"Runs when a mouse button is pressed or released, or the wheel scrolls. Put modifier keys in the box (like Control+Shift) to only run while exactly those are held; leave it empty to ignore modifiers. The click still goes through to the app.",SplitIntoList:"Cuts the text at every separator and puts the pieces in the list, replacing what was there. An empty separator splits it into letters.",Secret:"The value of a secret saved in Settings > Secrets. It comes from your system keyring, isn't stored in the macro file, and shows as dots in the run log.",NetworkConnected:"Whether the computer has a network route to the internet.",UsbConnected:"Whether a USB device whose name or vendor:product id contains the text is plugged in. Empty matches any device.",TextCheck:"Checks text: contains, starts with, ends with, or matches a regular expression. Case matters.",TextReplace:"Replaces every copy of the first text inside the second with the third.",RegexReplace:"Replaces every match of a regular expression. Use $1, $2 in the replacement for groups.",RegexFind:"The first match of a regular expression, or its first group if it has one. Empty when nothing matches.",SplitPart:"Cuts the text at every separator and gives one piece, counting from 1. Negative numbers count from the end.",SplitCount:"How many pieces the text has when cut at every separator.",Substring:"The letters between two positions, counting from 1, both included.",Trim:"The text without spaces or line breaks at its start and end.",FormatNumber:"The number as text with exactly this many digits after the point.",Timestamp:"The current time as a Unix timestamp: seconds since 1970. Add 86400 for a day later.",FormatDate:"Writes a timestamp as a date in local time. Codes: %Y year, %m month, %d day, %H hour, %M minute, %S second, %A weekday name, %B month name.",ParseDate:"The timestamp of a date written as text, in local time. Leave the format empty for dates like 2026-01-31 09:00, or use codes like %d.%m.%Y.",DatePart:"One part of a timestamp in local time. Weekday is 1 for Sunday to 7 for Saturday.",WhenTyped:"Text expansion: runs when you type the text anywhere, like ;addr. With 'erase it' the typed text is deleted first, so a paste block can replace it. Uses a US keyboard layout for symbols.",PasteText:"Pastes text with Ctrl+V (Cmd+V on macOS), then puts back the text that was on the clipboard. Faster and more reliable than typing for long text. Terminals that paste with Ctrl+Shift+V need a Key block instead.",WhenApp:"Runs when a program whose name contains the text starts or exits. Checked once a second.",WhenIdle:"Runs once you haven't touched the keyboard or mouse for this many minutes. Fires again after the next idle stretch.",WhenScreenLock:"Runs when the screen locks or unlocks.",WhenCron:"Runs on a cron schedule: minute hour day month weekday. Examples: '0 9 * * mon-fri' (9:00 on weekdays), '*/15 * * * *' (every 15 minutes), '@hourly'.",WithLock:"Runs its body while holding a named lock. Only one strand, in any macro, can be inside a lock with the same name at a time; others wait their turn. Use it so two macros don't type over each other.",AppRunning:"Whether a program whose name contains the text is running.",IdleSeconds:"How many seconds since the last keyboard or mouse input.",ScreenLocked:"Whether the screen is locked.",WaitUntil:"Pauses until the condition is true, checking about 10 times a second. If it's still false after the time limit it's an error; 0 waits forever.",FindImage:"Looks for the picture on screen and puts the screen position of its middle in the two variables, or -1 if it isn't there. Click the picture area to capture one. Tolerance is how different the pixels may be, in percent.",ClickImage:"Finds the picture on screen and clicks its middle. It's an error if the picture isn't there. Click the picture area to capture one.",WaitForImage:"Pauses until the picture appears on screen (or goes away). If that hasn't happened within the time limit it's an error; 0 waits forever.",DetectObjectsInWindow:"Captures the window and finds the objects in it, with a built-in model (the name of a .onnx file in Blockwork's models folder, or a path to one) or a detector running on this computer (a local http:// address). Each one becomes a list item, ordered best, closest to the window's center, or largest first; read them with 'detection of'. Objects keep their id from one run of this block to the next. Click the region button to search only part of the window; the aim still uses the window's center. Only use it where automation is allowed, never against other players.",WaitForDetection:"Pauses until the detector sees the label in the window (or stops seeing it). If that hasn't happened within the time limit it's an error; 0 waits forever. Only use it where automation is allowed, never against other players.",AimStep:"Asks once per session before it moves the mouse. Moves the mouse one small step from the window's center toward a screen point. Games that lock the pointer turn the view instead, so repeat it with a fresh detection each time. Gain is mouse movement per pixel off target: tune it to your sensitivity. Only use it where automation is allowed, never against other players.",AimAtDetection:"Asks once per session before it moves the mouse. Follows one item of a detections list: detects again, steps toward it, and repeats until it's held at the window's center for the hold time. Puts hit, timeout or lost in the variable. Fill the list with a detect block first. Only use it where automation is allowed, never against other players.",WindowInfo:"The position or size of a window, or its center (where games draw the crosshair), in screen coordinates.",Detection:"One part of a detection from a detect block's list: its box, center, score (0-100), label or tracking id. An item that isn't a detection is an error.",PixelColor:"The color of the screen at a point, as #rrggbb. Use the screen picker in the toolbar to find coordinates.",AverageColor:"The average color of a screen region, as #rrggbb.",ColorsMatch:"Whether two colors are within the given percentage of each other (0 = identical, 100 = black vs white).",ScreenText:"Reads the text in a screen region (OCR). Uses Windows or macOS text recognition, or Tesseract on Linux.",MousePosition:"Where the mouse pointer is, in screen coordinates.",ScreenSize:"The size of the whole desktop, across all monitors.",MonitorInfo:"The position or size of a monitor, counting from 1.",MonitorCount:"How many monitors are connected.",RunMacro:"Starts another macro by name. With 'and wait' this strand pauses until that run ends; stopping this run stops it too.",StopMacroNamed:"Stops every run of the named macro.",SetGlobal:"Sets a global variable. Globals are shared by every macro and kept after Blockwork restarts.",ChangeGlobal:"Adds a number to a global variable. A global that was never set counts as 0.",Retry:"Runs its body. If a block in it fails, waits and runs the whole body again, up to the given number of attempts. After the last failure it's an error, like any other.",WindowAction:"Focuses, minimizes, maximizes, restores or closes the first window whose title (window) or app name (app) contains the text. Leave the text empty for the active window.",MoveWindow:"Moves a window to a screen position, or resizes it, in pixels. Picks the first window whose title (window) or app name (app) contains the text; leave it empty for the active window. On Hyprland and Sway the window becomes floating.",WhenWindow:"Runs when a window whose title or app name contains the text gains or loses focus. Leave the text empty to match any window.",WhenFileChanged:"Runs when the file is created, changed or deleted. Checked once a second.",WriteFile:"Writes text to a file, replacing it, or appends to the end. ~ means your home folder.",HttpRequest:"Sends a web request and stores the response in a variable. Headers are one 'Name: value' per line. A 4xx or 5xx response is an error.",Notify:"Shows a desktop notification.",Log:"Adds a line to the run log.",TryCatch:"Runs the first part. If a block in it fails, stops there and runs the 'on error' part, where 'error message' says what went wrong.",ThrowError:"Fails with this message. Ends the strand unless a try block catches it.",StopMacro:"Stops every strand of this run.",WhenRan:"Runs this strand when the macro starts.",Key:"Presses, releases, or clicks a key or key combo.",WhenKey:"Runs when a key or key combo is pressed or released.",Wait:"Pauses this strand for the given milliseconds.",Text:"Types the given text.",SetVariable:"Sets a variable to a value.",ChangeVariable:"Adds a number to a variable.",SetClipboard:"Replaces the clipboard text.",If:"Runs its body when the condition is true.",IfElse:"Chooses one of two branches.",Repeat:"Runs its body a fixed number of times.",Forever:"Repeats until stopped.",While:"Repeats while its condition is true."};
        return x[type]||"A Blockwork instruction block.";
    }
    // ---- daemon commands whose result QML needs ----
    property int callSeq: 0
    property var callbacks: ({})
    function daemonCall(command, args, done) {
        const key = "call" + (++callSeq);
        callbacks[key] = done;
        root.appBridge.callCommand(key, command, JSON.stringify(args || {}));
    }
    Connections {
        target: root.appBridge
        function onReplySerialChanged() {
            const key = root.appBridge.replyKey, done = root.callbacks[key];
            if (!done) return;
            delete root.callbacks[key];
            const error = root.appBridge.replyError;
            done(error ? null : JSON.parse(root.appBridge.replyJson || "null"), error);
        }
    }

    // ---- screen picker: images for image blocks, points/regions for reporters ----
    // { kind: "image" | "region", strandId, path, instruction } or { kind: "tool" }
    property var pickRequest: null
    property int savedVisibility: Window.Windowed
    // Gives the compositor a moment to take a window off screen.
    function afterUnmap(action) { unmapDelay.action = action; unmapDelay.restart(); }
    Timer { id: unmapDelay; interval: 300; property var action: null; onTriggered: if (action) action() }
    function startScreenPick(request) {
        const win = Window.window;
        if (pickRequest || !win) return;
        pickRequest = request;
        // Out of the way, so whatever is behind the editor can be picked.
        savedVisibility = win.visibility;
        win.hide();
        const hint = { image: "Drag around the image to look for", region: "Drag out the part of the window to search" }[request.kind];
        root.afterUnmap(() => screenOverlay.begin(!!hint, hint || "Click a point or drag out a region"));
    }
    function restoreWindow() {
        const win = Window.window;
        if (!win) return;
        // show*() rather than assigning `visibility`, which fights the window's `visible` binding.
        if (savedVisibility === Window.Maximized) win.showMaximized();
        else if (savedVisibility === Window.FullScreen) win.showFullScreen();
        else win.showNormal();
        win.raise(); win.requestActivate();
    }
    ScreenOverlay {
        id: screenOverlay
        onCanceled: { root.pickRequest = null; root.restoreWindow(); }
        onPicked: (x, y, width, height) => {
            const request = root.pickRequest;
            root.pickRequest = null;
            if (request && request.kind === "region") { root.setBlockRegion(request, x, y, width, height); return; }
            // Capture once the overlay itself is gone.
            root.afterUnmap(() => root.daemonCall("screen_grab", { x: x, y: y, width: width, height: height }, (result, error) => {
                root.restoreWindow();
                if (!result) { pickError.show(error); return; }
                if (request && request.kind === "image") root.setBlockImage(request, result.image);
                else pickResult.show(result);
            }));
        }
    }
    function setBlockImage(request, image) {
        const updated = JSON.parse(JSON.stringify(request.instruction || {}));
        updated.image = image;
        root.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
    }
    // A detect block's region is stored relative to its window, so look the window up.
    function setBlockRegion(request, x, y, width, height) {
        root.restoreWindow();
        const ins = request.instruction || {};
        const pattern = ins.pattern && ins.pattern.kind === "Text" ? ins.pattern.value : null;
        if (pattern === null) { pickError.show("Type the window's name into the block to pick a region of it."); return; }
        root.daemonCall("window_bounds", { pattern: pattern, by: ins.by || "Title" }, (bounds, error) => {
            if (!bounds) { pickError.show(error); return; }
            const r = v => Math.round(v);
            const updated = JSON.parse(JSON.stringify(ins));
            updated.region = [r(x - bounds.x), r(y - bounds.y), r(width), r(height)].join(",");
            root.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
        });
    }
    function onPieceAction(action, strandId, path, instruction) {
        const request = { kind: "image", strandId: strandId, path: path, instruction: instruction };
        if (action === "captureImage") root.startScreenPick(request);
        else if (action === "loadImage") imageFileDialog.openFor(request);
        else if (action === "pickRegion") root.startScreenPick(Object.assign(request, { kind: "region" }));
        else if (action === "clearRegion") {
            const updated = JSON.parse(JSON.stringify(instruction || {}));
            updated.region = "";
            root.invoke("edit_instruction", { strandId: strandId, path: path, instruction: updated });
        }
    }
    // Canvas coordinates near the top-left of what's on screen, for blocks added from a dialog.
    function visibleCanvasPoint() {
        const scene = canvas.mapToItem(null, 60, 60);
        const at = canvas.workspacePoint(scene.x, scene.y);
        return at ? { x: Math.round(at.x), y: Math.round(at.y) } : { x: 120, y: 120 };
    }
    // Adds a reporter or block made from a pick to the canvas.
    function addPicked(kind, pick) {
        const n = v => numberValue(v), op = (name, args) => ({ kind: "Op", op: name, args: args });
        const region = [n(pick.x), n(pick.y), n(pick.width), n(pick.height)];
        const at = visibleCanvasPoint();
        if (kind === "PixelColor") root.invoke("create_floating_value", { x: at.x, y: at.y, value: op("PixelColor", [n(pick.x), n(pick.y)]), originBlockId: null });
        else if (kind === "AverageColor" || kind === "ScreenText") root.invoke("create_floating_value", { x: at.x, y: at.y, value: op(kind, region), originBlockId: null });
        else {
            const block = defaultInstruction(kind);
            if (kind === "MoveMouse") { block.x = n(pick.x); block.y = n(pick.y); block.coordinate = "Absolute"; }
            else block.image = pick.image;
            root.invoke("add_strand", { x: at.x, y: at.y, instruction: block });
        }
    }

    // ---- app picker ("Choose app" for OpenApp/CloseApp) ----
    property var appPickerRequest: null   // { strandId, path, instruction } for canvas, or { paletteType } for palette
    function openAppPicker(strandId, path, instruction) {
        appPickerRequest = { strandId: strandId, path: path, instruction: instruction };
        appSelector.title = instruction && instruction.type === "CloseApp" ? "Choose an App to Close" : "Choose an App";
        appSelector.open();
    }
    function openPaletteAppPicker(instruction) {
        const type = instruction ? instruction.type : "OpenApp";
        appPickerRequest = { paletteType: type };
        appSelector.title = type === "CloseApp" ? "Choose an App to Close" : "Choose an App";
        appSelector.open();
    }
    function resolvePickedIcon(app) {
        if (app.icon) return app.icon;
        // The backend ships no icons on some platforms - resolve the launch
        // target's native file icon so the block keeps showing it.
        try {
            if (root.appBridge && typeof root.appBridge.nativeAppIcon === "function") {
                const resolved = root.appBridge.nativeAppIcon(app.command || "");
                if (resolved) return resolved;
            }
        } catch (e) {
            console.warn("Failed to resolve native app icon", e);
        }
        return null;
    }
    function applyPickedApp(app) {
        const req = appPickerRequest;
        appPickerRequest = null;
        if (!req || !app) return;
        if (req.paletteType) {
            const fresh = defaultInstruction(req.paletteType);
            fresh.command = app.command;
            fresh.name = app.name;
            fresh.icon = root.resolvePickedIcon(app);
            root.invoke("add_strand", { x: 120, y: 120, instruction: fresh });
            return;
        }
        const current = req.instruction || {};
        const updated = JSON.parse(JSON.stringify(current));
        updated.command = app.command;
        updated.name = app.name;
        updated.icon = root.resolvePickedIcon(app);
        root.invoke("edit_instruction", { strandId: req.strandId, path: req.path, instruction: updated });
    }
    // ---- dragging a palette entry onto the canvas ----
    property var paletteDrag: null   // { spec, offsetX, offsetY }
    function beginPaletteDrag(spec, ox, oy, sx, sy) {
        paletteDrag = { spec: spec, offsetX: ox, offsetY: oy };
        ghost.spec = spec;
        movePaletteDrag(sx, sy);
        ghost.visible = true;
    }
    function movePaletteDrag(sx, sy) {
        if (!paletteDrag) return;
        const p = ghostLayer.mapFromItem(null, sx, sy);
        ghost.x = p.x - paletteDrag.offsetX; ghost.y = p.y - paletteDrag.offsetY;
        // Live attach preview, just like canvas drags: the canvas opens the
        // hovered gap and shows where the new block will land.
        const spec = paletteDrag.spec;
        if (spec && spec.kind === "value") {
            canvas.clearSnap();
            // Pass the dragged ghost's scene footprint so the canvas targets
            // the nearest slot to the block (proximity), not the bare cursor.
            // The ghost loader may not have built yet on the first move;
            // then the size is 0 and the canvas falls back to the pointer.
            const g1 = ghostLayer.mapToItem(null, ghost.x, ghost.y);
            const g2 = ghostLayer.mapToItem(null, ghost.x + ghost.width, ghost.y + ghost.height);
            canvas.updatePaletteValueTargetRect(g1.x, g1.y, g2.x, g2.y, sx, sy);
            return;
        }
        canvas.updatePaletteValueTarget(null, null);
        if (spec && spec.kind !== "value" && spec.instruction) {
            const at = canvas.workspacePoint(sx, sy);
            if (at) canvas.updatePaletteSnap(at.x - paletteDrag.offsetX / canvas.zoom, at.y - paletteDrag.offsetY / canvas.zoom, spec.instruction, ghost.width, ghost.height);
            else canvas.clearSnap();
        } else if (spec && (spec.kind === "custom" || spec.type)) {
            const at = canvas.workspacePoint(sx, sy);
            if (at) {
                const ins = spec.kind === "custom"
                    ? { id: "palette-new", type: ((spec.definition.pieces || []).filter(piece => piece.kind === "Branch").length ? "BranchCallBlock" : "CallBlock"), branches: (spec.definition.pieces || []).filter(piece => piece.kind === "Branch").map(() => []) }
                    : { id: "palette-new", type: spec.type, body: [], then_body: [], else_body: [], branches: [] };
                canvas.updatePaletteSnap(at.x - paletteDrag.offsetX / canvas.zoom, at.y - paletteDrag.offsetY / canvas.zoom, ins, ghost.width, ghost.height);
            } else canvas.clearSnap();
        } else canvas.clearSnap();
    }
    function endPaletteDrag(sx, sy) {
        const drag = paletteDrag;
        const snapValid = canvas.paletteSnapValid;
        const snapTargetId = canvas.paletteSnapTargetId;
        const snapPath = JSON.parse(JSON.stringify(canvas.paletteSnapPath || []));
        const valueTarget = canvas.paletteValueTarget ? JSON.parse(JSON.stringify(canvas.paletteValueTarget)) : null;
        cancelPaletteDrag();
        if (!drag || recording) return;
        const spec = drag.spec;
        // A fresh operator dropped straight onto a value slot fills it.
        if (spec.kind === "value" && valueTarget) { root.invoke("put_value", { location: valueTarget, value: spec.value }); return; }
        const at = canvas.workspacePoint(sx, sy);
        if (!at) return;
        const x = Math.round(at.x - drag.offsetX / canvas.zoom), y = Math.round(at.y - drag.offsetY / canvas.zoom);
        if (spec.kind === "value") { root.invoke("create_floating_value", { x: x, y: y, value: spec.value, originBlockId: null }); return; }
        // Attach into the previewed gap when one is showing, else park as a
        // new detached strand - mirrors the webapp palette drop.
        if (snapValid && snapTargetId) {
            if (spec.kind === "branch") {
                root.invoke("add_instruction", { strandId: snapTargetId, path: snapPath, instruction: { id: uuid(), type: "RunBranch", name: spec.instruction.name } });
            } else if (spec.kind === "custom") {
                const args = (spec.definition.pieces || []).filter(piece => piece.kind === "Input")
                    .map(piece => piece.value_type === "Bool" ? { kind: "Bool" } : numberValue(0));
                const branchCount = (spec.definition.pieces || []).filter(piece => piece.kind === "Branch").length;
                const instruction = { id: uuid(), type: branchCount ? "BranchCallBlock" : "CallBlock", block_id: spec.definition.id, args: args };
                if (branchCount) instruction.branches = Array.from({ length: branchCount }, () => []);
                root.invoke("add_instruction", { strandId: snapTargetId, path: snapPath, instruction: instruction });
            } else {
                root.invoke("add_instruction", { strandId: snapTargetId, path: snapPath, instruction: defaultInstruction(spec.type) });
            }
            return;
        }
        if (spec.kind === "branch") root.invoke("add_strand", { x: x, y: y, instruction: { id: uuid(), type: "RunBranch", name: spec.instruction.name } });
        else if (spec.kind === "custom") root.addCustomBlock(spec.definition, x, y);
        else root.addBlock(spec.type, x, y);
    }
    function cancelPaletteDrag() { paletteDrag = null; ghost.visible = false; ghost.spec = null; canvas.clearSnap(); canvas.updatePaletteValueTarget(null, null); }
    // A canvas block dropped on the sidebar deletes it and everything below it in its stack.
    function blockDefById(id) {
        const defs = root.macro ? root.macro.block_defs : [];
        for (let i = 0; i < defs.length; ++i) if (defs[i].id === id) return defs[i];
        return null;
    }
    function editBlockById(id) {
        const def = blockDefById(id);
        if (def) blockDialog.openForEdit(def);
    }
    // A custom block's header row takes its whole definition with it.
    function removeInstruction(strandId, path) {
        const strands = root.macro ? root.macro.strands : [];
        for (let i = 0; i < strands.length; ++i) {
            if (strands[i].id !== strandId) continue;
            const first = strands[i].instructions[0];
            if (path.length === 1 && path[0].index === 0 && first && first.type === "BlockHeader") {
                root.invoke("delete_block", { blockId: first.block_id });
                return;
            }
        }
        root.invoke("remove_instruction", { strandId: strandId, path: path });
    }
    function trashDraggedBlocks(strandId, path, tailCount, sx, sy) {
        if (root.sidebarCollapsed || !palette.contains(palette.mapFromItem(null, sx, sy))) return;
        if (path.length === 1 && path[0].index === 0) {
            const strands = root.macro ? root.macro.strands : [];
            for (let i = 0; i < strands.length; ++i) {
                if (strands[i].id !== strandId) continue;
                const first = strands[i].instructions[0];
                if (first && first.type === "BlockHeader") { root.invoke("delete_block", { blockId: first.block_id }); return; }
            }
            root.invoke("remove_strand", { strandId: strandId });
            return;
        }
        for (let i = 0; i < tailCount; ++i) root.invoke("remove_instruction", { strandId: strandId, path: path });
    }
    // An existing value dropped on the sidebar resets its slot (fields) or
    // deletes it (whole floating blocks) - mirrors the webapp trash.
    function trashDraggedValue(location, value, sx, sy) {
        if (root.sidebarCollapsed || !palette.contains(palette.mapFromItem(null, sx, sy))) return;
        if (location.kind === "Floating" && !(location.path && location.path.length)) root.invoke("remove_floating_value", { floatingId: location.floating_id });
        else root.invoke("take_value", { location: location });
    }
    onAppStateChanged: {
        if (appState.standalone_key !== null && appState.standalone_key !== undefined) {
            sidebarKey = appState.standalone_key;
            root.invoke("clear_standalone_key_capture");
        }
    }

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            id: toolbar
            // Collapse stages: 1 Record icon-only, 2 Run icon-only, 3 undo/redo/save in a menu.
            readonly property int tier: width < 700 ? 3 : (width < 780 ? 2 : (width < 860 ? 1 : 0))
            Layout.fillWidth: true; Layout.preferredHeight: 52; color: "#292a2d"; border.color: "#38393d"
            TitleBarDrag { anchors.fill: parent; visible: Qt.platform.os !== "osx" }
            RowLayout {
                anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                Image {
                    visible: Qt.platform.os !== "osx" && toolbar.tier < 3
                    source: "qrc:/icons/blockwork.png"; sourceSize: Qt.size(40, 40)
                    Layout.preferredWidth: 20; Layout.preferredHeight: 20; Layout.rightMargin: 2
                    fillMode: Image.PreserveAspectFit; smooth: true; mipmap: true
                }
                BwButton {
                    iconName: "panel-left"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: !!root.macro
                    ToolTip.visible: hovered; ToolTip.text: root.sidebarCollapsed ? "Show sidebar" : "Hide sidebar"
                    onClicked: { root.sidebarToggling = true; root.sidebarCollapsed = !root.sidebarCollapsed; }
                }
                Rectangle {
                    id: libraryButton
                    Layout.preferredWidth: Math.max(110, Math.min(240, toolbar.width * 0.2)); Layout.preferredHeight: 34
                    radius: Theme.radius; color: libraryHover.hovered ? "#3b3c40" : Theme.panelRaised; border.color: library.visible ? Theme.accent : Theme.border
                    Accessible.role: Accessible.Button; Accessible.name: "Macro library"
                    RowLayout {
                        anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 8; spacing: 7
                        LucideIcon { name: "blocks"; width: 15; height: 15; color: Theme.textDim }
                        Text {
                            Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 13
                            text: root.macro ? (root.macro.settings.folder ? root.macro.settings.folder + " / " : "") + root.macro.name : "Choose a macro"
                            color: root.macro ? Theme.text : Theme.textDim
                        }
                        LucideIcon { name: "chevron-down"; width: 14; height: 14; color: Theme.textDim }
                    }
                    HoverHandler { id: libraryHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: library.visible ? library.close() : library.open() }
                    ToolTip.visible: libraryHover.hovered && !library.visible; ToolTip.delay: 500; ToolTip.text: "Macro library"
                    LibraryPanel {
                        id: library
                        y: libraryButton.height + 4
                        appState: root.appState; invoke: root.invoke
                        runningIds: root.runningMacroIds
                        onImportRequested: if (root.macroFiles) root.macroFiles.importMacro()
                        onExportRequested: macroId => { if (root.macroFiles) root.macroFiles.exportMacro(macroId); }
                        onBundleRequested: (macroIds, name) => { if (root.macroFiles) root.macroFiles.exportBundle(macroIds, name); }
                        onTemplateRequested: templateDialog.open()
                        onDeleteRequested: (macroId, name) => { deleteMacroDialog.macroId = macroId; deleteMacroDialog.macroName = name; deleteMacroDialog.open(); }
                    }
                }
                BwButton {
                    id: macroMenuButton
                    iconName: "ellipsis"; text: ""; implicitWidth: 34; implicitHeight: 34
                    ToolTip.visible: hovered && !macroMenu.visible; ToolTip.text: "Macro actions"
                    property double pressStamp: 0
                    property double closeStamp: 0
                    onPressed: pressStamp = Date.now()
                    // The press that closes the menu also clicks the button; don't reopen on release.
                    onClicked: if (Math.abs(closeStamp - pressStamp) > 100) macroMenu.popup(macroMenuButton, 0, macroMenuButton.height + 4)
                    BwMenu {
                        id: macroMenu
                        onAboutToHide: macroMenuButton.closeStamp = Date.now()
                        implicitWidth: 200
                        BwMenuItem { iconName: "sliders"; text: "Macro settings"; enabled: !!root.macro; onTriggered: macroSettings.open() }
                        BwMenuItem { iconName: "plus"; text: "New macro"; onTriggered: root.invoke("new_macro") }
                        BwMenuItem { iconName: "sparkles"; text: "New from template…"; onTriggered: templateDialog.open() }
                        BwMenuItem { iconName: "copy"; text: "Duplicate macro"; enabled: !!root.macro; onTriggered: root.invoke("duplicate_macro", { id: root.macro.id }) }
                        BwMenuItem { iconName: "upload"; text: "Export…"; enabled: !!root.macro && !!root.macroFiles; onTriggered: root.macroFiles.exportMacro(root.macro.id) }
                        BwMenuItem { iconName: "circle-dot"; text: "Clear breakpoints"; enabled: (appState.breakpoints || []).length > 0; onTriggered: root.invoke("clear_breakpoints") }
                        BwMenuItem { iconName: "octagon"; text: "Emergency stop"; danger: true; onTriggered: root.invoke("emergency_stop") }
                        BwMenuItem { iconName: "trash"; text: "Delete macro"; danger: true; enabled: !!root.macro; onTriggered: removeDialog.open() }
                    }
                }
                Item { Layout.fillWidth: true }
                BwButton {
                    implicitHeight: 34; iconName: "play"; iconFilled: true; iconColor: Theme.accent; text: toolbar.tier < 2 ? "Run" : ""
                    implicitWidth: toolbar.tier < 2 ? 76 : 34
                    enabled: appState.macro_selected !== null; ToolTip.visible: hovered && toolbar.tier >= 2; ToolTip.text: "Run macro"
                    onClicked: root.invoke("run_macro")
                }
                Row {
                    spacing: -1
                    BwButton {
                        implicitHeight: 34; iconName: root.recording ? "square" : "circle"; iconFilled: true; iconColor: Theme.danger
                        text: toolbar.tier < 1 ? root.recordingLabel() : ""
                        implicitWidth: toolbar.tier < 1 ? 100 : 34
                        enabled: appState.macro_selected !== null; ToolTip.visible: hovered && toolbar.tier >= 1; ToolTip.text: root.recordingLabel()
                        onClicked: root.invoke(root.recording ? "stop_recording" : "start_recording")
                    }
                    BwButton {
                        iconName: "chevron-down"; text: ""; implicitWidth: 26; implicitHeight: 34
                        ToolTip.visible: hovered; ToolTip.text: "Recording settings"; onClicked: recordingSettings.open()
                    }
                }
                BwButton {
                    iconName: "crosshair"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: !!root.macro
                    ToolTip.visible: hovered; ToolTip.text: "Pick a point or region from the screen"
                    onClicked: root.startScreenPick({ kind: "tool" })
                }
                Item { Layout.fillWidth: true }
                BwButton { visible: toolbar.tier < 3; iconName: "undo"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: appState.can_undo; ToolTip.visible: hovered; ToolTip.text: "Undo"; onClicked: root.invoke("undo") }
                BwButton { visible: toolbar.tier < 3; iconName: "redo"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: appState.can_redo; ToolTip.visible: hovered; ToolTip.text: "Redo"; onClicked: root.invoke("redo") }
                BwButton { visible: toolbar.tier < 3; iconName: "save"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: !!root.macro; ToolTip.visible: hovered; ToolTip.text: "Save macro"; onClicked: root.invoke("save_macro") }
                BwButton {
                    id: editMenuButton
                    visible: toolbar.tier >= 3
                    iconName: "ellipsis"; text: ""; implicitWidth: 34; implicitHeight: 34
                    ToolTip.visible: hovered && !editMenu.visible; ToolTip.text: "Undo, redo and save"
                    property double pressStamp: 0
                    property double closeStamp: 0
                    onPressed: pressStamp = Date.now()
                    // The press that closes the menu also clicks the button; don't reopen on release.
                    onClicked: if (Math.abs(closeStamp - pressStamp) > 100) editMenu.popup(editMenuButton, 0, editMenuButton.height + 4)
                    BwMenu {
                        id: editMenu
                        onAboutToHide: editMenuButton.closeStamp = Date.now()
                        implicitWidth: 170
                        BwMenuItem { iconName: "undo"; text: "Undo"; enabled: appState.can_undo; onTriggered: root.invoke("undo") }
                        BwMenuItem { iconName: "redo"; text: "Redo"; enabled: appState.can_redo; onTriggered: root.invoke("redo") }
                        BwMenuItem { iconName: "save"; text: "Save macro"; enabled: !!root.macro; onTriggered: root.invoke("save_macro") }
                    }
                }
                BwButton { iconName: "settings"; text: ""; implicitWidth: 34; implicitHeight: 34; ToolTip.visible: hovered; ToolTip.text: "Settings"; onClicked: root.invoke("open_settings") }
                WindowControls { visible: Qt.platform.os !== "osx" }
            }
        }
        Rectangle {
            visible: !!appState.triggers_paused
            Layout.fillWidth: true; Layout.preferredHeight: 40; color: "#3a2a12"; border.color: "#5a4220"
            RowLayout {
                anchors.fill: parent; anchors.leftMargin: 14; anchors.rightMargin: 10; spacing: 10
                LucideIcon { name: "pause"; color: Theme.warning; width: 16; height: 16 }
                Text { Layout.fillWidth: true; text: "Emergency stop: triggers are paused until you resume them."; color: Theme.text; font.pixelSize: 13; elide: Text.ElideRight }
                BwButton { text: "Resume triggers"; implicitHeight: 28; onClicked: root.invoke("resume_triggers") }
            }
        }
        Rectangle {
            id: debugBar
            readonly property string macroId: root.macro ? root.macro.id : ""
            readonly property var paused: (appState.paused_strands || []).filter(p => p.macro_id === macroId)
            readonly property bool isPaused: !!macroId && (appState.paused_macros || []).indexOf(macroId) >= 0
            readonly property var strand: paused.length ? paused[0] : null
            function act(action) { root.invoke("debug_action", { action: action }); }
            visible: !!root.macro && (root.runningIds.length > 0 || isPaused || paused.length > 0)
            Layout.fillWidth: true; Layout.preferredHeight: debugColumn.implicitHeight + 14
            color: isPaused ? "#33291a" : "#1c2a21"; border.color: isPaused ? "#5a4220" : "#2b4a35"
            ColumnLayout {
                id: debugColumn
                anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
                anchors.leftMargin: 14; anchors.rightMargin: 10; anchors.topMargin: 7; spacing: 6
                RowLayout {
                    Layout.fillWidth: true; spacing: 8
                    LucideIcon { name: debugBar.isPaused ? "pause" : "play"; color: debugBar.isPaused ? Theme.warning : Theme.running; width: 16; height: 16 }
                    Text {
                        Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 13
                        text: debugBar.strand
                            ? "Paused before \u201c" + root.blockLabel(debugBar.strand.block_id) + "\u201d" + (debugBar.paused.length > 1 ? " (" + debugBar.paused.length + " strands waiting)" : "")
                            : debugBar.isPaused ? "Pausing at the next block…" : "Running"
                        HoverHandler { cursorShape: debugBar.strand ? Qt.PointingHandCursor : Qt.ArrowCursor }
                        TapHandler { enabled: !!debugBar.strand; onTapped: canvas.revealInstruction(debugBar.strand.block_id) }
                    }
                    BwButton { visible: !debugBar.isPaused; iconName: "pause"; text: "Pause"; implicitHeight: 28; ToolTip.visible: hovered; ToolTip.text: "Pause before the next block (F8)"; onClicked: debugBar.act("pause") }
                    BwButton { visible: debugBar.isPaused; iconName: "play"; text: "Resume"; implicitHeight: 28; ToolTip.visible: hovered; ToolTip.text: "Run on until the next breakpoint (F8)"; onClicked: debugBar.act("resume") }
                    BwButton { visible: debugBar.isPaused; iconName: "step-forward"; text: "Step"; implicitHeight: 28; enabled: debugBar.paused.length > 0; ToolTip.visible: hovered; ToolTip.text: "Run one block, then pause again (F10)"; onClicked: debugBar.act("step") }
                    BwButton { iconName: "square"; text: "Stop"; danger: true; implicitHeight: 28; onClicked: debugBar.act("stop") }
                }
                Flow {
                    visible: !!debugBar.strand
                    Layout.fillWidth: true; Layout.bottomMargin: 2; spacing: 6
                    Text {
                        visible: !!debugBar.strand && !debugBar.strand.variables.length && !debugBar.strand.lists.length
                        text: "No variables"; color: Theme.textDim; font.pixelSize: 12
                    }
                    Repeater {
                        model: debugBar.strand ? debugBar.strand.variables.map(v => v[0] + " = " + v[1])
                            .concat(debugBar.strand.lists.map(l => l[0] + ": [" + l[1].join(", ") + "]")) : []
                        delegate: Rectangle {
                            required property string modelData
                            width: Math.min(chipText.implicitWidth + 16, 320); height: 24; radius: 12
                            color: "#2a2b2e"; border.color: "#45464b"
                            Text {
                                id: chipText
                                anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8
                                verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight
                                text: parent.modelData; color: Theme.text; font.pixelSize: 12; font.family: "monospace"
                            }
                            ToolTip.visible: chipHover.hovered && chipText.truncated; ToolTip.text: modelData
                            HoverHandler { id: chipHover }
                        }
                    }
                }
            }
            Shortcut { sequence: "F8"; enabled: debugBar.visible; onActivated: debugBar.act(debugBar.isPaused ? "resume" : "pause") }
            Shortcut { sequence: "F10"; enabled: debugBar.visible && debugBar.paused.length > 0; onActivated: debugBar.act("step") }
        }
        Item {
            visible: !!root.macro
            Layout.fillWidth: true; Layout.fillHeight: true
            RowLayout { anchors.fill: parent; spacing: 0
                Item {
                    id: sidebarHost
                    // The flyout layout keeps only its icon rail in the layout and floats the list over the canvas.
                    Layout.preferredWidth: root.sidebarCollapsed ? 0 : (palette.layout === "flyout" ? palette.railWidth : root.sidebarWidth); Layout.fillHeight: true
                    clip: palette.layout !== "flyout"
                    z: palette.layout === "flyout" ? 5 : 0
                    // Animate only the collapse toggle, not drag-resizing.
                    Behavior on Layout.preferredWidth {
                        enabled: root.sidebarToggling
                        NumberAnimation { duration: 140; easing.type: Easing.OutCubic; onRunningChanged: if (!running) root.sidebarToggling = false }
                    }
                PalettePanel {
                    id: palette
                    // Fixed width and visible until the slot is closed, so it slides out under the clip.
                    anchors.left: parent.left; anchors.top: parent.top; anchors.bottom: parent.bottom
                    width: palette.layout === "flyout" && !palette.flyoutShown ? palette.railWidth : root.sidebarWidth
                    visible: sidebarHost.width > 0
                    layout: appState.palette_layout || "rail"
                    trashActive: canvas.dragging
                    trashArmed: canvas.dragging && palette.contains(palette.mapFromItem(null, canvas.dragSceneX, canvas.dragSceneY))
                    onResizeRequested: width => root.sidebarWidth = Math.max(root.minSidebarWidth, Math.min(root.maxSidebarWidth, width))
                    onDragStarted: (spec, sx, sy, ox, oy) => root.beginPaletteDrag(spec, ox, oy, sx, sy)
                    onDragMoved: (sx, sy) => root.movePaletteDrag(sx, sy)
                    onDragEnded: (sx, sy) => root.endPaletteDrag(sx, sy)
                    onDragCanceled: root.cancelPaletteDrag()
                    instructionTypes: root.instructionTypes
                    listInstructionTypes: root.listInstructionTypes
                    extraOperators: root.extraOperators
                    extraCategories: root.extraCategories
                    appCategories: [{ id: "screen", name: "Screen", icon: "crosshair" }]
                    variables: root.macro ? root.macro.variables : []
                    lists: root.macro && root.macro.lists ? root.macro.lists : []
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                    keyCapture: appState.key_capture
                    standaloneKey: root.sidebarKey
                    onBlockActivated: type => root.addBlock(type, 120, 120)
                    onCustomBlockActivated: definition => root.addCustomBlock(definition, 120, 120)
                    onMakeVariableRequested: variableDialog.openForCreate()
                    onRenameVariableRequested: name => variableDialog.openForRename(name)
                    onDeleteVariableRequested: name => root.invoke("delete_variable", { name: name })
                    onEditBlockRequested: definition => blockDialog.openForEdit(definition)
                    onDeleteBlockRequested: blockId => root.invoke("delete_block", { blockId: blockId })
                    onMakeListRequested: listDialog.openForCreate()
                    onListEditorStateRequested: (name, visible, x, y) => root.invoke("set_list_editor_state", { name: name, visible: visible, x: x, y: y })
                    onRenameListRequested: name => listDialog.openForRename(name)
                    onDeleteListRequested: name => { deleteListDialog.listName = name; deleteListDialog.open(); }
                    onMakeBlockRequested: blockDialog.openForCreate()
                    onStandaloneKeyCaptureRequested: root.invoke("start_standalone_key_capture")
                    onAppPickerRequested: instruction => root.openPaletteAppPicker(instruction)
                    onDetailsRequested:(name,identifier,explainer)=>detailsDialog.show(name,identifier,explainer)
                    onValueActivated: value => root.invoke("create_floating_value", { x: 160, y: 140, value: value, originBlockId: null })
                }
                }
                BlockCanvas {
                    id: canvas
                    Layout.fillWidth: true; Layout.fillHeight: true
                    strands: root.macro ? root.macro.strands : []
                    recordingTargetId: root.macro && root.macro.recording_target_strand_id ? root.macro.recording_target_strand_id : ""
                    comments: root.macro ? root.macro.comments : []
                    floatingValues: root.macro ? root.macro.floating_values : []
                    variables: root.macro ? root.macro.variables : []
                    lists: root.macro && root.macro.lists ? root.macro.lists : []
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                    keyCapture: appState.key_capture
                    locked: root.recording
                    onStrandMoved: (strandId, x, y) => root.invoke("move_strand", { strandId: strandId, x: x, y: y })
                    onInstructionSplit: (strandId, path, x, y) => root.invoke("split_strand", { strandId: strandId, path: path, x: x, y: y })
                    onStrandsMerged: (draggedId, targetId, path) => root.invoke("merge_strand", { draggedId: draggedId, targetId: targetId, path: path })
                    onTailMerged: (strandId, path, targetId, targetPath) => root.invoke("merge_tail", { strandId: strandId, path: path, targetId: targetId, targetPath: targetPath })
                    onBlockDragOutside: (strandId, path, tailCount, sx, sy) => root.trashDraggedBlocks(strandId, path, tailCount, sx, sy)
                    onInstructionRemoved: (strandId, path) => root.removeInstruction(strandId, path)
                    canPaste: root.clipboard.length > 0
                    clearIcon: appState.confirm_clear_instructions ? "triangle-alert" : "trash"
                    clearText: appState.confirm_clear_instructions ? "Confirm delete (" + appState.confirm_clear_instructions_remaining_secs + "s)?" : "Delete " + root.totalBlocks() + " Blocks"
                    onCanvasMenuAction: (action, x, y) => {
                        if (action === "paste" && root.clipboard.length) root.invoke("paste_instructions", { x: x, y: y, instructions: root.clipboard.map(root.refreshIds) });
                    }
                    onBlockMenuAction: (action, strandId, path, instruction) => {
                        if (action === "edit_block") root.editBlockById(instruction.block_id);
                        else if (action === "breakpoint") root.invoke("toggle_breakpoint", { blockId: instruction.id });
                        else if (action === "copy") root.clipboard = [instruction];
                        else if (action === "copy_all") {
                            const strand = canvas.strandById(strandId);
                            const list = strand ? canvas.listFor(strand, path.slice(0, path.length - 1)) : null;
                            if (list) root.clipboard = list.slice(path[path.length - 1].index);
                        } else if (action === "cut") { root.clipboard = [instruction]; root.removeInstruction(strandId, path); }
                    }
                    onInstructionDuplicated: (strandId, path, instruction) => root.invoke("add_instruction", { strandId: strandId, path: root.nextPath(path), instruction: root.refreshIds(instruction) })
                    onInstructionEdited: (strandId, path, instruction) => root.invoke("edit_instruction", { strandId: strandId, path: path, instruction: instruction })
                    onRunBranchRequested: (strandId, path, name) => root.invoke("add_instruction", { strandId: strandId, path: root.nextPath(path), instruction: { id: root.uuid(), type: "RunBranch", name: name } })
                    onValueEdited: (location, text) => root.invoke("edit_value_field", { location: location, text: text })
                    onValueTakeRequested: location => root.invoke("take_value", { location: location })
                    onValuePutRequested: (location, value) => root.invoke("put_value", { location: location, value: value })
                    onValueCreateRequested: (x, y, value) => root.invoke("create_floating_value", { x: x, y: y, value: value, originBlockId: null })
                    onValueDragOutside: (location, value, sx, sy) => root.trashDraggedValue(location, value, sx, sy)
                    onCommentForInstructionRequested: instruction => {
                        const existing = (root.macro && root.macro.comments || []).find(c => c.attached_to === instruction.id);
                        if (existing) { if (existing.collapsed) root.invoke("set_comment_collapsed", { commentId: existing.id, collapsed: false }); }
                        else root.invoke("create_attached_comment", { instructionId: instruction.id, dx: 48, dy: 18, text: "" });
                    }
                    onRecordingTargetRequested: strandId => root.invoke("set_recording_target", { strandId: strandId })
                    onKeyCaptureRequested:(strandId,path)=>root.invoke("start_key_capture",{strandId:strandId,path:path})
                    onAppPickerRequested:(strandId,path,instruction)=>root.openAppPicker(strandId,path,instruction)
                    onPieceActionRequested:(action,strandId,path,instruction)=>root.onPieceAction(action,strandId,path,instruction)
                    onDetailsRequested:type=>detailsDialog.show(type,type,root.instructionExplainer(type))
                    onCanvasNoteRequested: (x, y) => root.invoke("create_comment", { x: x, y: y, text: "" })
                    onClearRequested: root.invoke("clear_instructions")
                    onCommentMoved: (commentId, x, y) => root.invoke("move_comment", { commentId: commentId, x: x, y: y })
                    onCommentEdited: (commentId, text) => root.invoke("edit_comment_text", { commentId: commentId, text: text })
                    onCommentCollapseChanged: (commentId, collapsed) => root.invoke("set_comment_collapsed", { commentId: commentId, collapsed: collapsed })
                    onCommentRemoved: commentId => root.invoke("remove_comment", { commentId: commentId })
                    onFloatingValueMoved: (floatingId, x, y) => root.invoke("move_floating_value", { floatingId: floatingId, x: x, y: y })
                    onFloatingValueRemoved: floatingId => root.invoke("remove_floating_value", { floatingId: floatingId })
                    onListItemsEdited: (name, items) => root.invoke("set_list_items", { name: name, items: items })
                    onListEditorStateChanged: (name, visible, x, y) => root.invoke("set_list_editor_state", { name: name, visible: visible, x: x, y: y })
                }
            }
            Rectangle {
                visible: root.recording
                anchors.fill: parent; color: "#d8222326"; z: 20
                Column { anchors.centerIn: parent; spacing: 10
                    LucideIcon { anchors.horizontalCenter: parent.horizontalCenter; name: "circle"; color: "#ff4a4a"; width: 48; height: 48; strokeWidth: 7 }
                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: "Recording…"; color: "#ededee"; font.pixelSize: 17; font.weight: Font.Bold }
                    Text { text: "Adding and removing instructions is disabled while recording."; color: "#a5a6ab"; font.pixelSize: 14; font.weight: Font.DemiBold }
                }
            }
        }
        ActivityPanel {
            id: activity
            visible: !!root.macro
            Layout.fillWidth: true; Layout.preferredHeight: implicitHeight
            entries: root.runLog
            macroId: root.macro ? root.macro.id : ""
            onBlockRequested: blockId => canvas.revealInstruction(blockId)
            onClearRequested: root.invoke("clear_run_log")
        }
        Item {
            visible: !root.macro
            Layout.fillWidth: true; Layout.fillHeight: true
            Column { anchors.centerIn: parent; spacing: 8
                LucideIcon { anchors.horizontalCenter: parent.horizontalCenter; name: "blocks"; color: "#777980"; width: 48; height: 48 }
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: "No macro selected"; color: "#e7e7e8"; font.pixelSize: 16; font.weight: Font.DemiBold }
                Text { text: "Choose a macro above or create a new one."; color: "#9fa0a6"; font.pixelSize: 13 }
            }
        }
    }

    BwDialog {
        id: deleteMacroDialog; title: "Delete macro?"; standardButtons: Dialog.Yes | Dialog.Cancel
        property string macroId: ""
        property string macroName: ""
        Text { text: "This removes \u201c" + deleteMacroDialog.macroName + "\u201d from Blockwork."; color: "#e7e7e8" }
        onAccepted: root.invoke("delete_macro", { id: macroId })
    }
    BwDialog {
        id: removeDialog; title: "Delete macro?"; standardButtons: Dialog.Yes | Dialog.Cancel
        Text { text: "This removes the selected macro from Blockwork."; color: "#e7e7e8" }
        onAccepted: root.invoke("remove_macro")
    }
    BwDialog {
        id: macroSettings; title: "Macro settings"; standardButtons: Dialog.Close
        readonly property var settings: root.macro && root.macro.settings ? root.macro.settings : ({})
        function saveConditions(days) {
            root.invoke("set_macro_conditions", { onlyInWindow: condWindow.text, power: powerBox.options[powerBox.currentIndex].value,
                activeFrom: condFrom.text, activeUntil: condUntil.text, activeDays: days === undefined ? (settings.active_days || 0) : days });
        }
        Flickable {
        implicitWidth: 380; implicitHeight: Math.min(settingsColumn.implicitHeight, root.height - 190)
        contentHeight: settingsColumn.implicitHeight; clip: true; boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        Column { id: settingsColumn; spacing: 14; width: 360
            Text { text: "Controls behavior specific to this macro."; color: "#a5a6ab" }
            Text { text: "Title"; color: "#a1a2a7"; font.pixelSize: 13 }
            BwTextField { width: parent.width; text: root.macro ? root.macro.name : ""; onEditingFinished: root.invoke("set_title", { title: text }) }
            Text { text: "Speed"; color: "#a1a2a7"; font.pixelSize: 13 }
            RowLayout { width: parent.width; spacing: 10
                Slider {
                    id: speed; from: .1; to: 10; value: root.macro ? root.macro.speed_multiplier : 1; Layout.fillWidth: true; implicitHeight: 30
                    onMoved: root.invoke("set_macro_speed_multiplier", { multiplier: value })
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    background: Rectangle { x: speed.leftPadding; y: speed.topPadding + speed.availableHeight / 2 - height / 2; width: speed.availableWidth; height: 5; radius: 3; color: "#45474d"; Rectangle { width: speed.visualPosition * parent.width; height: parent.height; radius: 3; color: Theme.accent } }
                    handle: Rectangle { x: speed.leftPadding + speed.visualPosition * (speed.availableWidth - width); y: speed.topPadding + speed.availableHeight / 2 - height / 2; width: 16; height: 16; radius: 8; color: "white"; border.width: 2; border.color: Theme.accent }
                }
                BwTextField { text: Number(speed.value).toFixed(2); Layout.preferredWidth: 62; horizontalAlignment: Text.AlignRight; onEditingFinished: root.invoke("set_macro_speed_multiplier", { multiplier: Number(text) }) }
                Text { text: "x"; color: "#a1a2a7" }
            }
            BwSwitch { Accessible.name: "Listen for triggers while Blockwork is open"; checked: root.macro && root.macro.settings ? root.macro.settings.always_listen : false; onToggled: checked => root.invoke("set_macro_always_listen", { enabled: checked }) }
            Text { text: "Time limit"; color: "#a1a2a7"; font.pixelSize: 13 }
            RowLayout { width: parent.width; spacing: 10
                Text { text: "Stop a run after"; color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    Layout.preferredWidth: 70; horizontalAlignment: Text.AlignRight
                    validator: IntValidator { bottom: 0; top: 86400 }
                    text: root.macro && root.macro.settings ? String(root.macro.settings.max_run_secs) : "0"
                    onEditingFinished: root.invoke("set_macro_max_run_secs", { seconds: Number(text) || 0 })
                }
                Text { Layout.fillWidth: true; text: "seconds (0 = no limit)"; color: "#a1a2a7"; font.pixelSize: 13 }
            }
            Text { text: "Loop limit"; color: "#a1a2a7"; font.pixelSize: 13 }
            RowLayout { width: parent.width; spacing: 10
                Text { text: "Stop a loop after"; color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    Layout.preferredWidth: 90; horizontalAlignment: Text.AlignRight
                    validator: IntValidator { bottom: 0; top: 2000000000 }
                    text: root.macro && root.macro.settings ? String(root.macro.settings.max_loop_iterations || 0) : "0"
                    onEditingFinished: root.invoke("set_macro_max_loop_iterations", { iterations: Number(text) || 0 })
                }
                Text { Layout.fillWidth: true; text: "repeats (0 = no limit)"; color: "#a1a2a7"; font.pixelSize: 13 }
            }
            Text { text: "Conditions"; color: "#a1a2a7"; font.pixelSize: 13 }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: "Triggers and hotkeys only start this macro when all of these hold. The Run button ignores them."
            }
            RowLayout { width: parent.width; spacing: 8
                Text { text: "While"; color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    id: condWindow; Layout.fillWidth: true; placeholderText: "any window"
                    text: macroSettings.settings.only_in_window || ""
                    onEditingFinished: macroSettings.saveConditions()
                }
                Text { text: "is focused"; color: Theme.text; font.pixelSize: 13 }
            }
            RowLayout { width: parent.width; spacing: 8
                Text { text: "On"; color: Theme.text; font.pixelSize: 13 }
                BwComboBox {
                    id: powerBox; Layout.fillWidth: true; textRole: "text"
                    readonly property var options: [{ value: "Any", text: "battery or AC power" }, { value: "PluggedIn", text: "AC power only" }, { value: "OnBattery", text: "battery only" }]
                    model: options
                    currentIndex: Math.max(0, options.findIndex(o => o.value === macroSettings.settings.power))
                    onActivated: macroSettings.saveConditions()
                }
            }
            RowLayout { width: parent.width; spacing: 8
                Text { text: "Between"; color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    id: condFrom; Layout.preferredWidth: 70; horizontalAlignment: Text.AlignHCenter; placeholderText: "09:00"
                    text: macroSettings.settings.active_from || ""
                    onEditingFinished: if (!!text === !!condUntil.text) macroSettings.saveConditions()
                }
                Text { text: "and"; color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    id: condUntil; Layout.preferredWidth: 70; horizontalAlignment: Text.AlignHCenter; placeholderText: "17:00"
                    text: macroSettings.settings.active_until || ""
                    onEditingFinished: if (!!text === !!condFrom.text) macroSettings.saveConditions()
                }
                Text { Layout.fillWidth: true; text: "(empty = all day)"; color: Theme.textDim; font.pixelSize: 12 }
            }
            Row { spacing: 6
                Text { text: "On"; color: Theme.text; font.pixelSize: 13; anchors.verticalCenter: parent.verticalCenter; rightPadding: 2 }
                Repeater {
                    model: ["S", "M", "T", "W", "T", "F", "S"]
                    delegate: Rectangle {
                        required property string modelData
                        required property int index
                        readonly property int days: macroSettings.settings.active_days || 0
                        // No days picked means every day.
                        readonly property bool on: days === 0 || (days & (1 << index)) !== 0
                        width: 30; height: 30; radius: 15
                        color: on ? Theme.accent : Theme.panelRaised; border.color: Theme.border
                        Accessible.name: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][index]
                        Text { anchors.centerIn: parent; text: parent.modelData; color: parent.on ? "white" : Theme.textDim; font.pixelSize: 12; font.weight: Font.DemiBold }
                        MouseArea {
                            anchors.fill: parent; cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                const next = (parent.days === 0 ? 0x7f : parent.days) ^ (1 << parent.index);
                                if (next === 0) return; // keep at least one day
                                macroSettings.saveConditions(next === 0x7f ? 0 : next);
                            }
                        }
                    }
                }
            }
            Text { text: "Schedules"; color: "#a1a2a7"; font.pixelSize: 13 }
            BwSwitch {
                Accessible.name: "Catch up on missed schedules"
                checked: !!macroSettings.settings.catch_up_missed
                onToggled: checked => root.invoke("set_macro_catch_up", { enabled: checked })
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: "Runs a schedule or \"once at\" trigger once if its time passed while Blockwork was closed or the computer was asleep (up to a week back)."
            }
            Text { text: "Permissions"; color: "#a1a2a7"; font.pixelSize: 13 }
            BwSwitch {
                Accessible.name: "Ask before risky actions"
                checked: root.permissions !== null
                onToggled: checked => root.invoke("set_macro_restricted", { restricted: checked })
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: root.permissions === null
                    ? "This macro may do anything its blocks say. Imported macros ask first until you allow them."
                    : "Ticked abilities are allowed. The rest show a dialog the first time the macro tries them."
            }
            Repeater {
                model: root.permissions === null ? [] : root.guardedAbilities
                delegate: BwCheckBox {
                    required property var modelData
                    text: modelData.label
                    checked: root.permissions.indexOf(modelData.kind) >= 0
                    onToggled: root.invoke("set_macro_permission", { kind: modelData.kind, allowed: checked })
                }
            }
        }
        }
    }
    BwDialog {
        id: pickError; title: "Couldn't read the screen"; standardButtons: Dialog.Ok
        property string message: ""
        function show(text) { message = text || "Unknown error"; open(); }
        Text { width: 420; wrapMode: Text.WordWrap; text: pickError.message; color: Theme.text; font.pixelSize: 13 }
    }
    BwDialog {
        id: pickResult; title: "Picked from the screen"; standardButtons: Dialog.Close
        property var pick: null
        readonly property bool region: !!pick && pick.width > 0 && pick.height > 0
        function show(result) { pick = result; open(); }
        Column {
            width: 440; spacing: 12
            Row {
                spacing: 14
                Image {
                    visible: pickResult.region; source: pickResult.region ? pickResult.pick.image : ""
                    width: Math.min(160, implicitWidth); height: Math.min(90, implicitHeight); fillMode: Image.PreserveAspectFit; smooth: false
                }
                Column {
                    spacing: 6
                    TextEdit {
                        readOnly: true; selectByMouse: true; color: Theme.text; font.pixelSize: 13
                        text: !pickResult.pick ? "" : "x " + pickResult.pick.x + "   y " + pickResult.pick.y
                            + (pickResult.region ? "\nwidth " + pickResult.pick.width + "   height " + pickResult.pick.height : "")
                    }
                    Row {
                        spacing: 8
                        Rectangle { width: 18; height: 18; radius: 4; color: pickResult.pick ? pickResult.pick.color : "black"; border.color: Theme.border }
                        TextEdit { readOnly: true; selectByMouse: true; text: pickResult.pick ? pickResult.pick.color : ""; color: Theme.text; font.pixelSize: 13 }
                    }
                }
            }
            Text { text: "Add to the canvas:"; color: Theme.textDim; font.pixelSize: 12 }
            Flow {
                width: parent.width; spacing: 8
                Repeater {
                    model: pickResult.region
                        ? [["ScreenText", "text in this region"], ["AverageColor", "average color here"], ["ClickImage", "click this image"],
                           ["WaitForImage", "wait for this image"], ["FindImage", "find this image"]]
                        : [["PixelColor", "color at this point"], ["MoveMouse", "move mouse here"]]
                    delegate: BwButton {
                        required property var modelData
                        text: modelData[1]
                        onClicked: { root.addPicked(modelData[0], pickResult.pick); pickResult.close(); }
                    }
                }
            }
        }
    }
    BwDialog {
        id: imageFileDialog; title: "Use an image file"; standardButtons: Dialog.Ok | Dialog.Cancel
        property var request: null
        function openFor(req) { request = req; imagePath.text = ""; open(); imagePath.forceActiveFocus(); }
        Column { width: 420; spacing: 8
            Text { text: "Path to a PNG image"; color: Theme.text }
            BwTextField { id: imagePath; width: parent.width; placeholderText: "~/Pictures/button.png"; onAccepted: imageFileDialog.accept() }
        }
        onAccepted: {
            const req = request;
            root.daemonCall("image_from_file", { path: imagePath.text.trim() }, (image, error) => {
                if (image) root.setBlockImage(req, image); else pickError.show(error);
            });
        }
    }
    BwDialog {
        id: templateDialog; title: "Start from a template"; standardButtons: Dialog.Cancel
        TemplateList {
            // Cards report no implicit width, so give the dialog one to size from
            implicitWidth: 560; templates: appState.templates || []
            onPicked: id => { root.invoke("new_macro_from_template", { template: id }); templateDialog.close(); }
        }
    }
    BwDialog {
        id: recordingSettings; title: "Recording settings"; standardButtons: Dialog.Close
        Column { spacing: 14; width: 390
            BwSwitch { Accessible.name: "Record mouse movement"; checked: appState.record_mouse_movement; onToggled: checked => root.invoke("toggle_record_mouse_movement", { enabled: checked }) }
            BwSwitch { Accessible.name: "Use relative mouse movement"; checked: appState.record_mouse_relative; onToggled: checked => root.invoke("toggle_record_mouse_relative", { relative: checked }) }
            Text { width: parent.width; wrapMode: Text.WordWrap; text: appState.absolute_mouse_position_available ? "Absolute positioning is available on this system." : "Absolute positioning is unavailable; relative movement will be used."; color: "#9fa0a6"; font.pixelSize: 12 }
        }
    }
    BwDialog {
        id: variableDialog
        property string renameTarget: ""
        title: renameTarget.length ? "Rename Variable" : "Make a Variable"
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openForCreate() { renameTarget = ""; variableName.text = ""; open(); variableName.forceActiveFocus(); }
        function openForRename(name) { renameTarget = name; variableName.text = name; open(); variableName.forceActiveFocus(); variableName.selectAll(); }
        Column { width: 340; spacing: 8
            Text { text: "Variable name"; color: "#e7e7e8" }
            BwTextField { id: variableName; width: parent.width; placeholderText: "score"; onAccepted: variableDialog.accept() }
        }
        onAccepted: {
            const name = variableName.text.trim();
            if (name.length) {
                if (renameTarget.length) { if (name !== renameTarget) root.invoke("rename_variable", { oldName: renameTarget, newName: name }); }
                else root.invoke("create_variable", { name: name });
            }
            renameTarget = ""; variableName.clear();
        }
    }
    BwDialog {
        id: listDialog
        property string renameTarget: ""
        title: renameTarget.length ? "Rename List" : "Make a List"
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openForCreate() { renameTarget = ""; listName.text = ""; open(); listName.forceActiveFocus(); }
        function openForRename(name) { renameTarget = name; listName.text = name; open(); listName.forceActiveFocus(); listName.selectAll(); }
        Column { width: 340; spacing: 8
            Text { text: "List name"; color: "#e7e7e8" }
            BwTextField { id: listName; width: parent.width; placeholderText: "items" }
        }
        onAccepted: {
            const name = listName.text.trim();
            if (!name.length) return;
            if (renameTarget.length) root.invoke("rename_list", { oldName: renameTarget, newName: name });
            else root.invoke("create_list", { name: name });
            renameTarget = ""; listName.clear();
        }
    }
    BwDialog {
        id: deleteListDialog
        title: "Delete list?"; standardButtons: Dialog.Yes | Dialog.Cancel
        property string listName: ""
        Text { text: "Delete “" + deleteListDialog.listName + "” and its saved items?"; color: "#e7e7e8" }
        onAccepted: root.invoke("delete_list", { name: listName })
    }
    BwDialog {
        id:detailsDialog;standardButtons:Dialog.NoButton;width:430;padding:0;topPadding:0;bottomPadding:0;showClose:false
        property string detailName:"";property string detailIdentifier:"";property string detailExplainer:""
        function show(name,identifier,explainer){detailName=name;detailIdentifier=identifier;detailExplainer=explainer;open()}
        contentItem:Column{spacing:12;padding:18
            Row{spacing:9;LucideIcon{name:"info";width:20;height:20;color:Theme.accent;anchors.verticalCenter:parent.verticalCenter}Text{text:detailsDialog.detailName;color:Theme.text;font.pixelSize:17;font.weight:Font.Bold;anchors.verticalCenter:parent.verticalCenter}}
            Rectangle{width:parent.width-36;height:30;radius:5;color:Theme.field;border.color:Theme.border;Text{anchors.centerIn:parent;text:detailsDialog.detailIdentifier;color:Theme.accent;font.family:"monospace";font.pixelSize:12}}
            Text{width:parent.width-36;text:detailsDialog.detailExplainer;color:Theme.textDim;font.pixelSize:13;wrapMode:Text.WordWrap}
            Row{anchors.right:parent.right;anchors.rightMargin:18;BwButton{text:"Close";onClicked:detailsDialog.close()}}
        }
    }
    MakeBlockDialog {
        id:blockDialog
        onCreateRequested:(pieces,shape,color,icon)=>root.invoke("create_block",{pieces:pieces,shape:shape,color:color,icon:icon})
        onEditRequested:(blockId,pieces,shape,color,icon)=>root.invoke("edit_block",{blockId:blockId,pieces:pieces,shape:shape,color:color,icon:icon})
    }

    // Blockwork's automation blocks, declared for blockstitch's generic rows.
    readonly property var extraOperators: [
        ["ReadFile", "text", 1, [textValue("~/notes.txt")]], ["FileExists", "bool", 1, [textValue("~/notes.txt")]],
        ["ListDirectory", "text", 1, [textValue("~")]], ["HttpGet", "text", 1, [textValue("https://example.com")]],
        ["JsonGet", "text", 2, [textValue("name"), textValue('{"name": "Blockwork"}')]],
        ["ShellOutput", "text", 1, [textValue("echo hello")]], ["ShellSucceeded", "bool", 1, [textValue("true")]],
        ["ActiveWindowTitle", "text", 0], ["ActiveWindowApp", "text", 0], ["WindowExists", "bool", 1, [textValue("Firefox")]],
        ["LastError", "text", 0], ["MacroRunning", "bool", 1, [textValue("")]], ["GlobalVariable", "text", 1, [textValue("counter")]],
        ["PixelColor", "text", 2, [numberValue(100), numberValue(100)]], ["AverageColor", "text", 4, [numberValue(0), numberValue(0), numberValue(100), numberValue(100)]],
        ["ColorsMatch", "bool", 3, [textValue("#ffffff"), textValue("#fafafa"), numberValue(10)]],
        ["ScreenText", "text", 4, [numberValue(0), numberValue(0), numberValue(400), numberValue(100)]],
        ["MousePosition", "number", 1, [textValue("x")]], ["ScreenSize", "number", 1, [textValue("width")]],
        ["MonitorCount", "number", 0], ["MonitorInfo", "number", 2, [numberValue(1), textValue("width")]],
        ["WindowInfo", "number", 2, [textValue("Firefox"), textValue("center x")]],
        ["Detection", "text", 2, [textValue("center x"), { kind: "Op", op: "ListItem", args: [numberValue(1), textValue("detections")], saved: numberValue(0) }]],
        ["AppRunning", "bool", 1, [textValue("firefox")]], ["IdleSeconds", "number", 0], ["ScreenLocked", "bool", 0],
        ["NetworkConnected", "bool", 0], ["UsbConnected", "bool", 1, [textValue("")]], ["Secret", "text", 1, [textValue("api token")]],
        ["TextCheck", "bool", 3, [textValue("hello world"), textValue("contains"), textValue("world")]],
        ["TextReplace", "text", 3, [textValue("world"), textValue("hello world"), textValue("there")]],
        ["RegexReplace", "text", 3, [textValue("[0-9]+"), textValue("order 1234"), textValue("#")]],
        ["RegexFind", "text", 2, [textValue("[0-9]+"), textValue("order 1234")]],
        ["SplitPart", "text", 3, [numberValue(1), textValue("a,b,c"), textValue(",")]],
        ["SplitCount", "number", 2, [textValue("a,b,c"), textValue(",")]],
        ["Substring", "text", 3, [numberValue(1), numberValue(3), textValue("hello")]],
        ["Trim", "text", 1, [textValue("  hello  ")]], ["FormatNumber", "text", 2, [numberValue(3.14159), numberValue(2)]],
        ["Timestamp", "number", 0], ["FormatDate", "text", 2, [{ kind: "Op", op: "Timestamp", args: [], saved: numberValue(0) }, textValue("%Y-%m-%d %H:%M")]],
        ["ParseDate", "number", 2, [textValue("2026-01-31 09:00"), textValue("")]],
        ["DatePart", "number", 2, [textValue("day"), { kind: "Op", op: "Timestamp", args: [], saved: numberValue(0) }]]
    ]
    readonly property var extraCategories: ({
        WriteFile: "system", HttpRequest: "system", Notify: "system", Log: "system",
        AskText: "system", AskYesNo: "system", AskChoice: "system", ShowMessage: "system", SendSignal: "control",
        TryCatch: "control", Retry: "control", ThrowError: "control", WithLock: "control", PasteText: "input",
        AppRunning: "system", IdleSeconds: "system", ScreenLocked: "system", NetworkConnected: "system", UsbConnected: "system", Secret: "data",
        WindowAction: "system", MoveWindow: "system", WindowExists: "system", StopMacro: "control", LastError: "control",
        RunMacro: "control", StopMacroNamed: "control", MacroRunning: "control", SetGlobal: "data", ChangeGlobal: "data", GlobalVariable: "data",
        WaitUntil: "control", FindImage: "screen", ClickImage: "screen", WaitForImage: "screen", PixelColor: "screen", AverageColor: "screen",
        ColorsMatch: "screen", ScreenText: "screen", MousePosition: "screen", ScreenSize: "screen", MonitorCount: "screen", MonitorInfo: "screen",
        DetectObjectsInWindow: "screen", WaitForDetection: "screen", AimStep: "screen", AimAtDetection: "screen", WindowInfo: "screen", Detection: "screen",
        ReadFile: "system", FileExists: "system", ListDirectory: "system", HttpGet: "system", JsonGet: "system",
        ShellOutput: "system", ShellSucceeded: "system", ActiveWindowTitle: "system", ActiveWindowApp: "system"
    })
    function variableChoices() {
        return [{ value: "", label: "(no variable)" }].concat((root.macro ? root.macro.variables : []).map(v => ({ value: v, label: v })));
    }
    readonly property var mouseButtonOptions: [["Left", "left button"], ["Right", "right button"], ["Middle", "middle button"],
        ["Back", "back button"], ["Forward", "forward button"], ["ScrollUp", "wheel up"], ["ScrollDown", "wheel down"],
        ["ScrollLeft", "wheel left"], ["ScrollRight", "wheel right"]].map(b => ({ value: b[0], label: b[1] }))
    // What the open macro may do without asking; null when it's trusted.
    readonly property var permissions: appState.permissions === undefined ? null : appState.permissions
    readonly property var guardedAbilities: [
        { kind: "commands", label: "Run commands and open or close apps" }, { kind: "network", label: "Go online" },
        { kind: "files_write", label: "Write to files" }, { kind: "secrets", label: "Read your saved secrets" },
        { kind: "screen", label: "Read the screen" }]
    // Which part of a window the window blocks match against.
    readonly property var windowMatchOptions: [{ value: "Title", label: "window" }, { value: "App", label: "app" }]
    // How a detect block orders its results list.
    readonly property var detectionSortOptions: [{ value: "Closest", label: "closest first" }, { value: "Score", label: "best first" },
        { value: "Largest", label: "largest first" }]
    readonly property string defaultDetector: "http://127.0.0.1:8765/detect"
    // Another macro's name, for the Run macro prefab.
    function otherMacroName() {
        const names = appState.macro_names || [];
        const mine = root.macro ? root.macro.name : "";
        return names.find(n => n && n !== mine) || "";
    }
    function registerBlocks() {
        const label = text => ({ kind: "label", text: text });
        // The image blocks' picture: click to capture from the screen, or load a file.
        const imagePieces = [
            { kind: "button", action: "captureImage", image: "image", icon: "crosshair", text: i => i && i.image ? "" : "capture image" },
            { kind: "button", action: "loadImage", icon: "folder-open" }
        ];
        const value = (field, key) => ({ kind: "value", field: field, key: key });
        // The detect blocks' search area: the whole window, or a part picked on screen.
        const regionPieces = [
            { kind: "button", action: "pickRegion", icon: "square-dashed", text: i => i && i.region ? "region " + i.region : "whole window" },
            { kind: "button", action: "clearRegion", icon: "x", when: i => !!i && !!i.region }
        ];
        BlockRegistry.registerOperators({
            ReadFile: { prefix: "contents of file", result: "text" }, FileExists: { prefix: "file", suffix: "exists?", result: "bool" },
            ListDirectory: { prefix: "files in folder", result: "text" }, HttpGet: { prefix: "fetch", result: "text" },
            JsonGet: { prefix: "get", infix: "from JSON", result: "text" },
            ShellOutput: { prefix: "output of command", result: "text" }, ShellSucceeded: { prefix: "command", suffix: "succeeds?", result: "bool" },
            ActiveWindowTitle: { prefix: "active window title", result: "text" }, ActiveWindowApp: { prefix: "active app", result: "text" },
            WindowExists: { prefix: "window", suffix: "exists?", result: "bool" },
            LastError: { prefix: "error message", result: "text" },
            MacroRunning: { prefix: "macro", suffix: "is running?", result: "bool" }, GlobalVariable: { prefix: "global", result: "text" },
            PixelColor: { prefix: "color at x", infix: ["y"], result: "text" },
            AverageColor: { prefix: "average color at x", infix: ["y", "width", "height"], result: "text" },
            ColorsMatch: { infix: ["matches", "within"], suffix: "%", result: "bool" },
            ScreenText: { prefix: "text on screen at x", infix: ["y", "width", "height"], result: "text" },
            MousePosition: { prefix: "mouse", result: "number", enumArg: { index: 0, options: [{ value: "x", label: "x" }, { value: "y", label: "y" }] } },
            ScreenSize: { prefix: "screen", result: "number", enumArg: { index: 0, options: [{ value: "width", label: "width" }, { value: "height", label: "height" }] } },
            MonitorCount: { prefix: "number of monitors", result: "number" },
            MonitorInfo: { prefix: "monitor", result: "number",
                enumArg: { index: 1, options: ["x", "y", "width", "height"].map(v => ({ value: v, label: v })) } },
            WindowInfo: { prefix: "window", result: "number",
                enumArg: { index: 1, options: ["x", "y", "width", "height", "center x", "center y"].map(v => ({ value: v, label: v })) } },
            Detection: { infix: "of detection", result: "text",
                enumArg: { index: 0, options: ["center x", "center y", "x", "y", "width", "height", "score", "label", "id"].map(v => ({ value: v, label: v })) } },
            AppRunning: { prefix: "app", suffix: "is running?", result: "bool" },
            IdleSeconds: { prefix: "seconds idle", result: "number" },
            ScreenLocked: { prefix: "screen is locked?", result: "bool" },
            NetworkConnected: { prefix: "online?", result: "bool" },
            UsbConnected: { prefix: "USB device", suffix: "plugged in?", result: "bool" },
            Secret: { prefix: "secret", result: "text", enumArg: { index: 0, options: () => {
                const names = appState.secret_names || [];
                return names.length ? names.map(n => ({ value: n, label: n })) : [{ value: "", label: "(add one in Settings)" }];
            } } },
            TextCheck: { infix: ["", ""], suffix: "?", result: "bool",
                enumArg: { index: 1, options: ["contains", "starts with", "ends with", "matches regex"].map(v => ({ value: v, label: v })) } },
            TextReplace: { prefix: "replace", infix: ["in", "with"], result: "text" },
            RegexReplace: { prefix: "replace regex", infix: ["in", "with"], result: "text" },
            RegexFind: { prefix: "match of regex", infix: "in", result: "text" },
            SplitPart: { prefix: "part", infix: ["of", "split by"], result: "text" },
            SplitCount: { prefix: "number of parts in", infix: "split by", result: "number" },
            Substring: { prefix: "letters", infix: ["to", "of"], result: "text" },
            Trim: { prefix: "trim", result: "text" },
            FormatNumber: { infix: "with", suffix: "decimals", result: "text" },
            Timestamp: { prefix: "timestamp now", result: "number" },
            FormatDate: { prefix: "date of", infix: "as", result: "text" },
            ParseDate: { prefix: "timestamp of", infix: "in format", result: "number" },
            DatePart: { infix: "of", result: "number",
                enumArg: { index: 0, options: ["year", "month", "day", "hour", "minute", "second", "weekday"].map(v => ({ value: v, label: v })) } }
        });
        BlockRegistry.registerRows({
            WhenAt: { shape: "header", icon: "clock", head: [label("ONCE AT"), value("AtTime", "at")] },
            WhenSignal: { shape: "header", icon: "corner-down-right", head: [label("WHEN I RECEIVE"), value("SignalName", "name")] },
            WhenWebhook: { shape: "header", icon: "link", head: [label("WHEN WEBHOOK"), value("WebhookPath", "path"), label("IS CALLED, BODY IN"),
                { kind: "dropdown", key: "variable", options: () => root.variableChoices(), placeholder: "(no variable)" }] },
            SendSignal: { icon: "navigation", head: [label("send signal"), value("SignalName", "name"),
                { kind: "dropdown", key: "wait", options: [{ value: "start", label: "and continue" }, { value: "wait", label: "and wait" }],
                  encode: v => v === "wait", decode: v => v ? "wait" : "start" }] },
            AskText: { icon: "text-cursor-input", head: [label("ask"), value("AskPrompt", "prompt"), label("default"), value("AskDefault", "default"),
                label("into"), { kind: "dropdown", key: "name", options: () => root.variableChoices(), placeholder: "(no variable)" }] },
            AskYesNo: { icon: "circle-check", head: [label("ask yes or no"), value("AskPrompt", "prompt"),
                label("into"), { kind: "dropdown", key: "name", options: () => root.variableChoices(), placeholder: "(no variable)" }] },
            AskChoice: { icon: "list-checks", head: [label("choose from"), { kind: "dropdown", key: "list", options: "lists" }, label("asking"),
                value("AskPrompt", "prompt"), label("into"), { kind: "dropdown", key: "name", options: () => root.variableChoices(), placeholder: "(no variable)" }] },
            ShowMessage: { icon: "message-square", head: [label("show message"), value("MessageText", "message")] },
            WhenWindow: { shape: "header", icon: "app-window", head: [label("WHEN WINDOW"), value("WindowPattern", "pattern"), label("IS"),
                { kind: "dropdown", key: "focused", options: [{ value: "focused", label: "focused" }, { value: "unfocused", label: "unfocused" }],
                  encode: v => v === "focused", decode: v => v ? "focused" : "unfocused" }] },
            WhenFileChanged: { shape: "header", icon: "file", head: [label("WHEN FILE"), value("WatchedFilePath", "path"), label("CHANGES")] },
            WhenTyped: { shape: "header", icon: "type", head: [label("WHEN I TYPE"), value("TypedText", "text"), label("THEN"),
                { kind: "dropdown", key: "erase", options: [{ value: "erase", label: "erase it" }, { value: "keep", label: "keep it" }],
                  encode: v => v === "erase", decode: v => v ? "erase" : "keep" }] },
            WhenApp: { shape: "header", icon: "app-window", head: [label("WHEN APP"), value("AppPattern", "pattern"),
                { kind: "dropdown", key: "started", options: [{ value: "starts", label: "starts" }, { value: "exits", label: "exits" }],
                  encode: v => v === "starts", decode: v => v ? "starts" : "exits" }] },
            WhenIdle: { shape: "header", icon: "moon", head: [label("WHEN IDLE FOR"), value("IdleMinutes", "minutes"), label("MINUTES")] },
            WhenScreenLock: { shape: "header", icon: "lock", head: [label("WHEN SCREEN IS"),
                { kind: "dropdown", key: "locked", options: [{ value: "locked", label: "locked" }, { value: "unlocked", label: "unlocked" }],
                  encode: v => v === "locked", decode: v => v ? "locked" : "unlocked" }] },
            WhenCron: { shape: "header", icon: "clock", head: [label("ON SCHEDULE"), value("CronExpression", "expression")] },
            WhenNetwork: { shape: "header", icon: "radio", head: [label("WHEN NETWORK"),
                { kind: "dropdown", key: "connected", options: [{ value: "on", label: "connects" }, { value: "off", label: "disconnects" }],
                  encode: v => v === "on", decode: v => v ? "on" : "off" }] },
            WhenUsb: { shape: "header", icon: "plug-zap", head: [label("WHEN USB DEVICE"), value("UsbPattern", "pattern"), label("IS"),
                { kind: "dropdown", key: "connected", options: [{ value: "in", label: "plugged in" }, { value: "out", label: "unplugged" }],
                  encode: v => v === "in", decode: v => v ? "in" : "out" }] },
            WhenMouseButton: { shape: "header", icon: "mouse", head: [label("WHEN"),
                { kind: "dropdown", key: "button", options: root.mouseButtonOptions },
                { kind: "dropdown", key: "pressed", options: [{ value: "down", label: "is pressed" }, { value: "up", label: "is released" }],
                  encode: v => v === "down", decode: v => v ? "down" : "up" },
                label("WITH KEYS"), value("MouseModifiers", "modifiers")] },
            SplitIntoList: { icon: "scissors", head: [label("split"), value("SplitText", "text"), label("by"), value("SplitSeparator", "separator"),
                label("into"), { kind: "dropdown", key: "name", options: "lists" }] },
            PasteText: { icon: "clipboard-paste", head: [label("paste"), value("PasteValue", "text")] },
            WithLock: { icon: "lock", mouths: ["body"], head: [label("with lock"), value("LockName", "name")] },
            WriteFile: { icon: "file-text", head: [
                { kind: "dropdown", key: "append", options: [{ value: "write", label: "write" }, { value: "append", label: "append" }],
                  encode: v => v === "append", decode: v => v ? "append" : "write" },
                value("WriteFileText", "text"), label("to file"), value("WriteFilePath", "path")] },
            // Target first, like "set", so it isn't lost at the end of a long row.
            HttpRequest: { icon: "cloud", head: [
                label("set"), { kind: "dropdown", key: "variable", options: () => root.variableChoices(), placeholder: "(no variable)" },
                label("to response of"), { kind: "dropdown", key: "method", options: ["GET", "POST", "PUT", "PATCH", "DELETE"].map(m => ({ value: m, label: m })) },
                value("HttpUrl", "url"),
                Object.assign(label("with body"), { when: i => !i || i.method !== "GET" }), Object.assign(value("HttpBody", "body"), { when: i => !i || i.method !== "GET" }),
                label("headers"), value("HttpHeaders", "headers")] },
            Notify: { icon: "message-square", head: [label("notify"), value("NotifyTitle", "title"), label("saying"), value("NotifyMessage", "message")] },
            Log: { icon: "list", head: [label("log"), value("LogValue", "value")] },
            TryCatch: { icon: "circle-alert", mouths: ["body", "catch_body"], separators: ["on error"], head: [label("try")] },
            ThrowError: { shape: "cap", icon: "triangle-alert", head: [label("throw error"), value("ErrorMessage", "message")] },
            StopMacro: { shape: "cap", icon: "octagon", head: [label("stop macro")] },
            Retry: { icon: "refresh-cw", mouths: ["body"], head: [label("try up to"), value("RetryCount", "count"), label("times,"),
                value("RetryDelay", "delay"), label("ms apart")] },
            WindowAction: { icon: "app-window", head: [
                { kind: "dropdown", key: "action", options: ["Focus", "Minimize", "Maximize", "Restore", "Close"].map(a => ({ value: a, label: a.toLowerCase() })) },
                { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern")] },
            MoveWindow: { icon: "move", head: [
                { kind: "dropdown", key: "resize", options: [{ value: "move", label: "move" }, { value: "resize", label: "resize" }],
                  encode: v => v === "resize", decode: v => v ? "resize" : "move" },
                { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern"),
                Object.assign(label("to x"), { when: i => !i || !i.resize }), Object.assign(label("to width"), { when: i => !!i && i.resize }),
                value("WindowX", "x"),
                Object.assign(label("y"), { when: i => !i || !i.resize }), Object.assign(label("height"), { when: i => !!i && i.resize }),
                value("WindowY", "y")] },
            RunMacro: { icon: "play", head: [label("run macro"), value("MacroTarget", "target"),
                { kind: "dropdown", key: "wait", options: [{ value: "start", label: "and continue" }, { value: "wait", label: "and wait" }],
                  encode: v => v === "wait", decode: v => v ? "wait" : "start" }] },
            StopMacroNamed: { icon: "square", head: [label("stop macro"), value("MacroTarget", "target")] },
            SetGlobal: { icon: "variable", head: [label("set global"), value("GlobalName", "name"), label("to"), value("GlobalValue", "value")] },
            ChangeGlobal: { icon: "variable", head: [label("change global"), value("GlobalName", "name"), label("by"), value("GlobalValue", "by")] },
            WaitUntil: { icon: "loader", head: [label("wait until"), { kind: "value", field: "Condition", key: "condition", bool: true },
                label("for up to"), value("WaitTimeout", "timeout"), label("ms")] },
            FindImage: { icon: "search", head: [label("find"), ...imagePieces, label("put x in"),
                { kind: "dropdown", key: "x_variable", options: () => root.variableChoices(), placeholder: "(no variable)" },
                label("y in"), { kind: "dropdown", key: "y_variable", options: () => root.variableChoices(), placeholder: "(no variable)" },
                label("within"), value("ImageTolerance", "tolerance"), label("%")] },
            ClickImage: { icon: "mouse-pointer-click", head: [label("click"),
                { kind: "dropdown", key: "button", options: [{ value: "Left", label: "left" }, { value: "Right", label: "right" }, { value: "Middle", label: "middle" }] },
                label("on"), ...imagePieces, label("within"), value("ImageTolerance", "tolerance"), label("%")] },
            WaitForImage: { icon: "eye", head: [label("wait until"), ...imagePieces,
                { kind: "dropdown", key: "appear", options: [{ value: "appears", label: "appears" }, { value: "gone", label: "disappears" }],
                  encode: v => v === "appears", decode: v => v ? "appears" : "gone" },
                label("for up to"), value("WaitTimeout", "timeout"), label("ms, within"), value("ImageTolerance", "tolerance"), label("%")] },
            DetectObjectsInWindow: { icon: "aperture", head: [label("detect"), value("DetectLabel", "label"), label("in"),
                { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern"), ...regionPieces,
                label("with"), value("DetectorUrl", "detector"), label("at least"), value("DetectConfidence", "confidence"), label("% into"),
                { kind: "dropdown", key: "results_list", options: "lists" },
                { kind: "dropdown", key: "sort", options: root.detectionSortOptions },
                label("count in"), { kind: "dropdown", key: "count_variable", options: () => root.variableChoices(), placeholder: "(no variable)" }] },
            WaitForDetection: { icon: "eye", head: [label("wait until"), value("DetectLabel", "label"),
                { kind: "dropdown", key: "appear", options: [{ value: "appears", label: "appears" }, { value: "gone", label: "disappears" }],
                  encode: v => v === "appears", decode: v => v ? "appears" : "gone" },
                label("in"), { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern"), ...regionPieces,
                label("with"), value("DetectorUrl", "detector"), label("at least"), value("DetectConfidence", "confidence"),
                label("% for up to"), value("WaitTimeout", "timeout"), label("ms")] },
            AimStep: { icon: "crosshair", head: [label("aim from center of"), { kind: "dropdown", key: "by", options: root.windowMatchOptions },
                value("WindowPattern", "pattern"), label("toward x"), value("AimTargetX", "target_x"), label("y"), value("AimTargetY", "target_y"),
                label("gain"), value("AimGain", "gain"), label("max step"), value("AimMaxStep", "max_step"),
                label("deadzone"), value("AimDeadzone", "deadzone")] },
            AimAtDetection: { icon: "target", head: [label("aim at item"), value("AimIndex", "index"), label("of"),
                { kind: "dropdown", key: "results_list", options: "lists" },
                label("gain"), value("AimGain", "gain"), label("max step"), value("AimMaxStep", "max_step"),
                label("deadzone"), value("AimDeadzone", "deadzone"), label("hold"), value("AimSettle", "settle_ms"),
                label("ms, up to"), value("WaitTimeout", "timeout"), label("ms, result in"),
                { kind: "dropdown", key: "status_variable", options: () => root.variableChoices(), placeholder: "(no variable)" }] }
        });
        BlockRegistry.registerPrefabs({
            WhenAt: () => ({ at: textValue(Qt.formatDateTime(new Date(Date.now() + 3600000), "yyyy-MM-dd HH:00")) }),
            WhenSignal: { name: textValue("go") },
            WhenWebhook: () => ({ path: textValue("hello"), variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            SendSignal: { name: textValue("go"), wait: true },
            AskText: () => ({ prompt: textValue("What's your name?"), default: textValue(""), name: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            AskYesNo: () => ({ prompt: textValue("Continue?"), name: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            AskChoice: () => ({ prompt: textValue("Pick one"), list: root.firstListName(), name: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            ShowMessage: { message: textValue("Done!") },
            WhenWindow: { pattern: textValue("Firefox"), focused: true },
            WhenFileChanged: { path: textValue("~/notes.txt") },
            WhenTyped: { text: textValue(";addr"), erase: true },
            WhenApp: { pattern: textValue("firefox"), started: true },
            WhenIdle: { minutes: numberValue(5) },
            WhenScreenLock: { locked: true },
            WhenCron: { expression: textValue("0 9 * * mon-fri") },
            WhenNetwork: { connected: true },
            WhenUsb: { pattern: textValue(""), connected: true },
            WhenMouseButton: { button: "Back", pressed: true, modifiers: textValue("") },
            SplitIntoList: () => ({ text: textValue("a,b,c"), separator: textValue(","), name: root.firstListName() }),
            PasteText: { text: textValue("Hello!") },
            WithLock: { name: textValue("keyboard"), body: [] },
            WriteFile: { path: textValue("~/notes.txt"), text: textValue("hello"), append: true },
            HttpRequest: () => ({ method: "GET", url: textValue("https://example.com"), body: textValue(""), headers: textValue(""),
                variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            Notify: { title: textValue("Blockwork"), message: textValue("Done!") },
            Log: { value: textValue("checkpoint") },
            TryCatch: { body: [], catch_body: [] },
            ThrowError: { message: textValue("something went wrong") },
            StopMacro: {},
            Retry: { count: numberValue(3), delay: numberValue(1000), body: [] },
            WindowAction: { action: "Focus", by: "Title", pattern: textValue("Firefox") },
            MoveWindow: { pattern: textValue("Firefox"), by: "Title", resize: false, x: numberValue(0), y: numberValue(0) },
            RunMacro: () => ({ target: textValue(root.otherMacroName()), wait: true }),
            StopMacroNamed: () => ({ target: textValue(root.otherMacroName()) }),
            SetGlobal: { name: textValue("counter"), value: numberValue(0) },
            ChangeGlobal: { name: textValue("counter"), by: numberValue(1) },
            WaitUntil: { condition: { kind: "Bool" }, timeout: numberValue(0) },
            FindImage: () => ({ image: "", tolerance: numberValue(10), x_variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "",
                y_variable: root.macro && root.macro.variables.length > 1 ? root.macro.variables[1] : "" }),
            ClickImage: { image: "", tolerance: numberValue(10), button: "Left" },
            WaitForImage: { image: "", tolerance: numberValue(10), appear: true, timeout: numberValue(10000) },
            DetectObjectsInWindow: () => ({ pattern: textValue("Minecraft"), by: "Title", detector: textValue(root.defaultDetector),
                label: textValue("player"), confidence: numberValue(50), sort: "Closest", results_list: root.firstListName(), region: "",
                count_variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            WaitForDetection: { pattern: textValue("Minecraft"), by: "Title", detector: textValue(root.defaultDetector),
                label: textValue("player"), confidence: numberValue(50), appear: true, timeout: numberValue(10000), region: "" },
            AimStep: { pattern: textValue("Minecraft"), by: "Title", target_x: numberValue(0), target_y: numberValue(0),
                gain: numberValue(0.4), max_step: numberValue(40), deadzone: numberValue(6) },
            AimAtDetection: () => ({ results_list: root.firstListName(), index: numberValue(1), gain: numberValue(0.4),
                max_step: numberValue(40), deadzone: numberValue(6), settle_ms: numberValue(150), timeout: numberValue(3000),
                status_variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" })
        });
    }
    // The variable dropdowns read macro state through a function, so nudge them.
    readonly property string variablesKey: JSON.stringify([root.macro ? root.macro.variables : [], appState.secret_names || []])
    onVariablesKeyChanged: BlockRegistry.revision++

    // ---- run log ----
    readonly property var runLog: appState.run_log || []
    // Blocks that failed since this macro last started or fired get a marker.
    readonly property var blockMarks: {
        const id = root.macro ? root.macro.id : "", out = {};
        for (let i = runLog.length - 1; i >= 0; --i) {
            const e = runLog[i];
            if (e.macro_id !== id) continue;
            if (e.event === "Started" || e.event === "Fired") break;
            if (e.level === "Error" && e.block_id && !(e.block_id in out)) out[e.block_id] = e.message;
        }
        return out;
    }
    Binding { target: BlockRegistry; property: "blockMarks"; value: root.blockMarks }
    // Blocks this macro's running strands are on, outlined live.
    readonly property var runningIds: {
        const id = root.macro ? root.macro.id : "";
        let active = [];
        try { active = JSON.parse(root.appBridge.activeBlocksJson || "[]"); } catch (e) {}
        return active.filter(b => b.macro_id === id).map(b => b.block_id);
    }
    Binding { target: BlockRegistry; property: "runningIds"; value: root.runningIds }
    // Every macro with a strand running, for the library.
    readonly property var runningMacroIds: {
        let active = [];
        try { active = JSON.parse(root.appBridge.activeBlocksJson || "[]"); } catch (e) {}
        return active.map(b => b.macro_id).filter((id, i, all) => all.indexOf(id) === i);
    }
    Binding { target: BlockRegistry; property: "breakpointIds"; value: appState.breakpoints || [] }
    Binding { target: BlockRegistry; property: "pausedIds"; value: debugBar.paused.map(p => p.block_id) }
    Connections {
        target: BlockRegistry
        function onMarkClicked(instructionId) { activity.showBlockError(instructionId); }
        function onBreakpointClicked(instructionId) { root.invoke("toggle_breakpoint", { blockId: instructionId }); }
    }
    // A block's palette name, for the debugger bar.
    function findInstruction(id) {
        const walk = list => {
            for (const i of list || []) {
                if (i.id === id) return i;
                for (const key of ["body", "then_body", "else_body"]) { const found = walk(i[key]); if (found) return found; }
                for (const branch of i.branches || []) { const found = walk(branch); if (found) return found; }
            }
            return null;
        };
        for (const strand of root.macro ? root.macro.strands : []) { const found = walk(strand.instructions); if (found) return found; }
        return null;
    }
    function blockLabel(id) {
        const ins = findInstruction(id);
        if (!ins) return "a block";
        if (ins.type === "CallBlock" || ins.type === "BranchCallBlock") { const def = blockDefById(ins.block_id); return def ? "custom block" : "a block"; }
        return ins.type.replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase();
    }

    // Click-to-preview on operators: the daemon evaluates, the chip shows the answer.
    Component.onCompleted: {
        registerBlocks();
        BlockRegistry.blockMenu = [{ id: "breakpoint", text: "Toggle breakpoint", icon: "circle-dot" }];
        BlockRegistry.previewHandler = (key, value) => root.appBridge.previewValue(key, JSON.stringify(value));
    }
    Connections {
        target: root.appBridge
        function onPreviewSerialChanged() { BlockRegistry.deliverPreview(root.appBridge.previewKey, root.appBridge.previewText, root.appBridge.previewError); }
    }
    AppSelectorDialog {
        id: appSelector
        bridge: root.appBridge
        onSelected: app => root.applyPickedApp(app)
    }

    // Follows the pointer while a palette entry is dragged.
    Item {
        id: ghostLayer
        anchors.fill: parent; z: 500; enabled: false
        // Canvas drags are clipped by the canvas, so redraw them here while over the sidebar.
        Item {
            id: canvasGhost
            readonly property bool overSidebar: canvas.dragging && !root.sidebarCollapsed
                && palette.contains(palette.mapFromItem(null, canvas.dragSceneX, canvas.dragSceneY))
            visible: overSidebar
            property point pos: overSidebar ? ghostLayer.mapFromItem(null, canvas.dragGhostX, canvas.dragGhostY) : Qt.point(0, 0)
            x: pos.x; y: pos.y; opacity: 0.92
            Column {
                visible: !canvas.dragIsValue
                spacing: -8; scale: canvas.zoom; transformOrigin: Item.TopLeft
                Repeater {
                    model: canvasGhost.overSidebar && !canvas.dragIsValue ? canvas.dragInstructions : []
                    delegate: InstructionBlock {
                        required property var modelData
                        instruction: modelData; paletteMode: true; locked: true
                        variables: root.macro ? root.macro.variables : []; lists: root.macro && root.macro.lists ? root.macro.lists : []
                        blockDefinitions: root.macro ? root.macro.block_defs : []
                    }
                }
            }
            Loader {
                active: canvasGhost.overSidebar && canvas.dragIsValue
                scale: canvas.zoom; transformOrigin: Item.TopLeft
                sourceComponent: ValueChip {
                    valueData: canvas.dragValue; boxed: true; editable: false
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                }
            }
        }
        Item {
            id: ghost
            property var spec: null
            visible: false; opacity: 0.92
            width: ghostBlock.active ? ghostBlock.item.width : (ghostValue.active ? ghostValue.item.width : 0)
            height: ghostBlock.active ? ghostBlock.item.height : (ghostValue.active ? ghostValue.item.height : 0)
            Loader {
                id: ghostBlock
                active: ghost.visible && !!ghost.spec && ghost.spec.kind !== "value"
                sourceComponent: InstructionBlock {
                    instruction: ghost.spec.instruction; paletteMode: true; locked: true
                    blockColor: ghost.spec.color || Theme.block
                    variables: root.macro ? root.macro.variables : []; lists: root.macro && root.macro.lists ? root.macro.lists : []
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                }
            }
            Loader {
                id: ghostValue
                active: ghost.visible && !!ghost.spec && ghost.spec.kind === "value"
                sourceComponent: ValueChip {
                    valueData: ghost.spec.value; boxed: true; editable: false
                    forceBoolean: !!ghost.spec.forceBoolean; callDisplayLabel: ghost.spec.label || "custom block"
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                }
            }
        }
    }
}
