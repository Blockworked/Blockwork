import QtQuick
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockstitch 1.0 as Stitch
import com.blockworked.Blockwork 1.0 as Bw

// Blockwork's block vocabulary, in one place. The palette sections list
// every block once and give it its category; the rest is what blockstitch
// needs to draw and create them: fresh blocks' defaults, the reporters'
// result types and arities, dropdown options and the rows, operators, icons
// and prefabs handed to BlockRegistry. Help texts live in BlockHelp.qml.
//
// Adding a block or a reporter: see CONTRIBUTING.md, which lists every place
// (tests/wire_blocks.rs checks most of them against each other).
QtObject {
    id: catalog
    // The EditorPage these blocks are for: its macro, state and dialogs.
    required property Item page
    readonly property BlockHelp help: BlockHelp {}

    function numberValue(n) { return { kind: "Number", value: n }; }
    function textValue(t) { return { kind: "Text", value: t }; }
    function firstGlobalName() { return (page.appState.globals || []).length ? page.appState.globals[0].name : ""; }
    // A fresh block of this type: its prefab from register(), else
    // blockstitch's defaults for the shared control, variable and list blocks.
    function defaultInstruction(type) {
        const fresh = BlockRegistry.prefab(type) || page.sharedPrefab(type);
        fresh.id = BlockRegistry.newId();
        return fresh;
    }
    readonly property var listInstructionTypes: ["AddToList", "DeleteOfList", "DeleteAllOfList", "ShiftList", "InsertIntoList", "ReplaceItemOfList", "ReverseList", "SplitIntoList"]

    // Blockwork's reporters as blockstitch-core describes them (arity, slot
    // types, defaults), for BlockRegistry.freshValue.
    // BEGIN GENERATED operator descriptors (blockwork-core tests/operator_descriptors.rs; BLESS=1 rewrites)
    readonly property var hostDescriptors: [
        {"kind":"BatteryPercentage","op":"BatteryPercentage","arity":0,"args":[],"host":true},
        {"kind":"PluggedIn","op":"PluggedIn","arity":0,"args":[],"host":true},
        {"kind":"ClipboardText","op":"ClipboardText","arity":0,"args":[],"host":true},
        {"kind":"ClipboardHasImage","op":"ClipboardHasImage","arity":0,"args":[],"host":true},
        {"kind":"ClipboardHasFiles","op":"ClipboardHasFiles","arity":0,"args":[],"host":true},
        {"kind":"ReadFile","op":"ReadFile","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"~/notes.txt"}}],"host":true},
        {"kind":"FileExists","op":"FileExists","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"~/notes.txt"}}],"host":true},
        {"kind":"ListDirectory","op":"ListDirectory","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"~"}}],"host":true},
        {"kind":"HttpGet","op":"HttpGet","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"https://example.com"}}],"host":true},
        {"kind":"JsonGet","op":"JsonGet","arity":2,"args":[{"type":"text","default":{"kind":"Text","value":"name"}},{"type":"text","default":{"kind":"Text","value":"{\"name\": \"Blockwork\"}"}}],"host":true},
        {"kind":"ShellOutput","op":"ShellOutput","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"echo hello"}}],"host":true},
        {"kind":"ShellSucceeded","op":"ShellSucceeded","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"true"}}],"host":true},
        {"kind":"ActiveWindowTitle","op":"ActiveWindowTitle","arity":0,"args":[],"host":true},
        {"kind":"ActiveWindowApp","op":"ActiveWindowApp","arity":0,"args":[],"host":true},
        {"kind":"WindowExists","op":"WindowExists","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"Firefox"}}],"host":true},
        {"kind":"WindowInfo","op":"WindowInfo","arity":2,"args":[{"type":"text","default":{"kind":"Text","value":"Firefox"}},{"type":"text","default":{"kind":"Text","value":"center x"}}],"host":true},
        {"kind":"Detection","op":"Detection","arity":2,"args":[{"type":"text","default":{"kind":"Text","value":"center x"}},{"type":"text","default":{"kind":"Op","op":"ListItem","args":[{"kind":"Number","value":1.0},{"kind":"Text","value":"detections"}],"saved":{"kind":"Number","value":0.0}}}],"host":true},
        {"kind":"MacroRunning","op":"MacroRunning","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":""}}],"host":true},
        {"kind":"GlobalVariable","op":"GlobalVariable","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":""}}],"host":true},
        {"kind":"PixelColor","op":"PixelColor","arity":2,"args":[{"type":"number","default":{"kind":"Number","value":100.0}},{"type":"number","default":{"kind":"Number","value":100.0}}],"host":true},
        {"kind":"AverageColor","op":"AverageColor","arity":4,"args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":100.0}},{"type":"number","default":{"kind":"Number","value":100.0}}],"host":true},
        {"kind":"ColorsMatch","op":"ColorsMatch","arity":3,"args":[{"type":"text","default":{"kind":"Text","value":"#ffffff"}},{"type":"text","default":{"kind":"Text","value":"#fafafa"}},{"type":"number","default":{"kind":"Number","value":10.0}}],"host":true},
        {"kind":"ScreenText","op":"ScreenText","arity":4,"args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":400.0}},{"type":"number","default":{"kind":"Number","value":100.0}}],"host":true},
        {"kind":"MousePosition","op":"MousePosition","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"x"}}],"host":true},
        {"kind":"ScreenSize","op":"ScreenSize","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"width"}}],"host":true},
        {"kind":"MonitorCount","op":"MonitorCount","arity":0,"args":[],"host":true},
        {"kind":"MonitorInfo","op":"MonitorInfo","arity":2,"args":[{"type":"number","default":{"kind":"Number","value":1.0}},{"type":"text","default":{"kind":"Text","value":"width"}}],"host":true},
        {"kind":"AppRunning","op":"AppRunning","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"firefox"}}],"host":true},
        {"kind":"IdleSeconds","op":"IdleSeconds","arity":0,"args":[],"host":true},
        {"kind":"ScreenLocked","op":"ScreenLocked","arity":0,"args":[],"host":true},
        {"kind":"NetworkConnected","op":"NetworkConnected","arity":0,"args":[],"host":true},
        {"kind":"UsbConnected","op":"UsbConnected","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":""}}],"host":true},
        {"kind":"Secret","op":"Secret","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"api token"}}],"host":true},
        {"kind":"BrowserConnected","op":"BrowserConnected","arity":0,"args":[],"host":true},
        {"kind":"PageUrl","op":"PageUrl","arity":0,"args":[],"host":true},
        {"kind":"PageTitle","op":"PageTitle","arity":0,"args":[],"host":true},
        {"kind":"ElementText","op":"ElementText","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"h1"}}],"host":true},
        {"kind":"ElementExists","op":"ElementExists","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"text=Sign in"}}],"host":true},
        {"kind":"ElementCount","op":"ElementCount","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"button"}}],"host":true},
        {"kind":"TextCheck","op":"TextCheck","arity":3,"args":[{"type":"text","default":{"kind":"Text","value":"hello world"}},{"type":"text","default":{"kind":"Text","value":"contains"}},{"type":"text","default":{"kind":"Text","value":"world"}}],"host":true},
        {"kind":"TextReplace","op":"TextReplace","arity":3,"args":[{"type":"text","default":{"kind":"Text","value":"world"}},{"type":"text","default":{"kind":"Text","value":"hello world"}},{"type":"text","default":{"kind":"Text","value":"there"}}],"host":true},
        {"kind":"RegexReplace","op":"RegexReplace","arity":3,"args":[{"type":"text","default":{"kind":"Text","value":"[0-9]+"}},{"type":"text","default":{"kind":"Text","value":"order 1234"}},{"type":"text","default":{"kind":"Text","value":"#"}}],"host":true},
        {"kind":"RegexFind","op":"RegexFind","arity":2,"args":[{"type":"text","default":{"kind":"Text","value":"[0-9]+"}},{"type":"text","default":{"kind":"Text","value":"order 1234"}}],"host":true},
        {"kind":"SplitPart","op":"SplitPart","arity":3,"args":[{"type":"number","default":{"kind":"Number","value":1.0}},{"type":"text","default":{"kind":"Text","value":"a,b,c"}},{"type":"text","default":{"kind":"Text","value":","}}],"host":true},
        {"kind":"SplitCount","op":"SplitCount","arity":2,"args":[{"type":"text","default":{"kind":"Text","value":"a,b,c"}},{"type":"text","default":{"kind":"Text","value":","}}],"host":true},
        {"kind":"Substring","op":"Substring","arity":3,"args":[{"type":"number","default":{"kind":"Number","value":1.0}},{"type":"number","default":{"kind":"Number","value":3.0}},{"type":"text","default":{"kind":"Text","value":"hello"}}],"host":true},
        {"kind":"Trim","op":"Trim","arity":1,"args":[{"type":"text","default":{"kind":"Text","value":"  hello  "}}],"host":true},
        {"kind":"FormatNumber","op":"FormatNumber","arity":2,"args":[{"type":"number","default":{"kind":"Number","value":12.3456}},{"type":"number","default":{"kind":"Number","value":2.0}}],"host":true},
        {"kind":"Timestamp","op":"Timestamp","arity":0,"args":[],"host":true},
        {"kind":"FormatDate","op":"FormatDate","arity":2,"args":[{"type":"text","default":{"kind":"Op","op":"Timestamp","args":[],"saved":{"kind":"Number","value":0.0}}},{"type":"text","default":{"kind":"Text","value":"%Y-%m-%d %H:%M"}}],"host":true},
        {"kind":"ParseDate","op":"ParseDate","arity":2,"args":[{"type":"text","default":{"kind":"Text","value":"2026-01-31 09:00"}},{"type":"text","default":{"kind":"Text","value":""}}],"host":true},
        {"kind":"DatePart","op":"DatePart","arity":2,"args":[{"type":"text","default":{"kind":"Text","value":"day"}},{"type":"text","default":{"kind":"Op","op":"Timestamp","args":[],"saved":{"kind":"Number","value":0.0}}}],"host":true},
        {"kind":"LastError","op":"LastError","arity":0,"args":[],"host":true}
    ]
    // END GENERATED operator descriptors
    // Blockwork's reporters for the palette: [op, result, arity, args?], from
    // operatorRows() and the generated descriptors. A fresh one takes
    // hostDescriptors' defaults unless it gives its own args.
    readonly property var extraOperators: {
        const rows = operatorRows();
        return Object.keys(rows).map(op => {
            const descriptor = hostDescriptors.find(d => d.op === op);
            const spec = [op, rows[op].result, descriptor ? descriptor.arity : 0];
            if (op === "GlobalVariable") spec.push([textValue(firstGlobalName())]);
            return spec;
        });
    }
    // Blockwork's reporters: label pieces and result type, by operator. The
    // palette list above and BlockRegistry both come from this one table.
    function operatorRows() {
        return {
            BatteryPercentage: { prefix: qsTr("battery percentage"), result: "number" }, PluggedIn: { prefix: qsTr("plugged in?"), result: "bool" },
            ClipboardText: { prefix: qsTr("clipboard text"), result: "text" }, ClipboardHasImage: { prefix: qsTr("clipboard has image"), result: "bool" },
            ClipboardHasFiles: { prefix: qsTr("clipboard has files"), result: "bool" },
            ReadFile: { prefix: qsTr("contents of file"), result: "text" }, FileExists: { prefix: qsTr("file"), suffix: qsTr("exists?"), result: "bool" },
            ListDirectory: { prefix: qsTr("files in folder"), result: "text" }, HttpGet: { prefix: qsTr("fetch"), result: "text" },
            JsonGet: { prefix: qsTr("get"), infix: qsTr("from JSON"), result: "text" },
            ShellOutput: { prefix: qsTr("output of command"), result: "text" }, ShellSucceeded: { prefix: qsTr("command"), suffix: qsTr("succeeds?"), result: "bool" },
            ActiveWindowTitle: { prefix: qsTr("active window title"), result: "text" }, ActiveWindowApp: { prefix: qsTr("active app"), result: "text" },
            WindowExists: { prefix: qsTr("window"), suffix: qsTr("exists?"), result: "bool" },
            LastError: { prefix: qsTr("error message"), result: "text" },
            MacroRunning: { prefix: qsTr("macro"), suffix: qsTr("is running?"), result: "bool" },
            GlobalVariable: { prefix: qsTr("global"), result: "text", enumArg: { index: 0, options: () => page.globalChoices() } },
            PixelColor: { prefix: qsTr("color at x"), infix: [qsTr("y")], result: "text",
                pickOp: { action: "pickScreenPoint", icon: "crosshair", tip: qsTr("Click a point on screen") } },
            AverageColor: { prefix: qsTr("average color at x"), infix: [qsTr("y"), qsTr("width"), qsTr("height")], result: "text",
                pickOp: { action: "pickScreenRegion", icon: "square-dashed", tip: qsTr("Drag out a region on screen") } },
            ColorsMatch: { infix: [qsTr("matches"), qsTr("within")], suffix: "%", result: "bool" },
            ScreenText: { prefix: qsTr("text on screen at x"), infix: [qsTr("y"), qsTr("width"), qsTr("height")], result: "text",
                pickOp: { action: "pickScreenRegion", icon: "square-dashed", tip: qsTr("Drag out a region on screen") } },
            MousePosition: { prefix: qsTr("mouse"), result: "number", enumArg: { index: 0, options: [{ value: "x", label: qsTr("x") }, { value: "y", label: qsTr("y") }] } },
            ScreenSize: { prefix: qsTr("screen"), result: "number", enumArg: { index: 0, options: [{ value: "width", label: qsTr("width") }, { value: "height", label: qsTr("height") }] } },
            MonitorCount: { prefix: qsTr("number of monitors"), result: "number" },
            MonitorInfo: { prefix: qsTr("monitor"), result: "number",
                enumArg: { index: 1, options: choices([["x", qsTr("x")], ["y", qsTr("y")], ["width", qsTr("width")], ["height", qsTr("height")]]) } },
            WindowInfo: { prefix: qsTr("window"), result: "number",
                enumArg: { index: 1, options: choices([["x", qsTr("x")], ["y", qsTr("y")], ["width", qsTr("width")], ["height", qsTr("height")], ["center x", qsTr("center x")], ["center y", qsTr("center y")]]) } },
            Detection: { infix: qsTr("of detection"), result: "text",
                enumArg: { index: 0, options: choices([["center x", qsTr("center x")], ["center y", qsTr("center y")], ["x", qsTr("x")], ["y", qsTr("y")], ["width", qsTr("width")], ["height", qsTr("height")], ["score", qsTr("score")], ["label", qsTr("label")], ["id", qsTr("id")]]) } },
            AppRunning: { prefix: qsTr("app"), suffix: qsTr("is running?"), result: "bool" },
            IdleSeconds: { prefix: qsTr("seconds idle"), result: "number" },
            ScreenLocked: { prefix: qsTr("screen is locked?"), result: "bool" },
            NetworkConnected: { prefix: qsTr("online?"), result: "bool" },
            UsbConnected: { prefix: qsTr("USB device"), suffix: qsTr("plugged in?"), result: "bool", pickArg: { index: 0, action: "pickUsb", icon: "list" } },
            Secret: { prefix: qsTr("secret"), result: "text", enumArg: { index: 0, options: () => {
                const names = page.appState.secret_names || [];
                return names.length ? names.map(n => ({ value: n, label: n })) : [{ value: "", label: qsTr("(add one in Settings)") }];
            } } },
            TextCheck: { infix: ["", ""], suffix: "?", result: "bool",
                enumArg: { index: 1, options: choices([["contains", qsTr("contains")], ["starts with", qsTr("starts with")], ["ends with", qsTr("ends with")], ["matches regex", qsTr("matches regex")]]) } },
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
                enumArg: { index: 0, options: choices([["year", qsTr("year")], ["month", qsTr("month")], ["day", qsTr("day")], ["hour", qsTr("hour")], ["minute", qsTr("minute")], ["second", qsTr("second")], ["weekday", qsTr("weekday")]]) } },
            BrowserConnected: { prefix: qsTr("browser connected?"), result: "bool" },
            PageUrl: { prefix: qsTr("page address"), result: "text" }, PageTitle: { prefix: qsTr("page title"), result: "text" },
            ElementText: { prefix: qsTr("text of element"), result: "text" },
            ElementExists: { prefix: qsTr("element"), suffix: qsTr("exists?"), result: "bool" },
            ElementCount: { prefix: qsTr("number of elements matching"), result: "number" }
        };
    }
    // The palette's sections, in order. Statements come before reporters,
    // except where a section holds data: there the reporter comes first.
    readonly property var paletteSections: [
        { category: "events", title: qsTr("Start"), items: ["WhenRan", "WhenSignal", "WhenWebhook"] },
        { category: "events", title: qsTr("Keyboard and mouse"), items: ["WhenKey", "WhenTyped", "WhenMouseButton"] },
        { category: "events", title: qsTr("Time"), items: ["WhenTime", "WhenAt", "WhenCron", "WhenIdle"] },
        { category: "events", title: qsTr("Apps and windows"), items: ["WhenApp", "WhenWindow"] },
        { category: "events", title: qsTr("Files and clipboard"), items: ["WhenFileChanged", "WhenClipboardChanged"] },
        { category: "events", title: qsTr("Computer"), items: ["WhenPowerPluggedIn", "WhenPowerUnplugged", "WhenBatteryDischargedTo", "WhenBatteryChargedTo",
            "WhenScreenLock", "WhenNetwork", "WhenUsb"] },
        { category: "input", title: qsTr("Keyboard"), items: ["Text", "PasteText", "Key"] },
        { category: "input", title: qsTr("Mouse"), items: ["Button", "MoveMouse", "Scroll", "MousePosition"] },
        { category: "control", title: qsTr("Waiting"), items: ["Wait", "WaitUntil"] },
        { category: "control", title: qsTr("Conditions"), items: ["If", "IfElse"] },
        { category: "control", title: qsTr("Loops"), items: ["Repeat", "Forever", "While", "EscapeLoop", "ContinueLoop"] },
        { category: "control", title: qsTr("Errors"), items: ["TryCatch", "Retry", "ThrowError", "LastError"] },
        { category: "control", title: qsTr("Stopping"), items: ["Stop", "Return"] },
        { category: "control", title: qsTr("Other macros"), items: ["RunMacro", "StopMacroNamed", "MacroRunning"] },
        { category: "control", title: qsTr("Signals and locks"), items: ["SendSignal", "WithLock"] },
        { category: "data", title: qsTr("Variables"), action: "makeVariable", items: ["@variables", "SetVariable", "ChangeVariable"] },
        { category: "data", title: qsTr("Global variables"), action: "makeGlobal", actionText: qsTr("Make a Global"), items: ["GlobalVariable", "SetGlobal", "ChangeGlobal"] },
        { category: "data", title: qsTr("Lists"), action: "makeList", items: ["@lists", "ListItem", "ListItemNumber", "ListAmount", "ListLength",
            "ListContains", "ListItemExists", "ListIsEmpty", "ListAsJson", "AddToList", "DeleteOfList", "DeleteAllOfList", "ShiftList",
            "InsertIntoList", "ReplaceItemOfList", "ReverseList", "SplitIntoList"] },
        { category: "data", title: qsTr("Secrets"), items: ["Secret"] },
        { category: "operators", title: qsTr("Math"), items: ["Add", "Sub", "Mul", "Div", "Mod", "Round", "Math", "Random", "FormatNumber"] },
        { category: "operators", title: qsTr("Text"), items: ["Join", "Join3", "NewLine", "Tab", "Length", "LetterOf", "IndexOf", "LastIndexOf",
            "Substring", "Case", "Trim", "TextCheck", "TextReplace", "SplitPart", "SplitCount", "RegexFind", "RegexReplace"] },
        { category: "operators", title: qsTr("Comparisons and logic"), items: ["Eq", "Neq", "Gt", "Lt", "Gte", "Lte", "And", "Or", "Not", "True", "False"] },
        { category: "operators", title: qsTr("Dates and times"), items: ["CurrentTime", "Timestamp", "FormatDate", "ParseDate", "DatePart"] },
        { category: "system", title: qsTr("Apps and commands"), items: ["OpenApp", "CloseApp", "Command", "CommandWait", "AppRunning",
            "ShellOutput", "ShellSucceeded"] },
        { category: "system", title: qsTr("Windows"), items: ["WindowAction", "MoveWindow", "ActiveWindowTitle", "ActiveWindowApp", "WindowExists", "WindowInfo"] },
        { category: "system", title: qsTr("Clipboard"), items: ["ClipboardText", "ClipboardHasImage", "ClipboardHasFiles", "SetClipboard"] },
        { category: "system", title: qsTr("Files"), items: ["WriteFile", "ReadFile", "FileExists", "ListDirectory"] },
        { category: "system", title: qsTr("Web"), items: ["HttpRequest", "HttpGet", "JsonGet"] },
        { category: "system", title: qsTr("Messages and dialogs"), items: ["Notify", "ShowMessage", "AskText", "AskYesNo", "AskChoice", "Log"] },
        { category: "system", title: qsTr("Computer"), items: ["PluggedIn", "BatteryPercentage", "IdleSeconds", "ScreenLocked", "NetworkConnected", "UsbConnected"] },
        { category: "screen", title: qsTr("Images"), items: ["FindImage", "ClickImage", "WaitForImage"] },
        { category: "screen", title: qsTr("Colors and text"), items: ["PixelColor", "AverageColor", "ColorsMatch", "ScreenText"] },
        { category: "screen", title: qsTr("Monitors"), items: ["ScreenSize", "MonitorCount", "MonitorInfo"] },
        { category: "screen", title: qsTr("Object detection"), items: ["DetectObjectsInWindow", "WaitForDetection", "Detection", "AimStep", "AimAtDetection"] },
        { category: "browser", title: qsTr("Tabs"), items: ["BrowserOpen", "BrowserConnected", "PageUrl", "PageTitle"] },
        { category: "browser", title: qsTr("Page elements"), items: ["BrowserClick", "BrowserType", "BrowserWaitFor", "ElementText", "ElementExists", "ElementCount"] },
        { category: "browser", title: qsTr("Scripts"), items: ["BrowserScript"] }
    ]
    // A block's category: the first section that lists it.
    readonly property var categories: {
        const out = {};
        for (const s of paletteSections) for (const name of s.items) if (!out[name]) out[name] = s.category;
        return out;
    }
    function categoryOf(name) { return categories[name] || ""; }
    // [[value, label]] as dropdown options.
    function choices(pairs) { return pairs.map(p => ({ value: p[0], label: p[1] })); }
    readonly property var mouseButtonOptions: [["Left", qsTr("left button")], ["Right", qsTr("right button")], ["Middle", qsTr("middle button")],
        ["Back", qsTr("back button")], ["Forward", qsTr("forward button")], ["ScrollUp", qsTr("wheel up")], ["ScrollDown", qsTr("wheel down")],
        ["ScrollLeft", qsTr("wheel left")], ["ScrollRight", qsTr("wheel right")]].map(b => ({ value: b[0], label: b[1] }))
    // Which part of a window the window blocks match against.
    readonly property var windowMatchOptions: [{ value: "Title", label: qsTr("window") }, { value: "App", label: qsTr("app") }]
    // How a detect block orders its results list.
    readonly property var imagePickOptions: [{ value: "Nth", label: qsTr("match number") },
        { value: "NearMouse", label: qsTr("match nearest the mouse") }, { value: "NearPoint", label: qsTr("match nearest") }]
    readonly property var detectionSortOptions: [{ value: "Closest", label: qsTr("closest first") }, { value: "Score", label: qsTr("best first") },
        { value: "Largest", label: qsTr("largest first") }]
    readonly property string defaultDetector: "http://127.0.0.1:8765/detect"
    // Another macro's name, for the Run macro prefab.
    function otherMacroName() {
        const names = page.appState.macro_names || [];
        const mine = page.macro ? page.macro.name : "";
        return names.find(n => n && n !== mine) || "";
    }
    // BEGIN block rows: one function per palette category.
    // Pieces several rows share, handed to each row function below.
    function rowPieces() {
        const label = text => ({ kind: "label", text: text });
        const commandField = { kind: "text", key: "command", placeholder: qsTr("command"), minWidth: 92, maxWidth: 480 };
        // The image blocks' picture: click to capture from the screen, or load a file.
        const imagePieces = [
            { kind: "button", action: "captureImage", image: "image", icon: "crosshair", text: i => i && i.image ? "" : qsTr("capture image") },
            { kind: "button", action: "loadImage", icon: "folder-open" }
        ];
        const value = (field, key) => ({ kind: "value", field: field, key: key });
        // A text slot drawn as a box of its own under the row, for text with several lines.
        const box = (field, key, caption) => ({ kind: "value", field: field, key: key, box: true, boxLabel: caption });
        const signalPieces = [{ kind: "value", field: "SignalName", key: "name", pickAction: "pickSignal", expression: "expression" }];
        // The detect blocks' search area: the whole window, or a part picked on screen.
        const regionPieces = [
            { kind: "button", action: "pickRegion", icon: "square-dashed", text: i => i && i.region ? qsTr("region %1").arg(i.region) : qsTr("whole window") },
            { kind: "button", action: "clearRegion", icon: "x", when: i => !!i && !!i.region }
        ];
        const timeoutComponent = { key: "timeout", conflicts: ["error_timeout"], label: qsTr("for up to"), icon: "timer", fresh: () => numberValue(10000), pieces: [
            label(qsTr("for up to")), value("WaitTimeout", "timeout"), label(qsTr("ms"))
        ] };
        const errorTimeoutComponent = { key: "error_timeout", conflicts: ["timeout"], label: qsTr("or error after"), icon: "timer", fresh: () => numberValue(10000), pieces: [
            label(qsTr("or error after")), value("WaitErrorTimeout", "error_timeout"), label(qsTr("ms"))
        ] };
        const toleranceComponent = { key: "tolerance", label: qsTr("within %"), icon: "scan", fresh: () => numberValue(10), pieces: [
            label(qsTr("within")), value("ImageTolerance", "tolerance"), label("%")
        ] };
        // The image blocks' optional components, added from the block's "+".
        // The search area, in screen coordinates.
        const regionComponent = { key: "region", label: qsTr("search region"), icon: "square-dashed", fresh: "", pieces: [
            { kind: "button", action: "pickRegion", icon: "square-dashed", text: i => i && i.region ? qsTr("in region %1").arg(i.region) : qsTr("in a region (pick it)") }
        ] };
        // Which match to use when the picture is on screen more than once; the best without this.
        const picked = (mode, piece) => Object.assign(piece, { when: i => !!i && !!i.pick && i.pick.mode === mode });
        const pickComponent = { key: "pick", label: qsTr("which match"), icon: "list-ordered",
            fresh: () => ({ mode: "Nth", index: numberValue(1), near_x: numberValue(0), near_y: numberValue(0) }), pieces: [
            { kind: "dropdown", key: "pick", index: "mode", options: imagePickOptions },
            picked("Nth", { kind: "value", field: "ImageIndex", key: "pick", index: "index" }),
            picked("NearPoint", label(qsTr("x"))), picked("NearPoint", { kind: "value", field: "ImageNearX", key: "pick", index: "near_x" }),
            picked("NearPoint", label(qsTr("y"))), picked("NearPoint", { kind: "value", field: "ImageNearY", key: "pick", index: "near_y" })
        ] };
        // Which of the browser selector's matches to use; the first without this.
        const browserMatchComponent = { key: "nth", label: qsTr("which match"), icon: "list-ordered", fresh: () => numberValue(1), pieces: [
            label(qsTr("match")), value("BrowserNth", "nth")
        ] };
        const countComponent = { key: "count_variable", label: qsTr("count matches"), icon: "hash",
            fresh: () => page.firstVariableName(), pieces: [
            label(qsTr("count in")), { kind: "dropdown", key: "count_variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }
        ] };
        // "when [every week] [Monday] at [09:00]": the recurrence as drop-downs,
        // the time as a value slot that opens the time picker.
        const scheduleOf = i => (i && i.schedule) || { kind: "Daily" };
        const withSchedule = (i, changes) => Object.assign({}, scheduleOf(i), changes);
        const whenTimeHead = [
            label(qsTr("WHEN")),
            { kind: "dropdown", key: "schedule", decode: s => s ? s.kind : "Daily",
              options: choices([["Daily", qsTr("every day")], ["Weekly", qsTr("every week on")], ["Monthly", qsTr("every month on day")], ["Yearly", qsTr("every year on")]]),
              encode: (kind, i) => {
                  const s = scheduleOf(i), day = s.day || 1;
                  if (kind === "Weekly") return { kind: kind, weekday: s.weekday || "Monday" };
                  if (kind === "Monthly") return { kind: kind, day: day };
                  if (kind === "Yearly") return { kind: kind, month: s.month || 1, day: day };
                  return { kind: "Daily" };
              } },
            { kind: "dropdown", key: "schedule", when: i => scheduleOf(i).kind === "Weekly", decode: s => s ? s.weekday : "",
              options: choices([["Sunday", qsTr("Sunday")], ["Monday", qsTr("Monday")], ["Tuesday", qsTr("Tuesday")], ["Wednesday", qsTr("Wednesday")],
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
        const launchSummary = i => {
            const l = i && i.launch;
            if (!l) return "";
            const args = (l.args || "").trim();
            if (args) return args.length > 28 ? args.slice(0, 27) + "…" : args;
            return l.working_dir || (l.env && l.env.length) ? qsTr("custom") : "";
        };
        const direction = (pairs) => ({ kind: "dropdown", key: "direction", options: choices(pairs) });
        const keyValue = { kind: "value", field: "KeyValue", key: "key", keyCapture: true };
        return { label, commandField, imagePieces, value, box, signalPieces, regionPieces, timeoutComponent, errorTimeoutComponent, toleranceComponent, regionComponent, pickComponent, browserMatchComponent, countComponent, whenTimeHead, launchSummary, direction, keyValue };
    }
    // Triggers (the When... headers).
    function eventRows(p) {
        const { label, value, signalPieces, whenTimeHead, direction, keyValue } = p;
        return {
            WhenBatteryDischargedTo: { shape: "header", head: [label(qsTr("WHEN BATTERY DISCHARGED TO")),
                value("BatteryDischargeThreshold", "threshold"), label("%")] },
            WhenBatteryChargedTo: { shape: "header", head: [label(qsTr("WHEN BATTERY CHARGED TO")),
                value("BatteryChargeThreshold", "threshold"), label("%")] },
            WhenPowerPluggedIn: { shape: "header", head: [label(qsTr("WHEN POWER PLUGGED IN"))] },
            WhenPowerUnplugged: { shape: "header", head: [label(qsTr("WHEN POWER UNPLUGGED"))] },
            WhenClipboardChanged: { shape: "header", head: [label(qsTr("WHEN CLIPBOARD CHANGES"))] },
            WhenKey: { shape: "header", head: [label(qsTr("WHEN")), keyValue,
                direction([["Press", qsTr("Pressed")], ["Release", qsTr("Released")]])] },
            WhenRan: { shape: "header", icon: "play", head: [label(qsTr("WHEN RUN"))] },
            WhenAt: { shape: "header", icon: "clock", head: [label(qsTr("ONCE AT")),
                Object.assign(value("AtTime", "at"), { pick: { action: "pickDateTime", icon: "calendar", display: t => Schedules.dateTimeLabel(t), tip: qsTr("Click to pick a date and time, or drop a reporter here") } })] },
            WhenSignal: { shape: "header", icon: "corner-down-right", head: [label(qsTr("WHEN I RECEIVE")), ...signalPieces] },
            WhenWebhook: { shape: "header", icon: "link", head: [label(qsTr("WHEN WEBHOOK")), value("WebhookPath", "path"), label(qsTr("IS CALLED, BODY IN")),
                { kind: "dropdown", key: "variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }] },
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
            WhenMouseButton: { shape: "header", icon: "mouse", components: [
                { key: "modifiers", label: qsTr("with keys"), icon: "keyboard", fresh: () => textValue("Control"),
                  present: i => !!i && !!i.modifiers && (i.modifiers.kind !== "Text" || i.modifiers.value.trim().length > 0), pieces: [
                    label(qsTr("WITH KEYS")), value("MouseModifiers", "modifiers")
                ] }
            ], head: [label(qsTr("WHEN")),
                { kind: "dropdown", key: "button", options: mouseButtonOptions },
                { kind: "dropdown", key: "pressed", options: [{ value: "down", label: qsTr("is pressed") }, { value: "up", label: qsTr("is released") }],
                  encode: v => v === "down", decode: v => v ? "down" : "up" }] }
        };
    }

    // Keyboard and mouse.
    function inputRows(p) {
        const { label, value, direction, keyValue } = p;
        return {
            Text: {
                head: [label(qsTr("Text")), value("TextValue", "text")],
                components: [{
                    key: "speed", label: qsTr("typing speed"), icon: "timer",
                    fresh: () => ({ mode: "PerCharacter", value: numberValue(50) }),
                    pieces: [
                        label(qsTr("typing")),
                        { kind: "dropdown", key: "speed", index: "mode",
                          options: choices([["PerCharacter", qsTr("per character")], ["Total", qsTr("total time")]]) },
                        Object.assign(value("TextSpeed", "speed"), { index: "value", unit: qsTr("ms") })
                    ]
                }]
            },
            Key: { head: [label(qsTr("Key")), keyValue,
                direction([["Click", qsTr("Click")], ["Press", qsTr("Press")], ["Release", qsTr("Release")]])] },
            Button: { head: [label(qsTr("Mouse button")),
                { kind: "dropdown", key: "button", options: choices([["Left", qsTr("Left")], ["Right", qsTr("Right")], ["Middle", qsTr("Middle")],
                    ["Side", qsTr("Side")], ["Extra", qsTr("Extra")]]) },
                direction([["Click", qsTr("Click")], ["Press", qsTr("Press")], ["Release", qsTr("Release")]])] },
            MoveMouse: { head: [label(qsTr("Move mouse")),
                { kind: "dropdown", key: "coordinate", options: choices([["Relative", qsTr("Relative")], ["Absolute", qsTr("Absolute")]]) },
                Object.assign(value("MoveMouseX", "x"), { unit: "x" }), Object.assign(value("MoveMouseY", "y"), { unit: "y" })] },
            Scroll: { head: [label(qsTr("Scroll")), Object.assign(value("ScrollAmount", "amount"), { unit: qsTr("amount") }),
                { kind: "dropdown", key: "axis", options: choices([["Vertical", qsTr("Vertical")], ["Horizontal", qsTr("Horizontal")]]) }] },
            PasteText: { icon: "clipboard-paste", head: [label(qsTr("paste")), value("PasteValue", "text")] }
        };
    }

    // Waiting, errors, other macros, signals and locks.
    function controlRows(p) {
        const { label, value, signalPieces, timeoutComponent, errorTimeoutComponent } = p;
        return {
            Wait: { head: [label(qsTr("Wait")), Object.assign(value("WaitDuration", "duration"), { unit: qsTr("ms") })] },
            SendSignal: { icon: "navigation", head: [label(qsTr("send signal")), ...signalPieces,
                { kind: "dropdown", key: "wait", options: [{ value: "start", label: qsTr("and continue") }, { value: "wait", label: qsTr("and wait") }],
                  encode: v => v === "wait", decode: v => v ? "wait" : "start" }] },
            WithLock: { icon: "lock", mouths: ["body"], head: [label(qsTr("with lock")), value("LockName", "name")] },
            TryCatch: { icon: "circle-alert", mouths: ["body", "catch_body"], separators: [qsTr("on error")], head: [label(qsTr("try"))] },
            ThrowError: { shape: "cap", icon: "triangle-alert", head: [label(qsTr("throw error")), value("ErrorMessage", "message")] },
            Stop: { shape: i => i.scope === "OtherStrands" ? "stack" : "cap", icon: "octagon", head: [label(qsTr("stop")),
                { kind: "dropdown", key: "scope", options: choices([["All", qsTr("all")], ["ThisStrand", qsTr("this strand")], ["OtherStrands", qsTr("other strands in macro")]]) }] },
            Retry: { icon: "refresh-cw", mouths: ["body"], head: [label(qsTr("try up to")), value("RetryCount", "count"), label(qsTr("times,")),
                value("RetryDelay", "delay"), label(qsTr("ms apart"))] },
            RunMacro: { icon: "play", head: [label(qsTr("run macro")), value("MacroTarget", "target"),
                { kind: "dropdown", key: "wait", options: [{ value: "start", label: qsTr("and continue") }, { value: "wait", label: qsTr("and wait") }],
                  encode: v => v === "wait", decode: v => v ? "wait" : "start" }] },
            StopMacroNamed: { icon: "square", head: [label(qsTr("stop macro")), value("MacroTarget", "target")] },
            WaitUntil: { icon: "loader", components: [timeoutComponent, errorTimeoutComponent],
                head: [label(qsTr("wait until")), { kind: "value", field: "Condition", key: "condition", bool: true }] }
        };
    }

    // Variables, globals and lists.
    function dataRows(p) {
        const { label, value } = p;
        return {
            SetVariable: { head: [label(qsTr("set")),
                { kind: "dropdown", key: "name", options: () => page.variableChoices() },
                label(qsTr("to")), value("SetVariableValue", "value")] },
            ChangeVariable: { head: [label(qsTr("change")),
                { kind: "dropdown", key: "name", options: () => page.variableChoices() },
                label(qsTr("by")), value("ChangeVariableValue", "value")] },
            SplitIntoList: { icon: "scissors", head: [label(qsTr("split")), value("SplitText", "text"), label(qsTr("by")), value("SplitSeparator", "separator"),
                label(qsTr("into")), { kind: "dropdown", key: "name", options: "lists" }] },
            SetGlobal: { icon: "variable", head: [label(qsTr("set global")), { kind: "value", field: "GlobalName", key: "name", options: () => page.globalChoices(), placeholder: qsTr("(no global variable)") }, label(qsTr("to")), value("GlobalValue", "value")] },
            ChangeGlobal: { icon: "variable", head: [label(qsTr("change global")), { kind: "value", field: "GlobalName", key: "name", options: () => page.globalChoices(), placeholder: qsTr("(no global variable)") }, label(qsTr("by")), value("GlobalValue", "by")] }
        };
    }

    // Apps, windows, clipboard, files, web and dialogs.
    function systemRows(p) {
        const { label, commandField, value, box, launchSummary } = p;
        return {
            CloseApp: { head: [label(qsTr("Close app")),
                { kind: "button", action: "pickApp", palette: true, image: "icon", icon: "square-x", text: i => i && i.name ? i.name : qsTr("Choose app...") }] },
            SetClipboard: { head: [label(qsTr("set clipboard to")), value("SetClipboardValue", "value")] },
            AskText: { icon: "text-cursor-input", head: [label(qsTr("ask")), value("AskPrompt", "prompt"), label(qsTr("default")), value("AskDefault", "default"),
                label(qsTr("into")), { kind: "dropdown", key: "name", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }] },
            AskYesNo: { icon: "circle-check", head: [label(qsTr("ask yes or no")), value("AskPrompt", "prompt"),
                label(qsTr("into")), { kind: "dropdown", key: "name", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }] },
            AskChoice: { icon: "list-checks", head: [label(qsTr("choose from")), { kind: "dropdown", key: "list", options: "lists" }, label(qsTr("asking")),
                value("AskPrompt", "prompt"), label(qsTr("into")), { kind: "dropdown", key: "name", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }] },
            ShowMessage: { icon: "message-square", head: [label(qsTr("show message")), value("MessageText", "message")] },
            // The app, then a gear for its launch options, labelled with the
            // arguments or just that something else is set.
            OpenApp: { icon: "app-window", head: [label(qsTr("Open app")),
                { kind: "button", action: "pickApp", palette: true, image: "icon", icon: "app-window", text: i => i && i.name ? i.name : qsTr("Choose app...") },
                { kind: "button", action: "launchOptions", icon: "settings", text: launchSummary }] },
            Command: { icon: "terminal", head: [label(qsTr("Run command")), commandField] },
            CommandWait: { icon: "terminal", head: [label(qsTr("Run command")), commandField, label(qsTr("and wait"))] },
            WriteFile: { icon: "file-text", head: [
                { kind: "dropdown", key: "append", options: [{ value: "write", label: qsTr("write") }, { value: "append", label: qsTr("append") }],
                  encode: v => v === "append", decode: v => v ? "append" : "write" },
                label(qsTr("to file")), value("WriteFilePath", "path"), box("WriteFileText", "text", qsTr("text"))] },
            // Target first, like "set", so it isn't lost at the end of a long row.
            HttpRequest: { icon: "cloud", head: [
                label(qsTr("set")), { kind: "dropdown", key: "variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") },
                label(qsTr("to response of")), { kind: "dropdown", key: "method", options: ["GET", "POST", "PUT", "PATCH", "DELETE"].map(m => ({ value: m, label: m })) },
                value("HttpUrl", "url"),
                Object.assign(box("HttpBody", "body", qsTr("body")), { when: i => !i || i.method !== "GET" }),
                box("HttpHeaders", "headers", qsTr("headers"))] },
            Notify: { icon: "message-square", head: [label(qsTr("notify")), value("NotifyTitle", "title"), box("NotifyMessage", "message", qsTr("saying"))] },
            Log: { icon: "list", head: [label(qsTr("log")), value("LogValue", "value")] },
            WindowAction: { icon: "app-window", head: [
                { kind: "dropdown", key: "action", options: choices([["Focus", qsTr("focus")], ["Minimize", qsTr("minimize")], ["Maximize", qsTr("maximize")], ["Restore", qsTr("restore")], ["Close", qsTr("close")]]) },
                { kind: "dropdown", key: "by", options: windowMatchOptions }, value("WindowPattern", "pattern")] },
            MoveWindow: { icon: "move", head: [
                { kind: "dropdown", key: "resize", options: [{ value: "move", label: qsTr("move") }, { value: "resize", label: qsTr("resize") }],
                  encode: v => v === "resize", decode: v => v ? "resize" : "move" },
                { kind: "dropdown", key: "by", options: windowMatchOptions }, value("WindowPattern", "pattern"),
                Object.assign(label(qsTr("to x")), { when: i => !i || !i.resize }), Object.assign(label(qsTr("to width")), { when: i => !!i && i.resize }),
                value("WindowX", "x"),
                Object.assign(label(qsTr("y")), { when: i => !i || !i.resize }), Object.assign(label(qsTr("height")), { when: i => !!i && i.resize }),
                value("WindowY", "y")] }
        };
    }

    // Images and object detection.
    function screenRows(p) {
        const { label, imagePieces, value, box, regionPieces, timeoutComponent, errorTimeoutComponent, toleranceComponent, regionComponent, pickComponent, countComponent } = p;
        return {
            FindImage: { icon: "search", head: [label(qsTr("find")), ...imagePieces, label(qsTr("put x in")),
                { kind: "dropdown", key: "x_variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") },
                label(qsTr("y in")), { kind: "dropdown", key: "y_variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }],
                components: [toleranceComponent, pickComponent, regionComponent, countComponent] },
            ClickImage: { icon: "mouse-pointer-click", head: [label(qsTr("click")),
                { kind: "dropdown", key: "button", options: [{ value: "Left", label: qsTr("left") }, { value: "Right", label: qsTr("right") }, { value: "Middle", label: qsTr("middle") }] },
                label(qsTr("on")), ...imagePieces],
                components: [toleranceComponent, pickComponent, regionComponent] },
            WaitForImage: { icon: "eye", components: [timeoutComponent, errorTimeoutComponent, toleranceComponent, regionComponent], head: [label(qsTr("wait until")), ...imagePieces,
                { kind: "dropdown", key: "appear", options: [{ value: "appears", label: qsTr("appears") }, { value: "gone", label: qsTr("disappears") }],
                  encode: v => v === "appears", decode: v => v ? "appears" : "gone" }] },
            DetectObjectsInWindow: { icon: "aperture", head: [label(qsTr("detect")), value("DetectLabel", "label"), label(qsTr("in")),
                { kind: "dropdown", key: "by", options: windowMatchOptions }, value("WindowPattern", "pattern"), ...regionPieces,
                label(qsTr("with")), value("DetectorUrl", "detector"), { kind: "button", action: "pickModel", icon: "box" }, label(qsTr("at least")), value("DetectConfidence", "confidence"), label(qsTr("% into")),
                { kind: "dropdown", key: "results_list", options: "lists" },
                { kind: "dropdown", key: "sort", options: detectionSortOptions },
                label(qsTr("count in")), { kind: "dropdown", key: "count_variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }] },
            WaitForDetection: { icon: "eye", head: [label(qsTr("wait until")), value("DetectLabel", "label"),
                { kind: "dropdown", key: "appear", options: [{ value: "appears", label: qsTr("appears") }, { value: "gone", label: qsTr("disappears") }],
                  encode: v => v === "appears", decode: v => v ? "appears" : "gone" },
                label(qsTr("in")), { kind: "dropdown", key: "by", options: windowMatchOptions }, value("WindowPattern", "pattern"), ...regionPieces,
                label(qsTr("with")), value("DetectorUrl", "detector"), { kind: "button", action: "pickModel", icon: "box" }, label(qsTr("at least")), value("DetectConfidence", "confidence"),
                label(qsTr("% for up to")), value("WaitTimeout", "timeout"), label(qsTr("ms"))] },
            AimStep: { icon: "crosshair", head: [label(qsTr("aim from center of")), { kind: "dropdown", key: "by", options: windowMatchOptions },
                value("WindowPattern", "pattern"), label(qsTr("toward x")), value("AimTargetX", "target_x"), label(qsTr("y")), value("AimTargetY", "target_y"),
                label(qsTr("gain")), value("AimGain", "gain"), label(qsTr("max step")), value("AimMaxStep", "max_step"),
                label(qsTr("deadzone")), value("AimDeadzone", "deadzone")] },
            AimAtDetection: { icon: "target", head: [label(qsTr("aim at item")), value("AimIndex", "index"), label(qsTr("of")),
                { kind: "dropdown", key: "results_list", options: "lists" },
                label(qsTr("gain")), value("AimGain", "gain"), label(qsTr("max step")), value("AimMaxStep", "max_step"),
                label(qsTr("deadzone")), value("AimDeadzone", "deadzone"), label(qsTr("hold")), value("AimSettle", "settle_ms"),
                label(qsTr("ms, up to")), value("WaitTimeout", "timeout"), label(qsTr("ms, result in")),
                { kind: "dropdown", key: "status_variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") }] }
        };
    }

    // The browser extension.
    function browserRows(p) {
        const { label, value, box, browserMatchComponent } = p;
        return {
            BrowserOpen: { icon: "external-link", head: [label(qsTr("open")), value("BrowserUrl", "url"),
                { kind: "dropdown", key: "new_tab", options: [{ value: "here", label: qsTr("in this tab") }, { value: "new", label: qsTr("in a new tab") }],
                  encode: v => v === "new", decode: v => v ? "new" : "here" }] },
            BrowserClick: { icon: "mouse-pointer-click", head: [label(qsTr("click element")), value("BrowserSelector", "selector")],
                components: [browserMatchComponent] },
            BrowserType: { icon: "text-cursor-input", head: [label(qsTr("type")), value("BrowserText", "text"), label(qsTr("into element")),
                value("BrowserSelector", "selector")], components: [browserMatchComponent] },
            BrowserWaitFor: { icon: "eye", head: [label(qsTr("wait until element")), value("BrowserSelector", "selector"),
                { kind: "dropdown", key: "appear", options: [{ value: "appears", label: qsTr("appears") }, { value: "gone", label: qsTr("disappears") }],
                  encode: v => v === "appears", decode: v => v ? "appears" : "gone" },
                label(qsTr("for up to")), value("WaitTimeout", "timeout"), label(qsTr("ms"))] },
            BrowserScript: { icon: "square-code", head: [label(qsTr("set")), { kind: "dropdown", key: "variable", options: () => page.variableChoices(), placeholder: qsTr("(no variable)") },
                label(qsTr("to result of JavaScript")), Object.assign(box("BrowserCode", "code", ""), { monospace: true })] }
        };
    }
    // END block rows
    // Hands every row, operator, icon and prefab to blockstitch's BlockRegistry.
    // Labels are read once, so the page calls this again in a new language.
    function register() {
        // Blockwork's own icon set covers every icon below; the blocks draw
        // with blockstitch's, so hand it the ones it lacks.
        Stitch.Icons.add(Bw.Icons.paths);
        // Blockwork's own features and vocabulary; a neutral blockstitch host has none of these.
        BlockRegistry.strandMark = { text: qsTr("Recording target"), color: "" };
        BlockRegistry.registerDescriptors(hostDescriptors);
        BlockRegistry.registerIcons({
            WhenRan: "play", WhenBatteryDischargedTo: "battery-warning", WhenBatteryChargedTo: "battery-charging", WhenTime: "clock",
            WhenPowerPluggedIn: "plug-zap", WhenPowerUnplugged: "unplug", WhenClipboardChanged: "clipboard-check", WhenKey: "keyboard",
            Text: "text-cursor", Key: "keyboard", Button: "mouse-pointer-click", MoveMouse: "move", Scroll: "mouse", Command: "terminal",
            OpenApp: "app-window", CloseApp: "square-x", SetClipboard: "clipboard", Wait: "clock"
        });
        BlockRegistry.registerOperators(operatorRows());
        const p = rowPieces();
        const rows = Object.assign({}, eventRows(p), inputRows(p), controlRows(p), dataRows(p), systemRows(p), screenRows(p), browserRows(p));
        BlockRegistry.registerRows(rows);
        BlockRegistry.registerVariableFields({
            WhenWebhook: ["variable"], AskText: ["name"], AskYesNo: ["name"], AskChoice: ["name"],
            HttpRequest: ["variable"], BrowserScript: ["variable"],
            FindImage: ["x_variable", "y_variable", "count_variable"],
            DetectObjectsInWindow: ["count_variable"], AimAtDetection: ["status_variable"]
        });
        BlockRegistry.registerPrefabs({
            Command: { command: "" },
            WhenBatteryDischargedTo: { threshold: numberValue(20) },
            WhenBatteryChargedTo: { threshold: numberValue(100) },
            Wait: { duration: numberValue(1000) },
            Text: { text: textValue("text") },
            Key: () => ({ key: textValue(page.sidebarKey), direction: "Click" }),
            WhenKey: () => ({ key: textValue(page.sidebarKey), direction: "Press" }),
            Button: { button: "Left", direction: "Click" },
            MoveMouse: { x: numberValue(0), y: numberValue(0), coordinate: "Relative" },
            Scroll: { amount: numberValue(4), axis: "Vertical" },
            OpenApp: { command: "", name: "", icon: null },
            CloseApp: { command: "", name: "", icon: null },
            SetClipboard: { value: textValue("text") },
            CommandWait: { command: "" },
            WhenAt: () => ({ at: textValue(Qt.formatDateTime(new Date(Date.now() + 3600000), "yyyy-MM-dd HH:00")) }),
            WhenSignal: () => ({ name: textValue(page.macro && page.macro.signals.length ? page.macro.signals[0] : "go"), expression: false }),
            WhenWebhook: () => ({ path: textValue("hello"), variable: page.firstVariableName() }),
            SendSignal: () => ({ name: textValue(page.macro && page.macro.signals.length ? page.macro.signals[0] : "go"), expression: false, wait: true }),
            AskText: () => ({ prompt: textValue(qsTr("What's your name?")), default: textValue(""), name: page.firstVariableName() }),
            AskYesNo: () => ({ prompt: textValue(qsTr("Continue?")), name: page.firstVariableName() }),
            AskChoice: () => ({ prompt: textValue(qsTr("Pick one")), list: page.firstListName(), name: page.firstVariableName() }),
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
            WhenMouseButton: { button: "Back", pressed: true },
            SplitIntoList: () => ({ text: textValue("a,b,c"), separator: textValue(","), name: page.firstListName() }),
            PasteText: { text: textValue(qsTr("Hello!")) },
            WithLock: { name: textValue("keyboard"), body: [] },
            WriteFile: { path: textValue("~/notes.txt"), text: textValue("hello"), append: true },
            HttpRequest: () => ({ method: "GET", url: textValue("https://example.com"), body: textValue(""), headers: textValue(""),
                variable: page.firstVariableName() }),
            Notify: { title: textValue("Blockwork"), message: textValue(qsTr("Done!")) },
            Log: { value: textValue(qsTr("checkpoint")) },
            TryCatch: { body: [], catch_body: [] },
            ThrowError: { message: textValue(qsTr("something went wrong")) },
            Stop: { scope: "All" },
            Retry: { count: numberValue(3), delay: numberValue(1000), body: [] },
            WindowAction: { action: "Focus", by: "Title", pattern: textValue("Firefox") },
            MoveWindow: { pattern: textValue("Firefox"), by: "Title", resize: false, x: numberValue(0), y: numberValue(0) },
            RunMacro: () => ({ target: textValue(otherMacroName()), wait: true }),
            StopMacroNamed: () => ({ target: textValue(otherMacroName()) }),
            SetGlobal: () => ({ name: textValue(firstGlobalName()), value: numberValue(0) }),
            ChangeGlobal: () => ({ name: textValue(firstGlobalName()), by: numberValue(1) }),
            WaitUntil: { condition: { kind: "Bool" } },
            FindImage: () => ({ image: "", x_variable: page.firstVariableName(),
                y_variable: page.firstVariableName() }),
            ClickImage: { image: "", button: "Left" },
            WaitForImage: { image: "", appear: true },
            DetectObjectsInWindow: () => ({ pattern: textValue("Minecraft"), by: "Title", detector: textValue(defaultDetector),
                label: textValue("player"), confidence: numberValue(50), sort: "Closest", results_list: page.firstListName(), region: "",
                count_variable: page.firstVariableName() }),
            WaitForDetection: { pattern: textValue("Minecraft"), by: "Title", detector: textValue(defaultDetector),
                label: textValue("player"), confidence: numberValue(50), appear: true, timeout: numberValue(10000), region: "" },
            AimStep: { pattern: textValue("Minecraft"), by: "Title", target_x: numberValue(0), target_y: numberValue(0),
                gain: numberValue(0.5), max_step: numberValue(40), deadzone: numberValue(6) },
            AimAtDetection: () => ({ results_list: page.firstListName(), index: numberValue(1), gain: numberValue(0.5),
                max_step: numberValue(40), deadzone: numberValue(6), settle_ms: numberValue(150), timeout: numberValue(3000),
                status_variable: page.firstVariableName() }),
            BrowserOpen: { url: textValue("https://example.com"), new_tab: false },
            BrowserClick: { selector: textValue("text=Sign in") },
            BrowserType: { text: textValue(qsTr("Hello!")), selector: textValue("focused") },
            BrowserWaitFor: { selector: textValue("text=Done"), appear: true, timeout: numberValue(10000) },
            BrowserScript: () => ({ code: textValue("document.title"), variable: page.firstVariableName() })
        });
    }
}
