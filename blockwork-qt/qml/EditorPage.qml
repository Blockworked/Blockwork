import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import BlockworkStyle as Editing
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke
    required property var appBridge
    // Shared import/export flow (MacroFiles in Main.qml).
    property var macroFiles: null
    property var blockLibrary: null
    readonly property var macro: appState.current_macro
    readonly property bool recording: appState.recording_phase && appState.recording_phase.phase === "Active"
    readonly property bool recordingCountdown: !!appState.recording_phase && appState.recording_phase.phase === "Countdown"
    // Counting down or recording: the canvas stays locked for both.
    readonly property bool recordingBusy: recording || recordingCountdown
    property string sidebarKey: "a"
    property real sidebarWidth: 348
    property bool sidebarCollapsed: false
    property bool sidebarToggling: false
    readonly property real minSidebarWidth: 180
    readonly property real maxSidebarWidth: 600
    readonly property var instructionTypes: ["WhenRan", "WhenBatteryDischargedTo", "WhenBatteryChargedTo", "WhenTime", "WhenPowerPluggedIn", "WhenPowerUnplugged", "WhenClipboardChanged", "WhenKey", "WhenWindow", "WhenFileChanged", "WhenTyped", "WhenApp", "WhenIdle", "WhenScreenLock", "WhenCron", "WhenNetwork", "WhenUsb", "WhenMouseButton", "WhenAt", "WhenSignal", "WhenWebhook", "Wait", "Text", "PasteText", "Key", "Button", "MoveMouse", "Scroll", "Command", "CommandWait", "OpenApp", "CloseApp", "WindowAction", "MoveWindow", "WriteFile", "HttpRequest", "Notify", "AskText", "AskYesNo", "AskChoice", "ShowMessage", "Log", "SetVariable", "ChangeVariable", "SetClipboard", "Return", "If", "IfElse", "Repeat", "Forever", "While", "EscapeLoop", "ContinueLoop", "TryCatch", "Retry", "ThrowError", "StopMacro", "WithLock", "SendSignal", "RunMacro", "StopMacroNamed", "SetGlobal", "ChangeGlobal", "WaitUntil", "FindImage", "ClickImage", "WaitForImage", "DetectObjectsInWindow", "WaitForDetection", "AimStep", "AimAtDetection"]
    readonly property var listInstructionTypes: ["AddToList", "DeleteOfList", "DeleteAllOfList", "ShiftList", "InsertIntoList", "ReplaceItemOfList", "ReverseList", "SplitIntoList"]

    function uuid() { return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, c => { const r = Math.random() * 16 | 0; return (c === "x" ? r : (r & 3 | 8)).toString(16); }); }
    function numberValue(n) { return { kind: "Number", value: n }; }
    function textValue(t) { return { kind: "Text", value: t }; }
    function firstGlobalName() { return (appState.globals || []).length ? appState.globals[0].name : ""; }
    function defaultInstruction(type) {
        const registered = BlockRegistry.prefab(type);
        if (registered) { registered.id = uuid(); return registered; }
        const i = { id: uuid(), type: type };
        if (type === "WhenBatteryDischargedTo") i.threshold = numberValue(20);
        else if (type === "WhenBatteryChargedTo") i.threshold = numberValue(100);
        else if (type === "WhenTime") { i.schedule = { kind: "Daily" }; i.time = textValue("09:00"); }
        else if (type === "Wait") i.duration = numberValue(1000);
        else if (type === "Text") i.text = textValue("text");
        else if (type === "Key") { i.key = textValue(root.sidebarKey); i.direction = "Click"; }
        else if (type === "WhenKey") { i.key = textValue(root.sidebarKey); i.direction = "Press"; }
        else if (type === "Button") { i.button = "Left"; i.direction = "Click"; }
        else if (type === "MoveMouse") { i.x = numberValue(0); i.y = numberValue(0); i.coordinate = "Relative"; }
        else if (type === "Scroll") { i.amount = numberValue(4); i.axis = "Vertical"; }
        else if (type === "Command" || type === "CommandWait") i.command = "";
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
        const instruction = palette.customInstruction(definition);
        instruction.id = uuid();
        root.invoke("add_strand", { x: x, y: y, instruction: instruction });
    }
    function recordingLabel() {
        const phase = appState.recording_phase || { phase: "Idle" };
        if (phase.phase === "Countdown") return qsTr("Cancel (%1)").arg(phase.countdown);
        if (phase.phase === "Active") return qsTr("Stop");
        return qsTr("Record");
    }
    function recordingTooltip() {
        if (root.recordingCountdown) return qsTr("Cancel recording");
        if (root.recording) return qsTr("Stop recording");
        return qsTr("Record keys and clicks into this macro");
    }
    // The Stop Recording hotkey as shown in Settings, or "" when it isn't set.
    function stopRecordingShortcut() {
        const b = (appState.hotkey_bindings || []).find(b => b.action && b.action.type === "StopRecording");
        return b ? b.combo_display : "";
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
        const x={WhenAt:qsTr("Runs once, at a date and time like 2026-12-24 18:00 (local time). Click it to pick the day from a calendar, or drop a reporter that gives one. Turn on 'catch up on missed schedules' in Macro settings to still run it if Blockwork was closed or the computer was asleep at that moment."),WhenSignal:qsTr("Runs when a 'send signal' block in this macro sends the same name. It shares that run's variables, so one strand can hand work to others."),SendSignal:qsTr("Starts this macro's 'when I receive' strands with the same name. With 'and wait' this strand pauses until they've all finished: a way to wait for other strands."),WhenWebhook:qsTr("Runs when a web request reaches Blockwork's webhook server at this path. Turn the server on in Settings > Integrations, where its address and token are. The request body goes into the variable."),AskText:qsTr("Shows a dialog asking for text and puts the answer in the variable. Cancel stops the run, unless it's inside a try block, where it's an error you can catch."),AskYesNo:qsTr("Shows a yes/no question and puts true or false in the variable."),AskChoice:qsTr("Shows the items of a list and puts the one you pick in the variable. Cancel stops the run, unless it's inside a try block."),ShowMessage:qsTr("Shows a message in a dialog and waits until you close it."),WhenNetwork:qsTr("Runs when the computer goes online or offline: when a network route to the internet appears or goes away. Checked once a second."),WhenUsb:qsTr("Runs when a USB device is plugged in or unplugged. The text matches the device's name or its vendor:product id (like 046d:c52b); leave it empty for any device. Hubs don't count."),WhenMouseButton:qsTr("Runs when a mouse button is pressed or released, or the wheel scrolls. Put modifier keys in the box (like Control+Shift) to only run while exactly those are held; leave it empty to ignore modifiers. The click still goes through to the app. On Windows, clicks in programs running as administrator are only seen when Blockwork runs as administrator too."),SplitIntoList:qsTr("Cuts the text at every separator and puts the pieces in the list, replacing what was there. An empty separator splits it into letters."),Secret:qsTr("The value of a secret saved in Settings > Secrets. It comes from your system keyring, isn't stored in the macro file, and shows as dots in the run log."),NetworkConnected:qsTr("Whether the computer has a network route to the internet."),UsbConnected:qsTr("Whether a USB device whose name or vendor:product id contains the text is plugged in. Empty matches any device."),TextCheck:qsTr("Checks text: contains, starts with, ends with, or matches a regular expression. Case matters."),TextReplace:qsTr("Replaces every copy of the first text inside the second with the third."),RegexReplace:qsTr("Replaces every match of a regular expression. Use $1, $2 in the replacement for groups."),RegexFind:qsTr("The first match of a regular expression, or its first group if it has one. Empty when nothing matches."),SplitPart:qsTr("Cuts the text at every separator and gives one piece, counting from 1. Negative numbers count from the end."),SplitCount:qsTr("How many pieces the text has when cut at every separator."),Substring:qsTr("The letters between two positions, counting from 1, both included."),Trim:qsTr("The text without spaces or line breaks at its start and end."),FormatNumber:qsTr("The number as text with exactly this many digits after the point."),Timestamp:qsTr("The current time as a Unix timestamp: seconds since 1970. Add 86400 for a day later."),FormatDate:qsTr("Writes a timestamp as a date in local time. Codes: %Y year, %m month, %d day, %H hour, %M minute, %S second, %A weekday name, %B month name."),ParseDate:qsTr("The timestamp of a date written as text, in local time. Leave the format empty for dates like 2026-01-31 09:00, or use codes like %d.%m.%Y."),DatePart:qsTr("One part of a timestamp in local time. Weekday is 1 for Sunday to 7 for Saturday."),WhenTyped:qsTr("Text expansion: runs when you type the text anywhere, like ;addr. With 'erase it' the typed text is deleted first, so a paste block can replace it. Uses a US keyboard layout for symbols."),PasteText:qsTr("Pastes text with Ctrl+V (Cmd+V on macOS), then puts back the text that was on the clipboard. Faster and more reliable than typing for long text. Terminals that paste with Ctrl+Shift+V need a Key block instead."),WhenApp:qsTr("Runs when a program whose name contains the text starts or exits. Checked once a second."),WhenIdle:qsTr("Runs once you haven't touched the keyboard or mouse for this many minutes. Fires again after the next idle stretch."),WhenScreenLock:qsTr("Runs when the screen locks or unlocks."),WhenCron:qsTr("Runs on a schedule, like every 15 minutes on weekdays or 9:00 on the 1st of each month. Click the schedule to choose it. Underneath it's cron (minute hour day month weekday, like '0 9 * * mon-fri'), so you can also type one under Custom or drop a reporter that gives one."),WithLock:qsTr("Runs its body while holding a named lock. Only one strand, in any macro, can be inside a lock with the same name at a time; others wait their turn. Use it so two macros don't type over each other."),AppRunning:qsTr("Whether a program whose name contains the text is running."),IdleSeconds:qsTr("How many seconds since the last keyboard or mouse input."),ScreenLocked:qsTr("Whether the screen is locked."),WaitUntil:qsTr("Pauses until the condition is true, checking about 10 times a second. If it's still false after the time limit it's an error; 0 waits forever."),FindImage:qsTr("Looks for the picture on screen and puts the screen position of its middle in the two variables, or -1 if it isn't there. Click the picture area to capture one. Tolerance is how different the pixels may be, in percent. On Windows, games in exclusive fullscreen capture as black: switch them to borderless windowed."),ClickImage:qsTr("Finds the picture on screen and clicks its middle. It's an error if the picture isn't there. Click the picture area to capture one."),WaitForImage:qsTr("Pauses until the picture appears on screen (or goes away). If that hasn't happened within the time limit it's an error; 0 waits forever."),DetectObjectsInWindow:qsTr("Captures the window and finds the objects in it, with a built-in model (the name of a .onnx file in Blockwork's models folder, or a path to one) or a detector running on this computer (a local http:// address). Each one becomes a list item, ordered best, closest to the window's center, or largest first; read them with 'detection of'. Objects keep their id from one run of this block to the next. Click the region button to search only part of the window; the aim still uses the window's center. On Windows, games in exclusive fullscreen capture as black: switch them to borderless windowed. Only use it where automation is allowed, never against other players."),WaitForDetection:qsTr("Pauses until the detector sees the label in the window (or stops seeing it). If that hasn't happened within the time limit it's an error; 0 waits forever. Only use it where automation is allowed, never against other players."),AimStep:qsTr("Asks once per session before it moves the mouse. Moves the mouse one small step from the window's center toward a screen point. Games that lock the pointer turn the view instead, so repeat it with a fresh detection each time. Gain is mouse movement per pixel off target in a 1000 pixel tall window (it's scaled to the window's height): tune it to your sensitivity; about 0.5 suits Minecraft's default. Only use it where automation is allowed, never against other players."),AimAtDetection:qsTr("Asks once per session before it moves the mouse. Follows one item of a detections list: detects again and again while gliding the mouse toward it like a hand would, until it's held at the window's center for the hold time. Gain works as in the aim step block. Puts hit, timeout or lost in the variable. Fill the list with a detect block first. Only use it where automation is allowed, never against other players."),WindowInfo:qsTr("The position or size of a window, or its center (where games draw the crosshair), in screen coordinates."),Detection:qsTr("One part of a detection from a detect block's list: its box, center, score (0-100), label or tracking id. An item that isn't a detection is an error."),PixelColor:qsTr("The color of the screen at a point, as #rrggbb. Use the screen picker in the toolbar to find coordinates."),AverageColor:qsTr("The average color of a screen region, as #rrggbb."),ColorsMatch:qsTr("Whether two colors are within the given percentage of each other (0 = identical, 100 = black vs white)."),ScreenText:qsTr("Reads the text in a screen region (OCR). Uses Windows or macOS text recognition, or Tesseract on Linux. Windows reads the languages added in its Language settings."),MousePosition:qsTr("Where the mouse pointer is, in screen coordinates."),ScreenSize:qsTr("The size of the whole desktop, across all monitors."),MonitorInfo:qsTr("The position or size of a monitor, counting from 1."),MonitorCount:qsTr("How many monitors are connected."),RunMacro:qsTr("Starts another macro by name. With 'and wait' this strand pauses until that run ends; stopping this run stops it too."),StopMacroNamed:qsTr("Stops every run of the named macro."),SetGlobal:qsTr("Sets a global variable. Globals are shared by every macro and kept after Blockwork restarts."),ChangeGlobal:qsTr("Adds a number to a global variable. A global that was never set counts as 0."),Retry:qsTr("Runs its body. If a block in it fails, waits and runs the whole body again, up to the given number of attempts. After the last failure it's an error, like any other."),WindowAction:qsTr("Focuses, minimizes, maximizes, restores or closes the first window whose title (window) or app name (app) contains the text. Leave the text empty for the active window. On Windows it can only control programs running as administrator when Blockwork runs as administrator too."),MoveWindow:qsTr("Moves a window to a screen position, or resizes it, in pixels. Picks the first window whose title (window) or app name (app) contains the text; leave it empty for the active window. On Hyprland and Sway the window becomes floating. On Windows it can only move programs running as administrator when Blockwork runs as administrator too."),WhenWindow:qsTr("Runs when a window whose title or app name contains the text gains or loses focus. Leave the text empty to match any window."),WhenFileChanged:qsTr("Runs when the file is created, changed or deleted. Checked once a second."),WriteFile:qsTr("Writes text to a file, replacing it, or appends to the end. ~ means your home folder."),HttpRequest:qsTr("Sends a web request and stores the response in a variable. Headers are one 'Name: value' per line. A 4xx or 5xx response is an error."),Notify:qsTr("Shows a desktop notification."),Log:qsTr("Adds a line to the run log."),TryCatch:qsTr("Runs the first part. If a block in it fails, stops there and runs the 'on error' part, where 'error message' says what went wrong."),ThrowError:qsTr("Fails with this message. Ends the strand unless a try block catches it."),StopMacro:qsTr("Stops every strand of this run."),WhenRan:qsTr("Runs this strand when the macro starts, from the Run button or the macro's hotkey. A macro can have several, and they all run side by side."),
            WhenBatteryDischargedTo:qsTr("Runs when the battery drops to this percentage or below. It won't run again until the battery charges back above it and drops again."),
            WhenBatteryChargedTo:qsTr("Runs when the battery charges to this percentage or above. It won't run again until the battery drops back below it and charges again."),
            WhenTime:qsTr("Runs at a time of day, every day, week, month or year (local time). Click the time to pick it, or drop a reporter there that gives a time like 09:30. A monthly day that a month doesn't have, like the 31st, skips that month."),
            WhenPowerPluggedIn:qsTr("Runs when the computer starts charging from external power."),
            WhenPowerUnplugged:qsTr("Runs when the computer goes onto battery power. Never runs on a computer without a battery."),
            WhenClipboardChanged:qsTr("Runs whenever the clipboard changes, by you or by another program. Read the new contents with the clipboard text block."),
            WhenKey:qsTr("Runs when a key or key combo (like Control+Shift+K) is pressed or released, in any app. Click counts as press."),
            Wait:qsTr("Pauses this strand for the given number of milliseconds (1000 is one second). Other strands keep running."),
            Text:qsTr("Types the text as keystrokes into whatever app has focus. For long text, a paste block is faster and more reliable."),
            Key:qsTr("Presses, releases or clicks (press then release) a key or key combo like Control+C. Use press and release separately to hold a key down while other blocks run."),
            Button:qsTr("Presses, releases or clicks a mouse button where the pointer is now. Pair it with a move mouse block to click somewhere specific."),
            MoveMouse:qsTr("Moves the mouse pointer. Absolute goes to that screen position; relative moves it by that many pixels from where it is. Use the screen picker to find coordinates."),
            Scroll:qsTr("Turns the mouse wheel by this many notches. Positive scrolls up (or right), negative down (or left)."),
            Command:qsTr("Starts a shell command (bash) and moves on straight away, while the command keeps running. Use 'run command and wait' to wait for it to finish."),
            CommandWait:qsTr("Runs a shell command (bash) and waits for it to finish before moving on. Its output is thrown away; use the shell output reporter to read what it prints."),
            OpenApp:qsTr("Launches an installed app. Click it to choose the app."),
            CloseApp:qsTr("Closes a running app. Click it to choose the app."),
            SetVariable:qsTr("Sets a variable to a value, replacing what it held. Variables belong to this run of the macro and start at 0."),
            ChangeVariable:qsTr("Adds a number to a variable (use a negative number to subtract). A variable that isn't a number counts as 0."),
            SetClipboard:qsTr("Replaces the clipboard with this text, as if you'd copied it."),
            AddToList:qsTr("Adds an item to the end of the list."),
            DeleteOfList:qsTr("Removes the item at a position from the list, counting from 1. Later items move up one."),
            DeleteAllOfList:qsTr("Removes every item from the list."),
            ShiftList:qsTr("Rotates the list by this many places: positive moves items toward the end, and ones that fall off wrap around to the start."),
            InsertIntoList:qsTr("Inserts an item at a position in the list, counting from 1. The item there and those after it move down one."),
            ReplaceItemOfList:qsTr("Replaces the item at a position in the list, counting from 1."),
            ReverseList:qsTr("Reverses the order of the items in the list."),
            Return:qsTr("Only inside a custom reporter block's definition: stops it and gives this value back to wherever the block was used."),
            If:qsTr("Runs the blocks inside only when the condition is true, then carries on either way."),
            IfElse:qsTr("Runs the first set of blocks when the condition is true, otherwise the second."),
            Repeat:qsTr("Runs the blocks inside this many times, then carries on."),
            Forever:qsTr("Runs the blocks inside over and over until the macro is stopped or an escape loop block leaves it."),
            While:qsTr("Checks the condition, and while it's true runs the blocks inside and checks again. Never runs them if it starts false."),
            EscapeLoop:qsTr("Leaves the innermost repeat, forever or while loop right away and carries on after it. Does nothing outside a loop."),
            ContinueLoop:qsTr("Skips the rest of this pass of the innermost loop and starts the next one. Does nothing outside a loop."),
            CallBlock:qsTr("Runs a custom block defined in this macro. Right-click it to edit the block's definition."),
            BlockHeader:qsTr("The start of a custom block's definition. The blocks under it run whenever the custom block is used."),
            RunBranch:qsTr("Inside a custom block's definition: runs the blocks placed in that slot where the custom block is used."),
            Add:qsTr("Adds two numbers."),Sub:qsTr("Subtracts the second number from the first."),Mul:qsTr("Multiplies two numbers."),
            Div:qsTr("Divides the first number by the second. Dividing by 0 is an error."),
            Mod:qsTr("The remainder after dividing the first number by the second, never negative. 7 mod 3 is 1; handy for 'every Nth time'."),
            Round:qsTr("Rounds to the nearest whole number. Halves round away from 0."),
            Math:qsTr("Applies a function to a number: abs, floor, ceiling, square root, sin/cos/tan (in degrees), their inverses, ln, log, e^ or 10^."),
            Random:qsTr("A random number between the two, both included. Whole numbers give a whole number; a decimal in either gives a decimal."),
            Join:qsTr("Sticks two texts together with nothing in between."),Join3:qsTr("Sticks three texts together with nothing in between."),
            NewLine:qsTr("A line break, for joining into text."),Tab:qsTr("A tab character, for joining into text (like separating columns)."),
            IndexOf:qsTr("Where the first text first appears in the second, counting letters from 1. 0 when it isn't there."),
            LastIndexOf:qsTr("Where the first text last appears in the second, counting letters from 1. 0 when it isn't there."),
            LetterOf:qsTr("The letter at a position in the text, counting from 1. A position past the end is an error."),
            Length:qsTr("How many letters the text has."),Case:qsTr("The text in all upper case or all lower case."),
            Eq:qsTr("Whether two values are equal. Numbers compare as numbers (1 = 1.0); text compares exactly, so case matters."),
            Neq:qsTr("Whether two values are different. Numbers compare as numbers; text compares exactly."),
            Gt:qsTr("Whether the first number is greater than the second."),Lt:qsTr("Whether the first number is less than the second."),
            Gte:qsTr("Whether the first number is greater than or equal to the second."),Lte:qsTr("Whether the first number is less than or equal to the second."),
            And:qsTr("True when both sides are true."),Or:qsTr("True when either side is true."),Not:qsTr("Flips true to false and false to true."),
            True:qsTr("Always true."),False:qsTr("Always false."),
            PluggedIn:qsTr("Whether the computer is on external power. Always true on a computer without a battery."),
            BatteryPercentage:qsTr("The battery's charge, 0 to 100. An error on a computer without a battery."),
            CurrentTime:qsTr("One part of the current local date and time: year, month, date, day of week (1 is Sunday), hour, minute or second."),
            ClipboardText:qsTr("The text on the clipboard right now."),ClipboardHasImage:qsTr("Whether the clipboard holds an image."),
            ClipboardHasFiles:qsTr("Whether the clipboard holds copied files."),
            ListItem:qsTr("The item at a position in the list, counting from 1. Empty when there's no item there."),
            ListItemNumber:qsTr("The position of the first item equal to this one, counting from 1. 0 when it isn't in the list."),
            ListAmount:qsTr("How many items in the list equal this one."),ListLength:qsTr("How many items the list has."),
            ListContains:qsTr("Whether any item in the list equals this one."),
            ListItemExists:qsTr("Whether the list has an item at this position, counting from 1."),
            ListIsEmpty:qsTr("Whether the list has no items."),
            ListAsJson:qsTr("The whole list as a JSON array, like [1,\"two\"]. Handy for sending in a web request."),
            ReadFile:qsTr("The text in a file. ~ means your home folder. An error if the file can't be read."),
            FileExists:qsTr("Whether a file or folder exists at this path. ~ means your home folder."),
            ListDirectory:qsTr("The names of the files and folders inside a folder, sorted, one per line. Split them into a list to loop over them."),
            HttpGet:qsTr("Downloads a web address and gives back the response body as text."),
            JsonGet:qsTr("Reads one value out of JSON text by a dotted path like items.0.name (list positions count from 0). Text comes back without quotes; an empty path gives the whole thing."),
            ShellOutput:qsTr("Runs a shell command, waits for it, and gives back what it printed, without the final line break."),
            ShellSucceeded:qsTr("Runs a shell command, waits for it, and gives whether it succeeded (exit code 0)."),
            ActiveWindowTitle:qsTr("The title of the window that has focus."),ActiveWindowApp:qsTr("The app name of the window that has focus."),
            WindowExists:qsTr("Whether there's an open window whose title or app name contains the text."),
            LastError:qsTr("The message of the error caught by the surrounding try block. Empty when nothing has failed."),
            MacroRunning:qsTr("Whether the macro with this name is running right now."),
            GlobalVariable:qsTr("The value of a global variable. Globals are shared by every macro and kept after Blockwork restarts."),
            Var:qsTr("The current value of a variable. Drag it into any input to use it."),
            Param:qsTr("One of a custom block's inputs: the value it was given where the block is used. Only works inside the block's definition."),
            Call:qsTr("Runs a custom reporter block defined in this macro and gives back the value it returns."),
            Number:qsTr("A plain number typed into an input.")};
        return x[type]||"";
    }
    // Titles for blocks whose wire name doesn't read as English.
    function blockTitle(type) {
        const t={Add:qsTr("Add"),Sub:qsTr("Subtract"),Mul:qsTr("Multiply"),Div:qsTr("Divide"),Mod:qsTr("Modulo"),Eq:qsTr("Equals"),Neq:qsTr("Not equal"),
            Gt:qsTr("Greater than"),Lt:qsTr("Less than"),Gte:qsTr("At least"),Lte:qsTr("At most"),Join3:qsTr("Join three"),Var:qsTr("Variable"),Param:qsTr("Block input"),
            Call:qsTr("Custom reporter"),CallBlock:qsTr("Custom block"),BlockHeader:qsTr("Custom block definition"),ListAsJson:qsTr("List as JSON"),JsonGet:qsTr("JSON get"),HttpGet:qsTr("HTTP get"),HttpRequest:qsTr("HTTP request"),Text:qsTr("Type text")};
        if(t[type]) return t[type];
        const h=String(type).replace(/([a-z0-9])([A-Z])/g,"$1 $2").toLowerCase();
        return h.charAt(0).toUpperCase()+h.slice(1);
    }
    function showDetails(type,fallback) {
        const def = blockDefById(type);
        if (def) {
            const name = (def.pieces || []).map(p => p.kind === "Label" ? p.text : "(" + p.name + ")").join(" ");
            const explainer = def.read_only ? qsTr("A community block from %1.").arg(def.pack_name) : fallback;
            detailsDialog.show(name, def.identifier || def.id, def.description || explainer || qsTr("A custom block defined in this macro."));
        } else detailsDialog.show(blockTitle(type),type,instructionExplainer(type)||fallback||qsTr("A Blockwork block."));
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
        const hint = { image: qsTr("Drag around the image to look for"), region: qsTr("Drag out the part of the window to search") }[request.kind];
        root.afterUnmap(() => screenOverlay.begin(!!hint, hint || qsTr("Click a point or drag out a region")));
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
    function setBlockDetector(request, detector) {
        const updated = JSON.parse(JSON.stringify(request.instruction || {}));
        updated.detector = textValue(detector);
        root.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
    }
    function setBlockUsb(request, pattern) {
        const updated = JSON.parse(JSON.stringify(request.instruction || {}));
        updated.pattern = textValue(pattern);
        root.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
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
        if (pattern === null) { pickError.show(qsTr("Type the window's name into the block to pick a region of it.")); return; }
        root.daemonCall("window_bounds", { pattern: pattern, by: ins.by || "Title" }, (bounds, error) => {
            if (!bounds) { pickError.show(error); return; }
            const r = v => Math.round(v);
            const updated = JSON.parse(JSON.stringify(ins));
            updated.region = [r(x - bounds.x), r(y - bounds.y), r(width), r(height)].join(",");
            root.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
        });
    }
    function pickSignal(location, value) {
        const strand = canvas.strandById(location.strand_id);
        const path = location.index;
        const list = strand ? canvas.listFor(strand, path.slice(0, path.length - 1)) : null;
        const instruction = list ? list[path[path.length - 1].index] : null;
        if (!instruction) return;
        const macroId = root.macro.id, blockId = instruction.id;
        signalPicker.openFor((name, expression) => {
            if (!root.macro || root.macro.id !== macroId) return;
            const current = root.findInstruction(blockId);
            if (!current) return;
            const updated = JSON.parse(JSON.stringify(current));
            updated.expression = expression;
            if (!expression) updated.name = root.textValue(name);
            root.invoke("edit_instruction", { strandId: location.strand_id, path: path, instruction: updated });
        }, value && value.kind === "Text" ? value.value : "");
    }
    function onPieceAction(action, strandId, path, instruction) {
        const request = { kind: "image", strandId: strandId, path: path, instruction: instruction };
        if (action === "captureImage") root.startScreenPick(request);
        else if (action === "loadImage") imageFileDialog.openFor(request);
        else if (action === "pickModel") modelDialog.openFor(request);
        else if (action === "pickUsb") usbDialog.openFor(request);
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
        appSelector.title = instruction && instruction.type === "CloseApp" ? qsTr("Choose an App to Close") : qsTr("Choose an App");
        appSelector.open();
    }
    function openPaletteAppPicker(instruction) {
        const type = instruction ? instruction.type : "OpenApp";
        appPickerRequest = { paletteType: type };
        appSelector.title = type === "CloseApp" ? qsTr("Choose an App to Close") : qsTr("Choose an App");
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
        ghostHeld = false;
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
        const landing = canvas.paletteDropScenePoint();
        if (!drag || recordingBusy) { cancelPaletteDrag(); return; }
        paletteDrag = null;
        canvas.updatePaletteValueTarget(null, null);
        const spec = drag.spec;
        // A fresh operator dropped straight onto a value slot fills it.
        if (spec.kind === "value" && valueTarget) {
            canvas.clearSnap(); holdGhost(landing);
            root.invoke("put_value", { location: valueTarget, value: spec.value });
            return;
        }
        const at = canvas.workspacePoint(sx, sy);
        if (!at) { cancelPaletteDrag(); return; }
        const x = Math.round(at.x - drag.offsetX / canvas.zoom), y = Math.round(at.y - drag.offsetY / canvas.zoom);
        if (spec.kind === "value") {
            canvas.clearSnap(); holdGhost(null);
            root.invoke("create_floating_value", { x: x, y: y, value: spec.value, originBlockId: spec.originBlockId || null });
            return;
        }
        // Attach into the previewed gap when one is showing, else park as a
        // new detached strand - mirrors the webapp palette drop.
        if (snapValid && snapTargetId) {
            canvas.settlePaletteDrop(); holdGhost(landing);
            if (spec.kind === "branch") {
                root.invoke("add_instruction", { strandId: snapTargetId, path: snapPath, instruction: { id: uuid(), type: "RunBranch", name: spec.instruction.name } });
            } else if (spec.kind === "custom") {
                const instruction = palette.customInstruction(spec.definition);
                instruction.id = uuid();
                root.invoke("add_instruction", { strandId: snapTargetId, path: snapPath, instruction: instruction });
            } else {
                root.invoke("add_instruction", { strandId: snapTargetId, path: snapPath, instruction: defaultInstruction(spec.type) });
            }
            return;
        }
        canvas.clearSnap(); holdGhost(null);
        if (spec.kind === "branch") root.invoke("add_strand", { x: x, y: y, instruction: { id: uuid(), type: "RunBranch", name: spec.instruction.name } });
        else if (spec.kind === "custom") root.addCustomBlock(spec.definition, x, y);
        else root.addBlock(spec.type, x, y);
    }
    // Keeps the dropped ghost on screen (moved to `scenePoint` if given)
    // until the new state shows the real block, so it never blinks out.
    property bool ghostHeld: false
    function holdGhost(scenePoint) {
        if (scenePoint) { const p = ghostLayer.mapFromItem(null, scenePoint.x, scenePoint.y); ghost.x = p.x; ghost.y = p.y; }
        ghostHeld = true;
        ghostHeldDoc = macroDocKey();
        ghostHoldTimer.restart();
    }
    // Strands + floating values; unrelated state pushes leave this unchanged.
    property string ghostHeldDoc: ""
    function macroDocKey() { return root.macro ? JSON.stringify([root.macro.strands, root.macro.floating_values]) : ""; }
    function releaseGhost() {
        if (!ghostHeld) return;
        ghostHeld = false; ghost.visible = false; ghost.spec = null;
    }
    Timer { id: ghostHoldTimer; interval: 1500; onTriggered: root.releaseGhost() }
    onMacroChanged: if (ghostHeld && macroDocKey() !== ghostHeldDoc) releaseGhost()
    function cancelPaletteDrag() { paletteDrag = null; ghostHeld = false; ghost.visible = false; ghost.spec = null; canvas.clearSnap(); canvas.updatePaletteValueTarget(null, null); }
    // A canvas block dropped on the sidebar deletes it and everything below it in its stack.
    function blockDefById(id) {
        const defs = root.macro ? root.macro.block_defs : [];
        for (let i = 0; i < defs.length; ++i) if (defs[i].id === id) return defs[i];
        return null;
    }
    function editBlockById(id) {
        const def = blockDefById(id);
        if (def && !def.read_only) blockDialog.openForEdit(def);
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
        canvas.settleTrashed();
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
        root.invoke("remove_instruction", { strandId: strandId, path: path, count: tailCount });
    }
    // An existing value dropped on the sidebar resets its slot (fields) or
    // deletes it (whole floating blocks) - mirrors the webapp trash.
    function trashDraggedValue(location, value, sx, sy) {
        if (root.sidebarCollapsed || !palette.contains(palette.mapFromItem(null, sx, sy))) return;
        canvas.settleTrashed();
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
            Layout.fillWidth: true; Layout.preferredHeight: 52; color: Theme.field; border.color: Theme.borderSoft
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
                    ToolTip.visible: hovered; ToolTip.text: root.sidebarCollapsed ? qsTr("Show sidebar") : qsTr("Hide sidebar")
                    onClicked: { root.sidebarToggling = Motion.enabled; root.sidebarCollapsed = !root.sidebarCollapsed; }
                }
                Rectangle {
                    id: libraryButton
                    Layout.preferredWidth: Math.max(110, Math.min(240, toolbar.width * 0.2)); Layout.preferredHeight: 34
                    radius: Theme.radius; color: libraryHover.hovered ? Theme.hover : Theme.panelRaised; border.color: library.visible ? Theme.accent : Theme.border
                    Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                    Behavior on border.color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                    Accessible.role: Accessible.Button; Accessible.name: qsTr("Macro library")
                    RowLayout {
                        anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 8; spacing: 7
                        LucideIcon { name: "blocks"; width: 15; height: 15; color: Theme.textDim }
                        Text {
                            Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 13
                            text: root.macro ? (root.macro.settings.folder ? root.macro.settings.folder + " / " : "") + root.macro.name : qsTr("Choose a macro")
                            color: root.macro ? Theme.text : Theme.textDim
                        }
                        LucideIcon { name: "chevron-down"; width: 14; height: 14; color: Theme.textDim }
                    }
                    HoverHandler { id: libraryHover; cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: library.visible ? library.close() : library.open() }
                    ToolTip.visible: libraryHover.hovered && !library.visible; ToolTip.delay: 500; ToolTip.text: qsTr("Macro library")
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
                    ToolTip.visible: hovered && !macroMenu.visible; ToolTip.text: qsTr("Macro actions")
                    property double pressStamp: 0
                    property double closeStamp: 0
                    onPressed: pressStamp = Date.now()
                    // The press that closes the menu also clicks the button; don't reopen on release.
                    onClicked: if (Math.abs(closeStamp - pressStamp) > 100) macroMenu.popup(macroMenuButton, 0, macroMenuButton.height + 4)
                    BwMenu {
                        id: macroMenu
                        onAboutToHide: macroMenuButton.closeStamp = Date.now()
                        implicitWidth: 200
                        BwMenuItem { iconName: "sliders"; text: qsTr("Macro settings"); enabled: !!root.macro; onTriggered: macroSettings.open() }
                        BwMenuItem { iconName: "navigation"; text: qsTr("Signals…"); enabled: !!root.macro; onTriggered: signalsDialog.open() }
                        BwMenuItem { iconName: "variable"; text: qsTr("Globals…"); onTriggered: globalPicker.openFor(null) }
                        BwMenuItem { iconName: "plus"; text: qsTr("New macro"); onTriggered: root.invoke("new_macro") }
                        BwMenuItem { iconName: "sparkles"; text: qsTr("New from template..."); onTriggered: templateDialog.open() }
                        BwMenuItem { iconName: "copy"; text: qsTr("Duplicate macro"); enabled: !!root.macro; onTriggered: root.invoke("duplicate_macro", { id: root.macro.id }) }
                        BwMenuItem { iconName: "upload"; text: qsTr("Export..."); enabled: !!root.macro && !!root.macroFiles; onTriggered: root.macroFiles.exportMacro(root.macro.id) }
                        BwMenuItem { iconName: "circle-dot"; text: qsTr("Clear breakpoints"); enabled: (appState.breakpoints || []).length > 0; onTriggered: root.invoke("clear_breakpoints") }
                        BwMenuItem { iconName: "octagon"; text: qsTr("Emergency stop"); danger: true; onTriggered: root.invoke("emergency_stop") }
                        BwMenuItem { iconName: "trash"; text: qsTr("Delete macro"); danger: true; enabled: !!root.macro; onTriggered: removeDialog.open() }
                    }
                }
                Item { Layout.fillWidth: true }
                BwButton {
                    implicitHeight: 34; iconName: "play"; iconFilled: true; iconColor: Theme.accent; text: toolbar.tier < 2 ? qsTr("Run") : ""
                    implicitWidth: toolbar.tier < 2 ? 76 : 34
                    enabled: appState.macro_selected !== null; ToolTip.visible: hovered && toolbar.tier >= 2; ToolTip.text: qsTr("Run macro")
                    onClicked: root.invoke("run_macro")
                }
                Row {
                    spacing: -1
                    BwButton {
                        id: recordButton
                        implicitHeight: 34
                        iconName: root.recording ? "square" : (root.recordingCountdown ? "x" : "circle")
                        iconFilled: !root.recordingCountdown; iconColor: root.recordingCountdown ? Theme.text : Theme.danger
                        text: toolbar.tier < 1 ? root.recordingLabel() : ""
                        // Wide enough for its longest label, so the text never clips and
                        // the toolbar doesn't shift as the countdown ticks.
                        implicitWidth: toolbar.tier < 1 ? Math.ceil(Math.max(recordLabelWidth.width, stopLabelWidth.width, cancelLabelWidth.width)) + 16 + 8 + leftPadding + rightPadding : 34
                        TextMetrics { id: recordLabelWidth; font: recordButton.font; text: qsTr("Record") }
                        TextMetrics { id: stopLabelWidth; font: recordButton.font; text: qsTr("Stop") }
                        TextMetrics { id: cancelLabelWidth; font: recordButton.font; text: qsTr("Cancel (%1)").arg(88) }
                        enabled: appState.macro_selected !== null; ToolTip.visible: hovered; ToolTip.text: root.recordingTooltip()
                        onClicked: root.invoke(root.recordingBusy ? "stop_recording" : "start_recording")
                    }
                    BwButton {
                        iconName: "chevron-down"; text: ""; implicitWidth: 26; implicitHeight: 34
                        ToolTip.visible: hovered; ToolTip.text: qsTr("Recording settings"); onClicked: recordingSettings.open()
                    }
                }
                BwButton {
                    iconName: "crosshair"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: !!root.macro
                    ToolTip.visible: hovered; ToolTip.text: qsTr("Pick a point or region from the screen")
                    onClicked: root.startScreenPick({ kind: "tool" })
                }
                Item { Layout.fillWidth: true }
                BwButton { visible: toolbar.tier < 3; iconName: "undo"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: appState.can_undo; ToolTip.visible: hovered; ToolTip.text: qsTr("Undo"); onClicked: root.invoke("undo") }
                BwButton { visible: toolbar.tier < 3; iconName: "redo"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: appState.can_redo; ToolTip.visible: hovered; ToolTip.text: qsTr("Redo"); onClicked: root.invoke("redo") }
                BwButton { visible: toolbar.tier < 3; iconName: "save"; text: ""; implicitWidth: 34; implicitHeight: 34; enabled: !!root.macro; ToolTip.visible: hovered; ToolTip.text: qsTr("Save macro"); onClicked: root.invoke("save_macro") }
                BwButton {
                    id: editMenuButton
                    visible: toolbar.tier >= 3
                    iconName: "ellipsis"; text: ""; implicitWidth: 34; implicitHeight: 34
                    ToolTip.visible: hovered && !editMenu.visible; ToolTip.text: qsTr("Undo, redo and save")
                    property double pressStamp: 0
                    property double closeStamp: 0
                    onPressed: pressStamp = Date.now()
                    // The press that closes the menu also clicks the button; don't reopen on release.
                    onClicked: if (Math.abs(closeStamp - pressStamp) > 100) editMenu.popup(editMenuButton, 0, editMenuButton.height + 4)
                    BwMenu {
                        id: editMenu
                        onAboutToHide: editMenuButton.closeStamp = Date.now()
                        implicitWidth: 170
                        BwMenuItem { iconName: "undo"; text: qsTr("Undo"); enabled: appState.can_undo; onTriggered: root.invoke("undo") }
                        BwMenuItem { iconName: "redo"; text: qsTr("Redo"); enabled: appState.can_redo; onTriggered: root.invoke("redo") }
                        BwMenuItem { iconName: "save"; text: qsTr("Save macro"); enabled: !!root.macro; onTriggered: root.invoke("save_macro") }
                    }
                }
                BwButton { iconName: "settings"; text: ""; implicitWidth: 34; implicitHeight: 34; ToolTip.visible: hovered; ToolTip.text: qsTr("Settings"); onClicked: root.invoke("open_settings") }
                WindowControls { visible: Qt.platform.os !== "osx" }
            }
        }
        Rectangle {
            visible: !!appState.triggers_paused
            Layout.fillWidth: true; Layout.preferredHeight: 40; color: Theme.warningBackground; border.color: Theme.warningBorder
            RowLayout {
                anchors.fill: parent; anchors.leftMargin: 14; anchors.rightMargin: 10; spacing: 10
                LucideIcon { name: "pause"; color: Theme.warning; width: 16; height: 16 }
                Text { Layout.fillWidth: true; text: qsTr("Emergency stop: triggers are paused until you resume them."); color: Theme.text; font.pixelSize: 13; elide: Text.ElideRight }
                BwButton { text: qsTr("Resume triggers"); implicitHeight: 28; onClicked: root.invoke("resume_triggers") }
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
            color: isPaused ? Theme.warningBackground : Theme.successBackground; border.color: isPaused ? Theme.warningBorder : Theme.successBorder
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
                            ? (debugBar.paused.length > 1 ? qsTr("Paused before \"%1\" (%n strand(s) waiting)", "", debugBar.paused.length).arg(root.blockLabel(debugBar.strand.block_id))
                                 : qsTr("Paused before \"%1\"").arg(root.blockLabel(debugBar.strand.block_id)))
                            : debugBar.isPaused ? qsTr("Pausing at the next block...") : qsTr("Running")
                        HoverHandler { cursorShape: debugBar.strand ? Qt.PointingHandCursor : Qt.ArrowCursor }
                        TapHandler { enabled: !!debugBar.strand; onTapped: canvas.revealInstruction(debugBar.strand.block_id) }
                    }
                    BwButton { visible: !debugBar.isPaused; iconName: "pause"; text: qsTr("Pause"); implicitHeight: 28; ToolTip.visible: hovered; ToolTip.text: qsTr("Pause before the next block (F8)"); onClicked: debugBar.act("pause") }
                    BwButton { visible: debugBar.isPaused; iconName: "play"; text: qsTr("Resume"); implicitHeight: 28; ToolTip.visible: hovered; ToolTip.text: qsTr("Run on until the next breakpoint (F8)"); onClicked: debugBar.act("resume") }
                    BwButton { visible: debugBar.isPaused; iconName: "step-forward"; text: qsTr("Step"); implicitHeight: 28; enabled: debugBar.paused.length > 0; ToolTip.visible: hovered; ToolTip.text: qsTr("Run one block, then pause again (F10)"); onClicked: debugBar.act("step") }
                    BwButton { iconName: "square"; text: qsTr("Stop"); danger: true; implicitHeight: 28; onClicked: debugBar.act("stop") }
                }
                Flow {
                    visible: !!debugBar.strand
                    Layout.fillWidth: true; Layout.bottomMargin: 2; spacing: 6
                    Text {
                        visible: !!debugBar.strand && !debugBar.strand.variables.length && !debugBar.strand.lists.length
                        text: qsTr("No variables"); color: Theme.textDim; font.pixelSize: 12
                    }
                    Repeater {
                        model: debugBar.strand ? debugBar.strand.variables.map(v => v[0] + " = " + v[1])
                            .concat(debugBar.strand.lists.map(l => l[0] + ": [" + l[1].join(", ") + "]")) : []
                        delegate: Rectangle {
                            required property string modelData
                            width: Math.min(chipText.implicitWidth + 16, 320); height: 24; radius: 12
                            color: Theme.field; border.color: Theme.border
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
                        NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic; onRunningChanged: if (!running) root.sidebarToggling = false }
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
                    appCategories: [{ id: "screen", name: qsTr("Screen"), icon: "crosshair" }]
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
                    onEditBlockRequested: definition => { if (!definition.read_only) blockDialog.openForEdit(definition); }
                    onDeleteBlockRequested: blockId => root.invoke("delete_block", { blockId: blockId })
                    onMakeListRequested: listDialog.openForCreate()
                    onListEditorStateRequested: (name, visible, x, y) => root.invoke("set_list_editor_state", { name: name, visible: visible, x: x, y: y })
                    onRenameListRequested: name => listDialog.openForRename(name)
                    onDeleteListRequested: name => { deleteListDialog.listName = name; deleteListDialog.open(); }
                    onMakeBlockRequested: blockDialog.openForCreate()
                    blockLibraryEnabled: !!root.blockLibrary
                    onBrowseBlocksRequested: root.blockLibrary.browse()
                    onShareBlockRequested: definition => root.blockLibrary.share(definition)
                    onStandaloneKeyCaptureRequested: root.invoke("start_standalone_key_capture")
                    onAppPickerRequested: instruction => root.openPaletteAppPicker(instruction)
                    onDetailsRequested:(name,identifier,explainer)=>root.blockDefById(identifier)||name===identifier?root.showDetails(identifier,explainer):detailsDialog.show(name,identifier,explainer)
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
                    locked: root.recordingBusy
                    onStrandMoved: (strandId, x, y) => root.invoke("move_strand", { strandId: strandId, x: x, y: y })
                    onInstructionSplit: (strandId, path, x, y) => root.invoke("split_strand", { strandId: strandId, path: path, x: x, y: y })
                    onStrandsMerged: (draggedId, targetId, path) => root.invoke("merge_strand", { draggedId: draggedId, targetId: targetId, path: path })
                    onTailMerged: (strandId, path, targetId, targetPath) => root.invoke("merge_tail", { strandId: strandId, path: path, targetId: targetId, targetPath: targetPath })
                    onBlockDragOutside: (strandId, path, tailCount, sx, sy) => root.trashDraggedBlocks(strandId, path, tailCount, sx, sy)
                    onInstructionRemoved: (strandId, path) => root.removeInstruction(strandId, path)
                    canPaste: root.clipboard.length > 0
                    clearIcon: appState.confirm_clear_instructions ? "triangle-alert" : "trash"
                    clearText: appState.confirm_clear_instructions ? qsTr("Confirm delete (%1s)?").arg(appState.confirm_clear_instructions_remaining_secs) : qsTr("Delete %n block(s)", "", root.totalBlocks())
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
                    onPaletteDragStarted: (spec, sx, sy, ox, oy) => root.beginPaletteDrag(spec, ox, oy, sx, sy)
                    onPaletteDragMoved: (sx, sy) => root.movePaletteDrag(sx, sy)
                    onPaletteDragEnded: (sx, sy) => root.endPaletteDrag(sx, sy)
                    onPaletteDragCanceled: root.cancelPaletteDrag()
                    onValueEdited: (location, text) => root.invoke("edit_value_field", { location: location, text: text })
                    onValueTakeRequested: location => root.invoke("take_value", { location: location })
                    onValuePutRequested: (location, value) => root.invoke("put_value", { location: location, value: value })
                    onValueCreateRequested: (x, y, value, originBlockId) => root.invoke("create_floating_value", { x: x, y: y, value: value, originBlockId: originBlockId || null })
                    onValueMoveRequested: (from, to, x, y, originBlockId) => root.invoke("move_value", { from: from, to: to, x: x, y: y, originBlockId: originBlockId || null })
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
                    onDetailsRequested:type=>root.showDetails(type)
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
            RecordingOverlay {
                anchors.fill: parent; z: 20
                recording: root.recording
                countdown: root.recordingCountdown && appState.recording_phase.countdown != null ? appState.recording_phase.countdown : 0
                mouseMovement: !!appState.record_mouse_movement
                shortcut: root.stopRecordingShortcut()
                onStopRequested: root.invoke("stop_recording")
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
                LucideIcon { anchors.horizontalCenter: parent.horizontalCenter; name: "blocks"; color: Theme.textDim; width: 48; height: 48 }
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: qsTr("No macro selected"); color: Theme.text; font.pixelSize: 16; font.weight: Font.DemiBold }
                Text { text: qsTr("Choose a macro above or create a new one."); color: Theme.textDim; font.pixelSize: 13 }
            }
        }
    }

    AppDialog {
        id: deleteMacroDialog; title: qsTr("Delete macro?"); standardButtons: Dialog.Yes | Dialog.Cancel
        property string macroId: ""
        property string macroName: ""
        Text { text: qsTr("This removes \"%1\" from Blockwork.").arg(deleteMacroDialog.macroName); color: Theme.text }
        onAccepted: root.invoke("delete_macro", { id: macroId })
    }
    AppDialog {
        id: removeDialog; title: qsTr("Delete macro?"); standardButtons: Dialog.Yes | Dialog.Cancel
        Text { text: qsTr("This removes the selected macro from Blockwork."); color: Theme.text }
        onAccepted: root.invoke("remove_macro")
    }
    AppDialog {
        id: macroSettings; title: qsTr("Macro settings"); standardButtons: Dialog.Close
        readonly property var settings: root.macro && root.macro.settings ? root.macro.settings : ({})
        function saveConditions(days) {
            root.invoke("set_macro_conditions", { onlyInWindow: condWindow.text, power: powerBox.options[powerBox.currentIndex].value,
                activeFrom: condFrom.text, activeUntil: condUntil.text, activeDays: days === undefined ? (settings.active_days || 0) : days });
        }
        Flickable {
        id: macroSettingsFlick
        implicitWidth: 380; implicitHeight: Math.min(settingsColumn.implicitHeight, root.height - 190)
        WheelPan { flickable: macroSettingsFlick }
        contentHeight: settingsColumn.implicitHeight; clip: true; boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        Column { id: settingsColumn; spacing: 14; width: 360
            Text { text: qsTr("Controls behavior specific to this macro."); color: Theme.textDim }
            Text { text: qsTr("Title"); color: Theme.textDim; font.pixelSize: 13 }
            BwTextField { width: parent.width; text: root.macro ? root.macro.name : ""; onEditingFinished: root.invoke("set_title", { title: text }) }
            Text { text: qsTr("Speed"); color: Theme.textDim; font.pixelSize: 13 }
            RowLayout { width: parent.width; spacing: 10
                Slider {
                    id: speed; from: .1; to: 10; value: root.macro ? root.macro.speed_multiplier : 1; Layout.fillWidth: true; implicitHeight: 30
                    onMoved: root.invoke("set_macro_speed_multiplier", { multiplier: value })
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    background: Rectangle { x: speed.leftPadding; y: speed.topPadding + speed.availableHeight / 2 - height / 2; width: speed.availableWidth; height: 5; radius: 3; color: Theme.border; Rectangle { width: speed.visualPosition * parent.width; height: parent.height; radius: 3; color: Theme.accent } }
                    handle: Rectangle { x: speed.leftPadding + speed.visualPosition * (speed.availableWidth - width); y: speed.topPadding + speed.availableHeight / 2 - height / 2; width: 16; height: 16; radius: 8; color: "white"; border.width: 2; border.color: Theme.accent }
                }
                BwTextField { text: Number(speed.value).toFixed(2); Layout.preferredWidth: 62; horizontalAlignment: Text.AlignRight; onEditingFinished: root.invoke("set_macro_speed_multiplier", { multiplier: Number(text) }) }
                Text { text: "×"; color: Theme.textDim }
            }
            BwSwitch { Accessible.name: qsTr("Listen for triggers while Blockwork is open"); checked: root.macro && root.macro.settings ? root.macro.settings.always_listen : false; onToggled: checked => root.invoke("set_macro_always_listen", { enabled: checked }) }
            Text { text: qsTr("Time limit"); color: Theme.textDim; font.pixelSize: 13 }
            RowLayout { width: parent.width; spacing: 10
                Text { text: qsTr("Stop a run after"); color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    Layout.preferredWidth: 70; horizontalAlignment: Text.AlignRight
                    validator: IntValidator { bottom: 0; top: 86400 }
                    text: root.macro && root.macro.settings ? String(root.macro.settings.max_run_secs) : "0"
                    onEditingFinished: root.invoke("set_macro_max_run_secs", { seconds: Number(text) || 0 })
                }
                Text { Layout.fillWidth: true; text: qsTr("seconds (0 = no limit)"); color: Theme.textDim; font.pixelSize: 13 }
            }
            Text { text: qsTr("Loop limit"); color: Theme.textDim; font.pixelSize: 13 }
            RowLayout { width: parent.width; spacing: 10
                Text { text: qsTr("Stop a loop after"); color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    Layout.preferredWidth: 90; horizontalAlignment: Text.AlignRight
                    validator: IntValidator { bottom: 0; top: 2000000000 }
                    text: root.macro && root.macro.settings ? String(root.macro.settings.max_loop_iterations || 0) : "0"
                    onEditingFinished: root.invoke("set_macro_max_loop_iterations", { iterations: Number(text) || 0 })
                }
                Text { Layout.fillWidth: true; text: qsTr("repeats (0 = no limit)"); color: Theme.textDim; font.pixelSize: 13 }
            }
            Text { text: qsTr("Conditions"); color: Theme.textDim; font.pixelSize: 13 }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Triggers and hotkeys only start this macro when all of these hold. The Run button ignores them.")
            }
            RowLayout { width: parent.width; spacing: 8
                Text { text: qsTr("While"); color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    id: condWindow; Layout.fillWidth: true; placeholderText: qsTr("any window")
                    text: macroSettings.settings.only_in_window || ""
                    onEditingFinished: macroSettings.saveConditions()
                }
                Text { text: qsTr("is focused"); color: Theme.text; font.pixelSize: 13 }
            }
            RowLayout { width: parent.width; spacing: 8
                Text { text: qsTr("On"); color: Theme.text; font.pixelSize: 13 }
                BwComboBox {
                    id: powerBox; Layout.fillWidth: true; textRole: "text"
                    readonly property var options: [{ value: "Any", text: qsTr("battery or AC power") }, { value: "PluggedIn", text: qsTr("AC power only") }, { value: "OnBattery", text: qsTr("battery only") }]
                    model: options
                    currentIndex: Math.max(0, options.findIndex(o => o.value === macroSettings.settings.power))
                    onActivated: macroSettings.saveConditions()
                }
            }
            RowLayout { width: parent.width; spacing: 8
                Text { text: qsTr("Between"); color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    id: condFrom; Layout.preferredWidth: 70; horizontalAlignment: Text.AlignHCenter; placeholderText: "09:00"
                    text: macroSettings.settings.active_from || ""
                    onEditingFinished: if (!!text === !!condUntil.text) macroSettings.saveConditions()
                }
                Text { text: qsTr("and"); color: Theme.text; font.pixelSize: 13 }
                BwTextField {
                    id: condUntil; Layout.preferredWidth: 70; horizontalAlignment: Text.AlignHCenter; placeholderText: "17:00"
                    text: macroSettings.settings.active_until || ""
                    onEditingFinished: if (!!text === !!condFrom.text) macroSettings.saveConditions()
                }
                Text { Layout.fillWidth: true; text: qsTr("(empty = all day)"); color: Theme.textDim; font.pixelSize: 12 }
            }
            Row { spacing: 6
                Text { text: qsTr("On"); color: Theme.text; font.pixelSize: 13; anchors.verticalCenter: parent.verticalCenter; rightPadding: 2 }
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
            Text { text: qsTr("Schedules"); color: Theme.textDim; font.pixelSize: 13 }
            BwSwitch {
                Accessible.name: qsTr("Catch up on missed schedules")
                checked: !!macroSettings.settings.catch_up_missed
                onToggled: checked => root.invoke("set_macro_catch_up", { enabled: checked })
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Runs a schedule or \"once at\" trigger once if its time passed while Blockwork was closed or the computer was asleep (up to a week back).")
            }
            Text { text: qsTr("Permissions"); color: Theme.textDim; font.pixelSize: 13 }
            BwSwitch {
                Accessible.name: qsTr("Ask before risky actions")
                checked: root.permissions !== null
                onToggled: checked => root.invoke("set_macro_restricted", { restricted: checked })
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: root.permissions === null
                    ? qsTr("This macro may do anything its blocks say. Imported macros ask first until you allow them.")
                    : qsTr("Ticked abilities are allowed. The rest show a dialog the first time the macro tries them.")
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
    AppDialog {
        id: pickError; title: qsTr("Couldn't read the screen"); standardButtons: Dialog.Ok
        property string message: ""
        function show(text) { message = text || qsTr("Unknown error"); open(); }
        Text { width: 420; wrapMode: Text.WordWrap; text: pickError.message; color: Theme.text; font.pixelSize: 13 }
    }
    AppDialog {
        id: pickResult; title: qsTr("Picked from the screen"); standardButtons: Dialog.Close
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
                        id: pickCoordinates
                        ContextMenu.menu: Editing.TextEditingMenu { editor: pickCoordinates }
                        readOnly: true; selectByMouse: true; color: Theme.text; font.pixelSize: 13
                        text: !pickResult.pick ? "" : qsTr("x %1   y %2").arg(pickResult.pick.x).arg(pickResult.pick.y)
                            + (pickResult.region ? "\n" + qsTr("width %1   height %2").arg(pickResult.pick.width).arg(pickResult.pick.height) : "")
                    }
                    Row {
                        spacing: 8
                        Rectangle { width: 18; height: 18; radius: 4; color: pickResult.pick ? pickResult.pick.color : "black"; border.color: Theme.border }
                        TextEdit {
                            id: pickColor
                            ContextMenu.menu: Editing.TextEditingMenu { editor: pickColor }
                            readOnly: true; selectByMouse: true
                            text: pickResult.pick ? pickResult.pick.color : ""; color: Theme.text; font.pixelSize: 13
                        }
                    }
                }
            }
            Text { text: qsTr("Add to the canvas:"); color: Theme.textDim; font.pixelSize: 12 }
            Flow {
                width: parent.width; spacing: 8
                Repeater {
                    model: pickResult.region
                        ? [["ScreenText", qsTr("text in this region")], ["AverageColor", qsTr("average color here")], ["ClickImage", qsTr("click this image")],
                           ["WaitForImage", qsTr("wait for this image")], ["FindImage", qsTr("find this image")]]
                        : [["PixelColor", qsTr("color at this point")], ["MoveMouse", qsTr("move mouse here")]]
                    delegate: BwButton {
                        required property var modelData
                        text: modelData[1]
                        onClicked: { root.addPicked(modelData[0], pickResult.pick); pickResult.close(); }
                    }
                }
            }
        }
    }
    AppDialog {
        id: imageFileDialog; title: qsTr("Use an image file"); standardButtons: Dialog.Ok | Dialog.Cancel
        property var request: null
        function openFor(req) { request = req; imagePath.text = ""; open(); imagePath.forceActiveFocus(); }
        Column { width: 420; spacing: 8
            Text { text: qsTr("Path to a PNG image"); color: Theme.text }
            BwTextField { id: imagePath; width: parent.width; placeholderText: "~/Pictures/button.png"; onAccepted: imageFileDialog.accept() }
        }
        onAccepted: {
            const req = request;
            root.daemonCall("image_from_file", { path: imagePath.text.trim() }, (image, error) => {
                if (image) root.setBlockImage(req, image); else pickError.show(error);
            });
        }
    }
    AppDialog {
        id: modelDialog; title: qsTr("Choose a detection model"); standardButtons: Dialog.Cancel
        property var request: null
        property var models: []
        property bool busy: false
        function openFor(req) {
            request = req; models = []; busy = false; localUrl.text = "";
            root.daemonCall("list_models", {}, (list, error) => { if (list) models = list; else pickError.show(error); });
            open();
        }
        function use(id) { root.setBlockDetector(request, id); close(); }
        function download(id) {
            busy = true;
            root.daemonCall("download_model", { id: id }, (done, error) => {
                busy = false;
                if (done) use(id); else pickError.show(error);
            });
        }
        Column { width: 460; spacing: 10
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Models run inside Blockwork. Pick one, add your own .onnx file, or point the block at a detector running on this computer.") }
            Repeater {
                model: modelDialog.models
                delegate: Rectangle {
                    required property var modelData
                    width: 460; height: 56; radius: Theme.radius; color: Theme.panelRaised; border.color: Theme.border
                    RowLayout { anchors.fill: parent; anchors.margins: 10; spacing: 10
                        Column { Layout.fillWidth: true; spacing: 2
                            Text { text: modelData.id; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
                            Text { width: parent.width; text: modelData.about; color: Theme.textDim; font.pixelSize: 11; elide: Text.ElideRight }
                        }
                        BwButton {
                            enabled: !modelDialog.busy && (modelData.installed || modelData.downloadable)
                            text: modelData.installed ? qsTr("Use") : modelData.downloadable ? qsTr("Download %1 MB").arg(modelData.megabytes) : qsTr("Not published yet")
                            onClicked: modelData.installed ? modelDialog.use(modelData.id) : modelDialog.download(modelData.id)
                        }
                    }
                }
            }
            Text { visible: modelDialog.busy; text: qsTr("Downloading..."); color: Theme.textDim; font.pixelSize: 12 }
            BwButton { text: qsTr("Add a .onnx file..."); iconName: "folder-open"; enabled: !modelDialog.busy; onClicked: modelFile.open() }
            Text { text: qsTr("Or a detector on this computer"); color: Theme.text; font.pixelSize: 12 }
            RowLayout { width: parent.width; spacing: 8
                BwTextField { id: localUrl; Layout.fillWidth: true; placeholderText: root.defaultDetector; onAccepted: useUrl.clicked() }
                BwButton { id: useUrl; text: qsTr("Use"); enabled: localUrl.text.trim().length > 0; onClicked: modelDialog.use(localUrl.text.trim()) }
            }
        }
    }
    FileDialog {
        id: modelFile; title: qsTr("Add a detection model"); fileMode: FileDialog.OpenFile
        currentFolder: StandardPaths.writableLocation(StandardPaths.HomeLocation)
        nameFilters: [qsTr("ONNX models (*.onnx)"), qsTr("All files (*)")]
        onAccepted: {
            const text = decodeURIComponent(selectedFile.toString());
            const path = Qt.platform.os === "windows" ? text.replace(/^file:\/\/\//, "") : text.replace(/^file:\/\//, "");
            root.daemonCall("import_model", { path: path }, (id, error) => { if (id) modelDialog.use(id); else pickError.show(error); });
        }
    }
    NamePicker {
        id: signalPicker
        names: root.macro ? root.macro.signals || [] : []
        chooseTitle: qsTr("Choose a signal")
        createTitle: qsTr("New signal")
        newText: qsTr("New signal…")
        createCommand: "create_signal"
        scopeText: qsTr("In this macro")
        allowExpression: true; caseInsensitive: true
        callCommand: (command, args, done) => root.daemonCall(command, args, done)
    }
    GlobalPicker {
        id: globalPicker
        globals: root.appState.globals || []
        callCommand: (command, args, done) => root.daemonCall(command, args, done)
    }
    // The time and schedule pickers behind the time blocks' slots.
    BwTimePicker {
        id: timePicker
        use12h: Schedules.use12h; amText: Schedules.amText; pmText: Schedules.pmText
        property var location: null
        function openFor(where, value, anchor) {
            location = where;
            const t = Schedules.parseTime(value && value.value !== undefined ? value.value : "") || { hour: 9, minute: 0 };
            if (anchor) openAt(anchor, t.hour, t.minute);
            else { hour = t.hour; minute = t.minute; x = (parent.width - implicitWidth) / 2; y = (parent.height - implicitHeight) / 2; open(); }
        }
        onPicked: (h, m) => root.invoke("edit_value_field", { location: location, text: Schedules.stamp(h, m) })
    }
    BwTimePicker {
        id: dateTimePicker
        withDate: true
        use12h: Schedules.use12h; amText: Schedules.amText; pmText: Schedules.pmText
        monthNames: Array.from({ length: 12 }, (_, m) => Schedules.monthName(m + 1))
        weekdayNames: Array.from({ length: 7 }, (_, d) => Schedules.weekdayShort(d))
        property var location: null
        // A slot that isn't a date yet starts at the next whole hour.
        function openFor(where, value, anchor) {
            location = where;
            const soon = new Date(Date.now() + 3600000);
            const t = Schedules.parseDateTime(value && value.value !== undefined ? value.value : "")
                || { year: soon.getFullYear(), month: soon.getMonth() + 1, day: soon.getDate(), hour: soon.getHours(), minute: 0 };
            if (anchor) openAtDate(anchor, t.year, t.month, t.day, t.hour, t.minute);
            else { setDate(t.year, t.month, t.day); hour = t.hour; minute = t.minute; today = new Date(); x = (parent.width - implicitWidth) / 2; y = (parent.height - implicitHeight) / 2; open(); }
        }
        onPickedAt: (y, mo, d, h, m) => root.invoke("edit_value_field", { location: location, text: Schedules.dateTimeStamp(y, mo, d, h, m) })
    }
    SchedulePicker {
        id: scheduleDialog
        property var location: null
        function openFor(where, value) { location = where; openWith(value && value.value !== undefined ? value.value : ""); }
        onChosen: expression => root.invoke("edit_value_field", { location: location, text: expression })
    }
    AppDialog {
        id: usbDialog; title: qsTr("Choose a USB device"); standardButtons: Dialog.Cancel
        property var request: null
        property var devices: []
        property string error: ""
        property bool loaded: false
        // What the block matches now, if it's plain text. A reporter's request
        // carries its slot's value; a trigger's, the whole instruction.
        readonly property string current: {
            const p = !request ? null : request.kind === "value" ? request.value : request.instruction ? request.instruction.pattern : null;
            return p && p.kind === "Text" ? p.value.trim() : "";
        }
        readonly property bool currentPluggedIn: current === "" || devices.some(d => matches(d, current))
        function matches(device, pattern) {
            const p = pattern.toLowerCase();
            return device.name.toLowerCase().indexOf(p) >= 0 || device.ids.indexOf(p) >= 0;
        }
        function openFor(req) {
            request = req; devices = []; error = ""; loaded = false; customPattern.text = "";
            refresh();
            open();
        }
        // Devices come and go while it's open, so ask again every few seconds.
        function refresh() {
            root.daemonCall("list_usb_devices", {}, (list, error) => {
                loaded = true;
                if (list) { devices = list; usbDialog.error = ""; } else { devices = []; usbDialog.error = error || ""; }
            });
        }
        function use(pattern) {
            if (request.kind === "value") root.invoke("edit_value_field", { location: request.location, text: pattern });
            else root.setBlockUsb(request, pattern);
            close();
        }
        Timer { interval: 2000; repeat: true; running: usbDialog.visible; onTriggered: usbDialog.refresh() }
        Column { width: 460; spacing: 10
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Pick a device that's plugged in. The block matches it by its vendor:product id, so it keeps working on any USB port.") }
            Rectangle {
                visible: usbDialog.current !== "" && usbDialog.loaded && !usbDialog.currentPluggedIn
                width: 460; height: 56; radius: Theme.radius; color: Theme.panelRaised; border.color: Theme.warning
                RowLayout { anchors.fill: parent; anchors.margins: 10; spacing: 10
                    Column { Layout.fillWidth: true; spacing: 2
                        Text { width: parent.width; text: usbDialog.current; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                        Text { text: qsTr("Chosen now, but not plugged in"); color: Theme.warning; font.pixelSize: 11 }
                    }
                    BwButton { text: qsTr("Keep"); onClicked: usbDialog.close() }
                }
            }
            Text { visible: !usbDialog.loaded; text: qsTr("Looking for USB devices..."); color: Theme.textDim; font.pixelSize: 12 }
            Text { visible: usbDialog.error !== ""; width: parent.width; wrapMode: Text.WordWrap; text: usbDialog.error; color: Theme.danger; font.pixelSize: 12 }
            Text { visible: usbDialog.loaded && usbDialog.error === "" && usbDialog.devices.length === 0; width: parent.width; wrapMode: Text.WordWrap
                text: qsTr("No USB devices are plugged in. Plug one in and it shows up here."); color: Theme.textDim; font.pixelSize: 12 }
            ScrollView {
                visible: usbDialog.devices.length > 0
                id: usbScroll
                width: 460; height: Math.min(usbList.implicitHeight, 300); clip: true
                contentWidth: availableWidth
                WheelPan { flickable: usbScroll.contentItem }
                Column { id: usbList; width: 460; spacing: 8
                    Repeater {
                        model: usbDialog.devices
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool chosen: usbDialog.current !== "" && usbDialog.matches(modelData, usbDialog.current)
                            width: 460; height: 56; radius: Theme.radius; color: Theme.panelRaised; border.color: chosen ? Theme.accent : Theme.border
                            RowLayout { anchors.fill: parent; anchors.margins: 10; spacing: 10
                                Column { Layout.fillWidth: true; spacing: 2
                                    Text { width: parent.width; text: modelData.name; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                    Text { width: parent.width; color: Theme.textDim; font.pixelSize: 11; elide: Text.ElideRight
                                        text: modelData.maker ? qsTr("%1 · id %2").arg(modelData.maker).arg(modelData.ids) : qsTr("id %1").arg(modelData.ids) }
                                }
                                BwButton { text: chosen ? qsTr("Chosen") : qsTr("Use"); onClicked: usbDialog.use(modelData.ids) }
                            }
                        }
                    }
                }
            }
            RowLayout { width: parent.width; spacing: 8
                BwButton { text: qsTr("Any USB device"); enabled: usbDialog.current !== ""; onClicked: usbDialog.use("") }
                Item { Layout.fillWidth: true }
                BwButton { text: qsTr("Refresh"); iconName: "refresh-cw"; onClicked: usbDialog.refresh() }
            }
            Text { text: qsTr("Or part of a device's name or id"); color: Theme.text; font.pixelSize: 12 }
            RowLayout { width: parent.width; spacing: 8
                BwTextField { id: customPattern; Layout.fillWidth: true; placeholderText: "046d:c52b"; onAccepted: useCustom.clicked() }
                BwButton { id: useCustom; text: qsTr("Use"); enabled: customPattern.text.trim().length > 0; onClicked: usbDialog.use(customPattern.text.trim()) }
            }
        }
    }
    AppDialog {
        id: templateDialog; title: qsTr("Start from a template"); standardButtons: Dialog.Cancel
        width: Math.min(604, parent.width - 32)
        height: Math.min(implicitHeight, parent.height - 32)
        contentItem: ScrollView {
            id: templateScroll
            implicitWidth: 560; implicitHeight: templateList.implicitHeight
            contentWidth: availableWidth
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            WheelPan { flickable: templateScroll.contentItem }
            TemplateList {
                id: templateList
                width: templateScroll.availableWidth
                templates: appState.templates || []
                onPicked: id => { root.invoke("new_macro_from_template", { template: id }); templateDialog.close(); }
            }
        }
    }
    AppDialog {
        id: recordingSettings; title: qsTr("Recording settings"); standardButtons: Dialog.Close
        Column { spacing: 14; width: 390
            BwSwitch { Accessible.name: qsTr("Record mouse movement"); checked: appState.record_mouse_movement; onToggled: checked => root.invoke("toggle_record_mouse_movement", { enabled: checked }) }
            BwSwitch { Accessible.name: qsTr("Use relative mouse movement"); checked: appState.record_mouse_relative; onToggled: checked => root.invoke("toggle_record_mouse_relative", { relative: checked }) }
            Text { width: parent.width; wrapMode: Text.WordWrap; text: appState.absolute_mouse_position_available ? qsTr("Absolute positioning is available on this system.") : qsTr("Absolute positioning is unavailable; relative movement will be used."); color: Theme.textDim; font.pixelSize: 12 }
            Rectangle { width: parent.width; height: 1; color: Theme.borderSoft }
            Column { width: parent.width; spacing: 6
                Text { text: qsTr("Countdown"); color: Theme.text; font.pixelSize: 14 }
                Text { width: parent.width; wrapMode: Text.WordWrap; text: qsTr("How long Record waits before it starts, so you can switch to the app you want to record."); color: Theme.textDim; font.pixelSize: 12 }
                Row { spacing: 12
                    BwSlider {
                        id: countdownSlider
                        width: 280; from: 0; to: 10; stepSize: 1; ticks: true
                        anchors.verticalCenter: parent.verticalCenter
                        Accessible.name: qsTr("Countdown")
                        value: appState.recording_countdown_secs ?? 3
                        onMoved: root.invoke("set_recording_countdown", { seconds: Math.round(value) })
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        readonly property int secs: Math.round(countdownSlider.value)
                        text: secs === 0 ? qsTr("Off") : qsTr("%n second(s)", "", secs)
                        color: Theme.text; font.pixelSize: 13
                    }
                }
            }
        }
    }
    AppDialog {
        id: variableDialog
        property string renameTarget: ""
        title: renameTarget.length ? qsTr("Rename Variable") : qsTr("Make a Variable")
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openForCreate() { renameTarget = ""; variableName.text = ""; open(); variableName.forceActiveFocus(); }
        function openForRename(name) { renameTarget = name; variableName.text = name; open(); variableName.forceActiveFocus(); variableName.selectAll(); }
        Column { width: 340; spacing: 8
            Text { text: qsTr("Variable name"); color: Theme.text }
            BwTextField { id: variableName; width: parent.width; placeholderText: qsTr("score"); onAccepted: variableDialog.accept() }
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
    AppDialog {
        id: signalsDialog
        title: qsTr("Signals"); standardButtons: Dialog.Close
        Column {
            width: 400; spacing: 10
            Text { width: parent.width; text: qsTr("Named signals in this macro. Expressions can also send and receive names outside this list."); wrapMode: Text.WordWrap; color: Theme.textDim }
            ScrollView {
                width: parent.width; height: Math.min(300, signalRows.implicitHeight)
                clip: true
                Column {
                    id: signalRows; width: 380; spacing: 6
                    Repeater {
                        model: root.macro ? root.macro.signals || [] : []
                        delegate: RowLayout {
                            required property string modelData
                            width: signalRows.width; spacing: 6
                            Text { Layout.fillWidth: true; text: modelData; elide: Text.ElideRight; color: Theme.text }
                            BwButton { text: qsTr("Rename"); onClicked: signalDialog.openForRename(modelData) }
                            BwButton {
                                text: qsTr("Delete")
                                onClicked: root.daemonCall("delete_signal", { name: modelData }, (result, error) => { signalsError.text = error || ""; })
                            }
                        }
                    }
                }
            }
            Text { visible: !root.macro || !(root.macro.signals || []).length; text: qsTr("No named signals yet."); color: Theme.textDim }
            Text { id: signalsError; width: parent.width; visible: text.length > 0; wrapMode: Text.WordWrap; color: Theme.danger }
            BwButton { text: qsTr("New signal…"); iconName: "plus"; onClicked: signalPicker.openForCreate(null) }
        }
        onOpened: signalsError.text = ""
    }
    AppDialog {
        id: signalDialog
        property string renameTarget: ""
        property string macroId: ""
        property bool busy: false
        title: qsTr("Rename signal")
        standardButtons: Dialog.NoButton
        function openForRename(name) {
            renameTarget = name; signalName.text = name;
            macroId = root.macro ? root.macro.id : "";
            signalError.text = ""; busy = false;
            open(); signalName.forceActiveFocus(); signalName.selectAll();
        }
        function submit() {
            const name = signalName.text.trim();
            if (busy || !name.length || !root.macro || root.macro.id !== macroId) return;
            busy = true;
            root.daemonCall("rename_signal", { oldName: renameTarget, newName: name }, (result, error) => {
                busy = false;
                if (error) signalError.text = error;
                else close();
            });
        }
        Column {
            width: 400; spacing: 12
            Text { text: qsTr("Name"); color: Theme.text; font.pixelSize: 13 }
            BwTextField { id: signalName; width: parent.width; enabled: !signalDialog.busy; onAccepted: signalDialog.submit() }
            Text { width: parent.width; wrapMode: Text.WordWrap; text: qsTr("Updates named blocks in this macro. Expressions keep their contents."); color: Theme.textDim; font.pixelSize: 13 }
            Text { id: signalError; width: parent.width; visible: text.length > 0; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 13 }
        }
        footer: DialogButtonBox {
            alignment: Qt.AlignRight; spacing: 8; padding: 18; topPadding: 8
            background: null
            BwButton { text: qsTr("Cancel"); enabled: !signalDialog.busy; onClicked: signalDialog.reject() }
            BwButton { text: qsTr("Rename"); primary: true; enabled: !signalDialog.busy && signalName.text.trim().length > 0; onClicked: signalDialog.submit() }
        }
    }
    AppDialog {
        id: listDialog
        property string renameTarget: ""
        title: renameTarget.length ? qsTr("Rename List") : qsTr("Make a List")
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openForCreate() { renameTarget = ""; listName.text = ""; open(); listName.forceActiveFocus(); }
        function openForRename(name) { renameTarget = name; listName.text = name; open(); listName.forceActiveFocus(); listName.selectAll(); }
        Column { width: 340; spacing: 8
            Text { text: qsTr("List name"); color: Theme.text }
            BwTextField { id: listName; width: parent.width; placeholderText: qsTr("items") }
        }
        onAccepted: {
            const name = listName.text.trim();
            if (!name.length) return;
            if (renameTarget.length) root.invoke("rename_list", { oldName: renameTarget, newName: name });
            else root.invoke("create_list", { name: name });
            renameTarget = ""; listName.clear();
        }
    }
    AppDialog {
        id: deleteListDialog
        title: qsTr("Delete list?"); standardButtons: Dialog.Yes | Dialog.Cancel
        property string listName: ""
        Text { text: qsTr("Delete \"%1\" and its saved items?").arg(deleteListDialog.listName); color: Theme.text }
        onAccepted: root.invoke("delete_list", { name: listName })
    }
    AppDialog {
        id:detailsDialog;standardButtons:Dialog.NoButton;width:430;padding:0;topPadding:0;bottomPadding:0;showClose:false
        property string detailName:"";property string detailIdentifier:"";property string detailExplainer:""
        function show(name,identifier,explainer){detailName=name;detailIdentifier=identifier;detailExplainer=explainer;open()}
        contentItem:Column{spacing:12;padding:18
            Row{spacing:9;LucideIcon{name:"info";width:20;height:20;color:Theme.accent;anchors.verticalCenter:parent.verticalCenter}Text{text:detailsDialog.detailName;color:Theme.text;font.pixelSize:17;font.weight:Font.Bold;anchors.verticalCenter:parent.verticalCenter}}
            Rectangle{width:parent.width-36;height:30;radius:5;color:Theme.field;border.color:Theme.border;Text{anchors.centerIn:parent;text:detailsDialog.detailIdentifier;color:Theme.accent;font.family:"monospace";font.pixelSize:12}}
            Text{width:parent.width-36;text:detailsDialog.detailExplainer;color:Theme.textDim;font.pixelSize:13;wrapMode:Text.WordWrap}
            Row{anchors.right:parent.right;anchors.rightMargin:18;BwButton{text:qsTr("Close");onClicked:detailsDialog.close()}}
        }
    }
    MakeBlockDialog {
        id:blockDialog
        onCreateRequested:(pieces,shape,color,icon,identifier,description)=>root.invoke("create_block",{pieces:pieces,shape:shape,color:color,icon:icon,identifier:identifier,description:description})
        onEditRequested:(blockId,pieces,shape,color,icon,identifier,description)=>root.invoke("edit_block",{blockId:blockId,pieces:pieces,shape:shape,color:color,icon:icon,identifier:identifier,description:description})
    }

    // Blockwork's automation blocks, declared for blockstitch's generic rows.
    readonly property var extraOperators: [
        ["ReadFile", "text", 1, [textValue("~/notes.txt")]], ["FileExists", "bool", 1, [textValue("~/notes.txt")]],
        ["ListDirectory", "text", 1, [textValue("~")]], ["HttpGet", "text", 1, [textValue("https://example.com")]],
        ["JsonGet", "text", 2, [textValue("name"), textValue('{"name": "Blockwork"}')]],
        ["ShellOutput", "text", 1, [textValue("echo hello")]], ["ShellSucceeded", "bool", 1, [textValue("true")]],
        ["ActiveWindowTitle", "text", 0], ["ActiveWindowApp", "text", 0], ["WindowExists", "bool", 1, [textValue("Firefox")]],
        ["LastError", "text", 0], ["MacroRunning", "bool", 1, [textValue("")]], ["GlobalVariable", "text", 1, [textValue(firstGlobalName())]],
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
        ["Trim", "text", 1, [textValue("  hello  ")]], ["FormatNumber", "text", 2, [numberValue(12.3456), numberValue(2)]],
        ["Timestamp", "number", 0], ["FormatDate", "text", 2, [{ kind: "Op", op: "Timestamp", args: [], saved: numberValue(0) }, textValue("%Y-%m-%d %H:%M")]],
        ["ParseDate", "number", 2, [textValue("2026-01-31 09:00"), textValue("")]],
        ["DatePart", "number", 2, [textValue("day"), { kind: "Op", op: "Timestamp", args: [], saved: numberValue(0) }]]
    ]
    readonly property var extraCategories: ({
        CommandWait: "system", WriteFile: "system", HttpRequest: "system", Notify: "system", Log: "system",
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
    // [[value, label]] as dropdown options.
    function choices(pairs) { return pairs.map(p => ({ value: p[0], label: p[1] })); }
    function variableChoices() {
        return [{ value: "", label: qsTr("(no variable)") }].concat((root.macro ? root.macro.variables : []).map(v => ({ value: v, label: v })));
    }
    readonly property var mouseButtonOptions: [["Left", qsTr("left button")], ["Right", qsTr("right button")], ["Middle", qsTr("middle button")],
        ["Back", qsTr("back button")], ["Forward", qsTr("forward button")], ["ScrollUp", qsTr("wheel up")], ["ScrollDown", qsTr("wheel down")],
        ["ScrollLeft", qsTr("wheel left")], ["ScrollRight", qsTr("wheel right")]].map(b => ({ value: b[0], label: b[1] }))
    // What the open macro may do without asking; null when it's trusted.
    readonly property var permissions: appState.permissions === undefined ? null : appState.permissions
    readonly property var guardedAbilities: [
        { kind: "commands", label: qsTr("Run commands and open or close apps") }, { kind: "network", label: qsTr("Go online") },
        { kind: "files_write", label: qsTr("Write to files") }, { kind: "secrets", label: qsTr("Read your saved secrets") },
        { kind: "screen", label: qsTr("Read the screen") }, { kind: "files_read", label: qsTr("Read files and folders") },
        { kind: "input", label: qsTr("Press keys and move the mouse") }, { kind: "clipboard", label: qsTr("Read the clipboard") },
        { kind: "macros", label: qsTr("Run or stop your other macros") }, { kind: "globals", label: qsTr("Read or change global variables") }]
    // Which part of a window the window blocks match against.
    readonly property var windowMatchOptions: [{ value: "Title", label: qsTr("window") }, { value: "App", label: qsTr("app") }]
    // How a detect block orders its results list.
    readonly property var detectionSortOptions: [{ value: "Closest", label: qsTr("closest first") }, { value: "Score", label: qsTr("best first") },
        { value: "Largest", label: qsTr("largest first") }]
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
            { kind: "button", action: "captureImage", image: "image", icon: "crosshair", text: i => i && i.image ? "" : qsTr("capture image") },
            { kind: "button", action: "loadImage", icon: "folder-open" }
        ];
        const value = (field, key) => ({ kind: "value", field: field, key: key });
        const signalPieces = [{ kind: "value", field: "SignalName", key: "name", pickAction: "pickSignal", expression: "expression" }];
        // The detect blocks' search area: the whole window, or a part picked on screen.
        const regionPieces = [
            { kind: "button", action: "pickRegion", icon: "square-dashed", text: i => i && i.region ? qsTr("region %1").arg(i.region) : qsTr("whole window") },
            { kind: "button", action: "clearRegion", icon: "x", when: i => !!i && !!i.region }
        ];
        BlockRegistry.registerOperators({
            ReadFile: { prefix: qsTr("contents of file"), result: "text" }, FileExists: { prefix: qsTr("file"), suffix: qsTr("exists?"), result: "bool" },
            ListDirectory: { prefix: qsTr("files in folder"), result: "text" }, HttpGet: { prefix: qsTr("fetch"), result: "text" },
            JsonGet: { prefix: qsTr("get"), infix: qsTr("from JSON"), result: "text" },
            ShellOutput: { prefix: qsTr("output of command"), result: "text" }, ShellSucceeded: { prefix: qsTr("command"), suffix: qsTr("succeeds?"), result: "bool" },
            ActiveWindowTitle: { prefix: qsTr("active window title"), result: "text" }, ActiveWindowApp: { prefix: qsTr("active app"), result: "text" },
            WindowExists: { prefix: qsTr("window"), suffix: qsTr("exists?"), result: "bool" },
            LastError: { prefix: qsTr("error message"), result: "text" },
            MacroRunning: { prefix: qsTr("macro"), suffix: qsTr("is running?"), result: "bool" },
            GlobalVariable: { prefix: qsTr("global"), result: "text", pickArg: { index: 0, action: "pickGlobal", icon: "chevron-down" } },
            PixelColor: { prefix: qsTr("color at x"), infix: [qsTr("y")], result: "text" },
            AverageColor: { prefix: qsTr("average color at x"), infix: [qsTr("y"), qsTr("width"), qsTr("height")], result: "text" },
            ColorsMatch: { infix: [qsTr("matches"), qsTr("within")], suffix: "%", result: "bool" },
            ScreenText: { prefix: qsTr("text on screen at x"), infix: [qsTr("y"), qsTr("width"), qsTr("height")], result: "text" },
            MousePosition: { prefix: qsTr("mouse"), result: "number", enumArg: { index: 0, options: [{ value: "x", label: qsTr("x") }, { value: "y", label: qsTr("y") }] } },
            ScreenSize: { prefix: qsTr("screen"), result: "number", enumArg: { index: 0, options: [{ value: "width", label: qsTr("width") }, { value: "height", label: qsTr("height") }] } },
            MonitorCount: { prefix: qsTr("number of monitors"), result: "number" },
            MonitorInfo: { prefix: qsTr("monitor"), result: "number",
                enumArg: { index: 1, options: root.choices([["x", qsTr("x")], ["y", qsTr("y")], ["width", qsTr("width")], ["height", qsTr("height")]]) } },
            WindowInfo: { prefix: qsTr("window"), result: "number",
                enumArg: { index: 1, options: root.choices([["x", qsTr("x")], ["y", qsTr("y")], ["width", qsTr("width")], ["height", qsTr("height")], ["center x", qsTr("center x")], ["center y", qsTr("center y")]]) } },
            Detection: { infix: qsTr("of detection"), result: "text",
                enumArg: { index: 0, options: root.choices([["center x", qsTr("center x")], ["center y", qsTr("center y")], ["x", qsTr("x")], ["y", qsTr("y")], ["width", qsTr("width")], ["height", qsTr("height")], ["score", qsTr("score")], ["label", qsTr("label")], ["id", qsTr("id")]]) } },
            AppRunning: { prefix: qsTr("app"), suffix: qsTr("is running?"), result: "bool" },
            IdleSeconds: { prefix: qsTr("seconds idle"), result: "number" },
            ScreenLocked: { prefix: qsTr("screen is locked?"), result: "bool" },
            NetworkConnected: { prefix: qsTr("online?"), result: "bool" },
            UsbConnected: { prefix: qsTr("USB device"), suffix: qsTr("plugged in?"), result: "bool", pickArg: { index: 0, action: "pickUsb", icon: "list" } },
            Secret: { prefix: qsTr("secret"), result: "text", enumArg: { index: 0, options: () => {
                const names = appState.secret_names || [];
                return names.length ? names.map(n => ({ value: n, label: n })) : [{ value: "", label: qsTr("(add one in Settings)") }];
            } } },
            TextCheck: { infix: ["", ""], suffix: "?", result: "bool",
                enumArg: { index: 1, options: root.choices([["contains", qsTr("contains")], ["starts with", qsTr("starts with")], ["ends with", qsTr("ends with")], ["matches regex", qsTr("matches regex")]]) } },
            TextReplace: { prefix: qsTr("replace"), infix: [qsTr("in"), qsTr("with")], result: "text" },
            RegexReplace: { prefix: qsTr("replace regex"), infix: [qsTr("in"), qsTr("with")], result: "text" },
            RegexFind: { prefix: qsTr("match of regex"), infix: qsTr("in"), result: "text" },
            SplitPart: { prefix: qsTr("part"), infix: [qsTr("of"), qsTr("split by")], result: "text" },
            SplitCount: { prefix: qsTr("number of parts in"), infix: qsTr("split by"), result: "number" },
            Substring: { prefix: qsTr("letters"), infix: [qsTr("to"), qsTr("of")], result: "text" },
            Trim: { prefix: qsTr("trim"), result: "text" },
            FormatNumber: { infix: qsTr("with"), suffix: qsTr("decimals"), result: "text" },
            Timestamp: { prefix: qsTr("timestamp now"), result: "number" },
            FormatDate: { prefix: qsTr("date of"), infix: qsTr("as"), result: "text" },
            ParseDate: { prefix: qsTr("timestamp of"), infix: qsTr("in format"), result: "number" },
            DatePart: { infix: qsTr("of"), result: "number",
                enumArg: { index: 0, options: root.choices([["year", qsTr("year")], ["month", qsTr("month")], ["day", qsTr("day")], ["hour", qsTr("hour")], ["minute", qsTr("minute")], ["second", qsTr("second")], ["weekday", qsTr("weekday")]]) } }
        });
        // "when [every week] [Monday] at [09:00]": the recurrence as drop-downs,
        // the time as a value slot that opens the time picker.
        const scheduleOf = i => (i && i.schedule) || { kind: "Daily" };
        const withSchedule = (i, changes) => Object.assign({}, scheduleOf(i), changes);
        const whenTimeHead = [
            label(qsTr("WHEN")),
            { kind: "dropdown", key: "schedule", decode: s => s ? s.kind : "Daily",
              options: root.choices([["Daily", qsTr("every day")], ["Weekly", qsTr("every week on")], ["Monthly", qsTr("every month on day")], ["Yearly", qsTr("every year on")]]),
              encode: (kind, i) => {
                  const s = scheduleOf(i), day = s.day || 1;
                  if (kind === "Weekly") return { kind: kind, weekday: s.weekday || "Monday" };
                  if (kind === "Monthly") return { kind: kind, day: day };
                  if (kind === "Yearly") return { kind: kind, month: s.month || 1, day: day };
                  return { kind: "Daily" };
              } },
            { kind: "dropdown", key: "schedule", when: i => scheduleOf(i).kind === "Weekly", decode: s => s ? s.weekday : "",
              options: root.choices([["Sunday", qsTr("Sunday")], ["Monday", qsTr("Monday")], ["Tuesday", qsTr("Tuesday")], ["Wednesday", qsTr("Wednesday")],
                                     ["Thursday", qsTr("Thursday")], ["Friday", qsTr("Friday")], ["Saturday", qsTr("Saturday")]]),
              encode: (w, i) => withSchedule(i, { weekday: w }) },
            { kind: "dropdown", key: "schedule", when: i => scheduleOf(i).kind === "Yearly", decode: s => s ? String(s.month) : "",
              options: Array.from({ length: 12 }, (_, m) => ({ value: String(m + 1), label: Schedules.monthName(m + 1) })),
              encode: (m, i) => withSchedule(i, { month: Number(m) }) },
            { kind: "dropdown", key: "schedule", when: i => ["Monthly", "Yearly"].indexOf(scheduleOf(i).kind) >= 0, decode: s => s ? String(s.day) : "",
              options: Array.from({ length: 31 }, (_, d) => ({ value: String(d + 1), label: String(d + 1) })),
              encode: (d, i) => withSchedule(i, { day: Number(d) }) },
            label(qsTr("at")),
            Object.assign(value("ScheduleTime", "time"), { pick: { action: "pickTime", icon: "clock", display: t => Schedules.timeLabel(t), tip: qsTr("Click to pick a time, or drop a reporter here") } })
        ];
        BlockRegistry.registerRows({
            WhenRan: { shape: "header", icon: "play", head: [label(qsTr("WHEN RUN"))] },
            WhenAt: { shape: "header", icon: "clock", head: [label(qsTr("ONCE AT")),
                Object.assign(value("AtTime", "at"), { pick: { action: "pickDateTime", icon: "calendar", display: t => Schedules.dateTimeLabel(t), tip: qsTr("Click to pick a date and time, or drop a reporter here") } })] },
            WhenSignal: { shape: "header", icon: "corner-down-right", head: [label(qsTr("WHEN I RECEIVE")), ...signalPieces] },
            WhenWebhook: { shape: "header", icon: "link", head: [label(qsTr("WHEN WEBHOOK")), value("WebhookPath", "path"), label(qsTr("IS CALLED, BODY IN")),
                { kind: "dropdown", key: "variable", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") }] },
            SendSignal: { icon: "navigation", head: [label(qsTr("send signal")), ...signalPieces,
                { kind: "dropdown", key: "wait", options: [{ value: "start", label: qsTr("and continue") }, { value: "wait", label: qsTr("and wait") }],
                  encode: v => v === "wait", decode: v => v ? "wait" : "start" }] },
            AskText: { icon: "text-cursor-input", head: [label(qsTr("ask")), value("AskPrompt", "prompt"), label(qsTr("default")), value("AskDefault", "default"),
                label(qsTr("into")), { kind: "dropdown", key: "name", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") }] },
            AskYesNo: { icon: "circle-check", head: [label(qsTr("ask yes or no")), value("AskPrompt", "prompt"),
                label(qsTr("into")), { kind: "dropdown", key: "name", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") }] },
            AskChoice: { icon: "list-checks", head: [label(qsTr("choose from")), { kind: "dropdown", key: "list", options: "lists" }, label(qsTr("asking")),
                value("AskPrompt", "prompt"), label(qsTr("into")), { kind: "dropdown", key: "name", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") }] },
            ShowMessage: { icon: "message-square", head: [label(qsTr("show message")), value("MessageText", "message")] },
            WhenWindow: { shape: "header", icon: "app-window", head: [label(qsTr("WHEN WINDOW")), value("WindowPattern", "pattern"), label(qsTr("IS")),
                { kind: "dropdown", key: "focused", options: [{ value: "focused", label: qsTr("focused") }, { value: "unfocused", label: qsTr("unfocused") }],
                  encode: v => v === "focused", decode: v => v ? "focused" : "unfocused" }] },
            WhenFileChanged: { shape: "header", icon: "file", head: [label(qsTr("WHEN FILE")), value("WatchedFilePath", "path"), label(qsTr("CHANGES"))] },
            WhenTyped: { shape: "header", icon: "type", head: [label(qsTr("WHEN I TYPE")), value("TypedText", "text"), label(qsTr("THEN")),
                { kind: "dropdown", key: "erase", options: [{ value: "erase", label: qsTr("erase it") }, { value: "keep", label: qsTr("keep it") }],
                  encode: v => v === "erase", decode: v => v ? "erase" : "keep" }] },
            WhenApp: { shape: "header", icon: "app-window", head: [label(qsTr("WHEN APP")), value("AppPattern", "pattern"),
                { kind: "dropdown", key: "started", options: [{ value: "starts", label: qsTr("starts") }, { value: "exits", label: qsTr("exits") }],
                  encode: v => v === "starts", decode: v => v ? "starts" : "exits" }] },
            WhenIdle: { shape: "header", icon: "moon", head: [label(qsTr("WHEN IDLE FOR")), value("IdleMinutes", "minutes"), label(qsTr("MINUTES"))] },
            WhenScreenLock: { shape: "header", icon: "lock", head: [label(qsTr("WHEN SCREEN IS")),
                { kind: "dropdown", key: "locked", options: [{ value: "locked", label: qsTr("locked") }, { value: "unlocked", label: qsTr("unlocked") }],
                  encode: v => v === "locked", decode: v => v ? "locked" : "unlocked" }] },
            WhenCron: { shape: "header", icon: "calendar-clock", head: [label(qsTr("ON SCHEDULE")),
                Object.assign(value("CronExpression", "expression"), { pick: { action: "pickSchedule", icon: "calendar-clock", display: t => Schedules.scheduleLabel(t), tip: qsTr("Click to choose when it runs, or drop a reporter here") } })] },
            WhenTime: { shape: "header", icon: "clock", head: whenTimeHead },
            WhenNetwork: { shape: "header", icon: "radio", head: [label(qsTr("WHEN NETWORK")),
                { kind: "dropdown", key: "connected", options: [{ value: "on", label: qsTr("connects") }, { value: "off", label: qsTr("disconnects") }],
                  encode: v => v === "on", decode: v => v ? "on" : "off" }] },
            WhenUsb: { shape: "header", icon: "plug-zap", head: [label(qsTr("WHEN USB DEVICE")), value("UsbPattern", "pattern"), { kind: "button", action: "pickUsb", icon: "list" }, label(qsTr("IS")),
                { kind: "dropdown", key: "connected", options: [{ value: "in", label: qsTr("plugged in") }, { value: "out", label: qsTr("unplugged") }],
                  encode: v => v === "in", decode: v => v ? "in" : "out" }] },
            WhenMouseButton: { shape: "header", icon: "mouse", head: [label(qsTr("WHEN")),
                { kind: "dropdown", key: "button", options: root.mouseButtonOptions },
                { kind: "dropdown", key: "pressed", options: [{ value: "down", label: qsTr("is pressed") }, { value: "up", label: qsTr("is released") }],
                  encode: v => v === "down", decode: v => v ? "down" : "up" },
                label(qsTr("WITH KEYS")), value("MouseModifiers", "modifiers")] },
            SplitIntoList: { icon: "scissors", head: [label(qsTr("split")), value("SplitText", "text"), label(qsTr("by")), value("SplitSeparator", "separator"),
                label(qsTr("into")), { kind: "dropdown", key: "name", options: "lists" }] },
            PasteText: { icon: "clipboard-paste", head: [label(qsTr("paste")), value("PasteValue", "text")] },
            CommandWait: { icon: "terminal", head: [label(qsTr("Run command")), { kind: "text", key: "command", placeholder: qsTr("command") }, label(qsTr("and wait"))] },
            WithLock: { icon: "lock", mouths: ["body"], head: [label(qsTr("with lock")), value("LockName", "name")] },
            WriteFile: { icon: "file-text", head: [
                { kind: "dropdown", key: "append", options: [{ value: "write", label: qsTr("write") }, { value: "append", label: qsTr("append") }],
                  encode: v => v === "append", decode: v => v ? "append" : "write" },
                value("WriteFileText", "text"), label(qsTr("to file")), value("WriteFilePath", "path")] },
            // Target first, like "set", so it isn't lost at the end of a long row.
            HttpRequest: { icon: "cloud", head: [
                label(qsTr("set")), { kind: "dropdown", key: "variable", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") },
                label(qsTr("to response of")), { kind: "dropdown", key: "method", options: ["GET", "POST", "PUT", "PATCH", "DELETE"].map(m => ({ value: m, label: m })) },
                value("HttpUrl", "url"),
                Object.assign(label(qsTr("with body")), { when: i => !i || i.method !== "GET" }), Object.assign(value("HttpBody", "body"), { when: i => !i || i.method !== "GET" }),
                label(qsTr("headers")), value("HttpHeaders", "headers")] },
            Notify: { icon: "message-square", head: [label(qsTr("notify")), value("NotifyTitle", "title"), label(qsTr("saying")), value("NotifyMessage", "message")] },
            Log: { icon: "list", head: [label(qsTr("log")), value("LogValue", "value")] },
            TryCatch: { icon: "circle-alert", mouths: ["body", "catch_body"], separators: [qsTr("on error")], head: [label(qsTr("try"))] },
            ThrowError: { shape: "cap", icon: "triangle-alert", head: [label(qsTr("throw error")), value("ErrorMessage", "message")] },
            StopMacro: { shape: "cap", icon: "octagon", head: [label(qsTr("stop macro"))] },
            Retry: { icon: "refresh-cw", mouths: ["body"], head: [label(qsTr("try up to")), value("RetryCount", "count"), label(qsTr("times,")),
                value("RetryDelay", "delay"), label(qsTr("ms apart"))] },
            WindowAction: { icon: "app-window", head: [
                { kind: "dropdown", key: "action", options: root.choices([["Focus", qsTr("focus")], ["Minimize", qsTr("minimize")], ["Maximize", qsTr("maximize")], ["Restore", qsTr("restore")], ["Close", qsTr("close")]]) },
                { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern")] },
            MoveWindow: { icon: "move", head: [
                { kind: "dropdown", key: "resize", options: [{ value: "move", label: qsTr("move") }, { value: "resize", label: qsTr("resize") }],
                  encode: v => v === "resize", decode: v => v ? "resize" : "move" },
                { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern"),
                Object.assign(label(qsTr("to x")), { when: i => !i || !i.resize }), Object.assign(label(qsTr("to width")), { when: i => !!i && i.resize }),
                value("WindowX", "x"),
                Object.assign(label(qsTr("y")), { when: i => !i || !i.resize }), Object.assign(label(qsTr("height")), { when: i => !!i && i.resize }),
                value("WindowY", "y")] },
            RunMacro: { icon: "play", head: [label(qsTr("run macro")), value("MacroTarget", "target"),
                { kind: "dropdown", key: "wait", options: [{ value: "start", label: qsTr("and continue") }, { value: "wait", label: qsTr("and wait") }],
                  encode: v => v === "wait", decode: v => v ? "wait" : "start" }] },
            StopMacroNamed: { icon: "square", head: [label(qsTr("stop macro")), value("MacroTarget", "target")] },
            SetGlobal: { icon: "variable", head: [label(qsTr("set global")), { kind: "value", field: "GlobalName", key: "name", pickAction: "pickGlobal" }, label(qsTr("to")), value("GlobalValue", "value")] },
            ChangeGlobal: { icon: "variable", head: [label(qsTr("change global")), { kind: "value", field: "GlobalName", key: "name", pickAction: "pickGlobal" }, label(qsTr("by")), value("GlobalValue", "by")] },
            WaitUntil: { icon: "loader", head: [label(qsTr("wait until")), { kind: "value", field: "Condition", key: "condition", bool: true },
                label(qsTr("for up to")), value("WaitTimeout", "timeout"), label(qsTr("ms"))] },
            FindImage: { icon: "search", head: [label(qsTr("find")), ...imagePieces, label(qsTr("put x in")),
                { kind: "dropdown", key: "x_variable", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") },
                label(qsTr("y in")), { kind: "dropdown", key: "y_variable", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") },
                label(qsTr("within")), value("ImageTolerance", "tolerance"), label("%")] },
            ClickImage: { icon: "mouse-pointer-click", head: [label(qsTr("click")),
                { kind: "dropdown", key: "button", options: [{ value: "Left", label: qsTr("left") }, { value: "Right", label: qsTr("right") }, { value: "Middle", label: qsTr("middle") }] },
                label(qsTr("on")), ...imagePieces, label(qsTr("within")), value("ImageTolerance", "tolerance"), label("%")] },
            WaitForImage: { icon: "eye", head: [label(qsTr("wait until")), ...imagePieces,
                { kind: "dropdown", key: "appear", options: [{ value: "appears", label: qsTr("appears") }, { value: "gone", label: qsTr("disappears") }],
                  encode: v => v === "appears", decode: v => v ? "appears" : "gone" },
                label(qsTr("for up to")), value("WaitTimeout", "timeout"), label(qsTr("ms, within")), value("ImageTolerance", "tolerance"), label("%")] },
            DetectObjectsInWindow: { icon: "aperture", head: [label(qsTr("detect")), value("DetectLabel", "label"), label(qsTr("in")),
                { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern"), ...regionPieces,
                label(qsTr("with")), value("DetectorUrl", "detector"), { kind: "button", action: "pickModel", icon: "box" }, label(qsTr("at least")), value("DetectConfidence", "confidence"), label(qsTr("% into")),
                { kind: "dropdown", key: "results_list", options: "lists" },
                { kind: "dropdown", key: "sort", options: root.detectionSortOptions },
                label(qsTr("count in")), { kind: "dropdown", key: "count_variable", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") }] },
            WaitForDetection: { icon: "eye", head: [label(qsTr("wait until")), value("DetectLabel", "label"),
                { kind: "dropdown", key: "appear", options: [{ value: "appears", label: qsTr("appears") }, { value: "gone", label: qsTr("disappears") }],
                  encode: v => v === "appears", decode: v => v ? "appears" : "gone" },
                label(qsTr("in")), { kind: "dropdown", key: "by", options: root.windowMatchOptions }, value("WindowPattern", "pattern"), ...regionPieces,
                label(qsTr("with")), value("DetectorUrl", "detector"), { kind: "button", action: "pickModel", icon: "box" }, label(qsTr("at least")), value("DetectConfidence", "confidence"),
                label(qsTr("% for up to")), value("WaitTimeout", "timeout"), label(qsTr("ms"))] },
            AimStep: { icon: "crosshair", head: [label(qsTr("aim from center of")), { kind: "dropdown", key: "by", options: root.windowMatchOptions },
                value("WindowPattern", "pattern"), label(qsTr("toward x")), value("AimTargetX", "target_x"), label(qsTr("y")), value("AimTargetY", "target_y"),
                label(qsTr("gain")), value("AimGain", "gain"), label(qsTr("max step")), value("AimMaxStep", "max_step"),
                label(qsTr("deadzone")), value("AimDeadzone", "deadzone")] },
            AimAtDetection: { icon: "target", head: [label(qsTr("aim at item")), value("AimIndex", "index"), label(qsTr("of")),
                { kind: "dropdown", key: "results_list", options: "lists" },
                label(qsTr("gain")), value("AimGain", "gain"), label(qsTr("max step")), value("AimMaxStep", "max_step"),
                label(qsTr("deadzone")), value("AimDeadzone", "deadzone"), label(qsTr("hold")), value("AimSettle", "settle_ms"),
                label(qsTr("ms, up to")), value("WaitTimeout", "timeout"), label(qsTr("ms, result in")),
                { kind: "dropdown", key: "status_variable", options: () => root.variableChoices(), placeholder: qsTr("(no variable)") }] }
        });
        BlockRegistry.registerPrefabs({
            WhenAt: () => ({ at: textValue(Qt.formatDateTime(new Date(Date.now() + 3600000), "yyyy-MM-dd HH:00")) }),
            WhenSignal: () => ({ name: textValue(root.macro && root.macro.signals.length ? root.macro.signals[0] : "go"), expression: false }),
            WhenWebhook: () => ({ path: textValue("hello"), variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            SendSignal: () => ({ name: textValue(root.macro && root.macro.signals.length ? root.macro.signals[0] : "go"), expression: false, wait: true }),
            AskText: () => ({ prompt: textValue(qsTr("What's your name?")), default: textValue(""), name: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            AskYesNo: () => ({ prompt: textValue(qsTr("Continue?")), name: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            AskChoice: () => ({ prompt: textValue(qsTr("Pick one")), list: root.firstListName(), name: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            ShowMessage: { message: textValue(qsTr("Done!")) },
            WhenWindow: { pattern: textValue("Firefox"), focused: true },
            WhenFileChanged: { path: textValue("~/notes.txt") },
            WhenTyped: { text: textValue(";addr"), erase: true },
            WhenApp: { pattern: textValue("firefox"), started: true },
            WhenIdle: { minutes: numberValue(5) },
            WhenScreenLock: { locked: true },
            WhenCron: { expression: textValue("0 9 * * mon-fri") },
            WhenTime: { schedule: { kind: "Daily" }, time: textValue("09:00") },
            WhenNetwork: { connected: true },
            WhenUsb: { pattern: textValue(""), connected: true },
            WhenMouseButton: { button: "Back", pressed: true, modifiers: textValue("") },
            SplitIntoList: () => ({ text: textValue("a,b,c"), separator: textValue(","), name: root.firstListName() }),
            PasteText: { text: textValue(qsTr("Hello!")) },
            WithLock: { name: textValue("keyboard"), body: [] },
            WriteFile: { path: textValue("~/notes.txt"), text: textValue("hello"), append: true },
            HttpRequest: () => ({ method: "GET", url: textValue("https://example.com"), body: textValue(""), headers: textValue(""),
                variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" }),
            Notify: { title: textValue("Blockwork"), message: textValue(qsTr("Done!")) },
            Log: { value: textValue(qsTr("checkpoint")) },
            TryCatch: { body: [], catch_body: [] },
            ThrowError: { message: textValue(qsTr("something went wrong")) },
            StopMacro: {},
            Retry: { count: numberValue(3), delay: numberValue(1000), body: [] },
            WindowAction: { action: "Focus", by: "Title", pattern: textValue("Firefox") },
            MoveWindow: { pattern: textValue("Firefox"), by: "Title", resize: false, x: numberValue(0), y: numberValue(0) },
            RunMacro: () => ({ target: textValue(root.otherMacroName()), wait: true }),
            StopMacroNamed: () => ({ target: textValue(root.otherMacroName()) }),
            SetGlobal: () => ({ name: textValue(firstGlobalName()), value: numberValue(0) }),
            ChangeGlobal: () => ({ name: textValue(firstGlobalName()), by: numberValue(1) }),
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
                gain: numberValue(0.5), max_step: numberValue(40), deadzone: numberValue(6) },
            AimAtDetection: () => ({ results_list: root.firstListName(), index: numberValue(1), gain: numberValue(0.5),
                max_step: numberValue(40), deadzone: numberValue(6), settle_ms: numberValue(150), timeout: numberValue(3000),
                status_variable: root.macro && root.macro.variables.length ? root.macro.variables[0] : "" })
        });
    }
    // The variable dropdowns read macro state through a function, so nudge them.
    readonly property string variablesKey: JSON.stringify([root.macro ? root.macro.variables : [], root.macro ? root.macro.signals : [], appState.secret_names || []])
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
                for (const key of ["body", "then_body", "else_body", "catch_body"]) { const found = walk(i[key]); if (found) return found; }
                for (const branch of i.branches || []) { const found = walk(branch); if (found) return found; }
            }
            return null;
        };
        for (const strand of root.macro ? root.macro.strands : []) { const found = walk(strand.instructions); if (found) return found; }
        return null;
    }
    function blockLabel(id) {
        const ins = findInstruction(id);
        if (!ins) return qsTr("a block");
        if (ins.type === "CallBlock" || ins.type === "BranchCallBlock") { const def = blockDefById(ins.block_id); return def ? qsTr("custom block") : qsTr("a block"); }
        return ins.type.replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase();
    }

    // Click-to-preview on operators: the daemon evaluates, the chip shows the answer.
    // Block labels are read once, so register them again in a new language.
    readonly property string uiLanguage: appState.language || ""
    onUiLanguageChanged: Qt.callLater(() => { BlockRegistry.resetOperators(); registerBlocks(); BlockRegistry.blockMenu = [{ id: "breakpoint", text: qsTr("Toggle breakpoint"), icon: "circle-dot" }]; })
    Component.onCompleted: {
        registerBlocks();
        BlockRegistry.blockMenu = [{ id: "breakpoint", text: qsTr("Toggle breakpoint"), icon: "circle-dot" }];
        BlockRegistry.previewHandler = (key, value) => root.appBridge.previewValue(key, JSON.stringify(value));
        // Name slots share the same picker in instructions and reporters.
        BlockRegistry.pickHandler = (action, location, value, anchor) => {
            if (action === "pickUsb") usbDialog.openFor({ kind: "value", location: location, value: value });
            else if (action === "pickGlobal") globalPicker.openFor(name => root.invoke("put_value", { location: location, value: root.textValue(name) }), value && value.kind === "Text" ? value.value : "");
            else if (action === "pickSignal") root.pickSignal(location, value);
            else if (action === "pickTime") timePicker.openFor(location, value, anchor);
            else if (action === "pickDateTime") dateTimePicker.openFor(location, value, anchor);
            else if (action === "pickSchedule") scheduleDialog.openFor(location, value);
        };
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

    MouseArea {
        objectName: "dragCursor"
        anchors.fill: parent; z: 501
        visible: root.paletteDrag !== null || canvas.dragging
        acceptedButtons: Qt.NoButton
        cursorShape: Qt.ClosedHandCursor
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
                    originBlockId: canvas.dragValueOrigin; dragPreview: true
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
                    valueData: ghost.spec.value; boxed: true; editable: false; dragPreview: true
                    referenceColor: ghost.spec.color || Theme.accent
                    forceBoolean: !!ghost.spec.forceBoolean; callDisplayLabel: ghost.spec.label || qsTr("custom block")
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                }
            }
        }
    }
}
