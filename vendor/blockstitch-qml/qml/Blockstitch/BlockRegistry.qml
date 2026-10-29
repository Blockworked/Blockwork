pragma Singleton
import QtQuick

// What a host's blocks look like, as data - the QML side of the Vue
// frontend's register* calls. Built-in tables cover blockstitch's own
// operators and control blocks; a host adds its vocabulary with
// `registerRows`/`registerOperators`/`registerIcons`/`registerDescriptors`.
QtObject {
    id: registry

    // Names the list/dict reporters and name dropdowns offer. BlockCanvas
    // keeps these in step with the document it shows.
    property var listNames: []
    // Optional host dialog for creating and selecting a list.
    property var listCreationHandler: null
    property var dictNames: []
    // Bump to re-evaluate option lists that read host state through a function.
    property int revision: 0
    // Whether block menus offer "Set Recording Target". Off unless the host has recording.
    property bool recordingTargets: false
    // Kept in step with the canvas so block menus can label themselves.
    property string recordingTargetId: ""
    property var commentedIds: []
    // Host markers on canvas blocks, instruction id -> message (an error
    // from the last run, say). Clicking one emits markClicked.
    property var blockMarks: ({})
    signal markClicked(string instructionId)
    // Instruction ids the host's running strands are on, outlined live.
    property var runningIds: []
    // Debugger: blocks with a breakpoint get a dot (clicking it emits
    // breakpointClicked), and blocks a strand is paused at get an outline.
    property var breakpointIds: []
    property var pausedIds: []
    signal breakpointClicked(string instructionId)
    // JSON of the value location just dropped elsewhere; that chip stays
    // blank until the new state arrives.
    property string settlingValueKey: ""
    // Key-order-independent JSON for a value location, so copies compare equal.
    function locationKey(loc) {
        const canon = v => Array.isArray(v) ? v.map(canon)
            : (v && typeof v === "object") ? Object.keys(v).sort().reduce((o, k) => { o[k] = canon(v[k]); return o; }, {})
            : v;
        return JSON.stringify(canon(loc));
    }
    // Outlined for a moment after BlockCanvas.revealInstruction.
    property string flashId: ""
    function flash(id) { flashId = id; flashTimer.restart(); }
    property var flashTimer: Timer { interval: 1600; onTriggered: registry.flashId = "" }
    // Extra right-click entries a host adds: [{id, text, icon, danger}]. A
    // choice comes back as BlockCanvas.blockMenuAction / canvasMenuAction.
    property var blockMenu: []
    property var canvasMenu: []

    // Click-to-preview: a click on a reporter calls `previewHandler(key, value)`
    // and the host answers with `deliverPreview(key, text, isError)`. The
    // result shows beside the chip that asked, then clears itself.
    property var previewHandler: null
    property var preview: null   // {key, text, error}
    property int previewSerial: 0
    function requestPreview(key, value) { if (previewHandler) previewHandler(key, value); }
    // A button beside an operator's `pickArg` slot, or a value piece's
    // `pick` leaf, calls `pickHandler(action, location, value, anchor)`,
    // `location` being that slot's and `anchor` the item clicked (when there
    // is one), so the host can open its own picker beside it and edit the
    // slot with the result.
    property var pickHandler: null
    function requestPick(action, location, value, anchor) { if (pickHandler) pickHandler(action, location, value, anchor || null); }
    function deliverPreview(key, text, isError) {
        preview = { key: key, text: text, error: !!isError };
        previewTimer.restart();
    }
    function nextPreviewKey() { return "preview-" + (++previewSerial); }
    property var previewTimer: Timer { interval: 2500; onTriggered: registry.preview = null }

    // type -> { shape, icon, head: [piece], mouths: [key], separators: [text] }
    // `head` may be a function(instruction) -> [piece] for a type whose row
    // depends on the instruction. Return the same piece objects each time:
    // a changed array rebuilds the row's controls.
    // piece: {kind:"label",text} | {kind:"value",field,key,index,bool,pick,options,placeholder}
    //      | {kind:"dropdown",key,index,options,placeholder,encode,decode}
    //      | {kind:"text",key,index,placeholder}
    // A piece's `index`, when set, picks one entry of the array held under
    // `key`; without it `key` holds the value itself. A value piece's `pick`
    // shows typed text as a picker button (see ValueChip.pick); `options`
    // gives its Text leaf a dropdown while still accepting reporters. A dropdown's
    // `encode(chosen, instruction)` turns the choice into what's stored.
    // Any piece may carry `when: function(instruction) -> bool`.
    // A row may also list `components`: optional groups of pieces the user
    // adds from the row's "+" button and takes off with each group's "x".
    // component: {key, label, icon, fresh, pieces, present?}. It is on while
    // the instruction holds `key`; adding sets `key` to `fresh` (a value, or
    // function(instruction) -> value), removing deletes it. `present(instruction)`
    // overrides that test. Components follow the head in declared order, or
    // stand where the head has a {kind:"components"} piece.
    // `options` is an array of {value,label}, a function(instruction), or
    // "lists"/"dicts" for the document's names. A choice may carry
    // `action(select)` to open a host picker and call select(value) later.
    property var rows: ({})
    // op -> { prefix, infix (one label, or a list: one per gap), suffix, result, enumArg: {index, options|source}, oneBased: [index],
    //        pickArg: {index, action, icon} (a button after that arg's text field, see pickHandler),
    //        pickOp: {action, icon, tip} (a button after the whole operator; pickHandler gets its own location) }
    // An operator may instead give `layout`: [piece] or function(value) -> [piece],
    // piece {label, arg: -1} | {arg, bool}, `arg` indexing the value's args. Args no
    // piece names are not drawn (a host's generic operator keeps its
    // identity there). Return the same objects each time.
    property var operators: defaultOperators()
    // type -> instruction fields a fresh palette block starts with
    property var prefabs: ({})

    function registerRows(map) { rows = Object.assign({}, rows, map); }
    function registerOperators(map) { operators = Object.assign({}, operators, map); }
    // Back to the built-in operators, labelled in the current language; a
    // host re-registers its own after this.
    function resetOperators() { operators = defaultOperators(); }
    function registerPrefabs(map) { prefabs = Object.assign({}, prefabs, map); }

    function defaultOperators() {
        const mathOptions = [
            {value:"Abs",label:qsTr("abs")},{value:"Floor",label:qsTr("floor")},{value:"Ceiling",label:qsTr("ceiling")},{value:"Sign",label:qsTr("sign")},
            {value:"Sqrt",label:qsTr("sqrt")},{value:"Sin",label:qsTr("sin")},{value:"Cos",label:qsTr("cos")},{value:"Tan",label:qsTr("tan")},
            {value:"Asin",label:qsTr("asin")},{value:"Acos",label:qsTr("acos")},{value:"Atan",label:qsTr("atan")},{value:"Ln",label:qsTr("ln")},
            {value:"Log",label:qsTr("log")},{value:"Log2",label:qsTr("log2")},{value:"EPower",label:qsTr("e ^")},{value:"TenPower",label:"10 ^"}
        ];
        const caseOptions = [{value:"Upper",label:qsTr("uppercase")},{value:"Lower",label:qsTr("lowercase")}];
        const timeOptions = [
            {value:"Year",label:qsTr("year")},{value:"Month",label:qsTr("month")},{value:"Date",label:qsTr("date")},{value:"DayOfWeek",label:qsTr("day of week")},
            {value:"Hour",label:qsTr("hour")},{value:"Minute",label:qsTr("minute")},{value:"Second",label:qsTr("second")}
        ];
        const n = "number", t = "text", b = "bool";
        return {
            Add:{infix:"+",result:n}, Sub:{infix:"−",result:n}, Mul:{infix:"×",result:n}, Div:{infix:"/",result:n}, Mod:{infix:qsTr("mod"),result:n},
            Round:{prefix:qsTr("round"),result:n}, Math:{infix:qsTr("of"),result:n,enumArg:{index:0,options:mathOptions}},
            Random:{prefix:qsTr("pick random"),infix:qsTr("to"),result:n},
            Join:{prefix:qsTr("join"),result:t}, NewLine:{prefix:qsTr("new line"),result:t}, Tab:{prefix:qsTr("tab character"),result:t},
            IndexOf:{prefix:qsTr("index of"),infix:qsTr("in"),result:n}, LastIndexOf:{prefix:qsTr("last index of"),infix:qsTr("in"),result:n},
            LetterOf:{prefix:qsTr("letter"),infix:qsTr("of"),result:t}, Length:{prefix:qsTr("length of"),result:n},
            Case:{infix:qsTr("to"),result:t,enumArg:{index:1,options:caseOptions}},
            Eq:{infix:"=",result:b}, Neq:{infix:"≠",result:b}, Gt:{infix:">",result:b}, Lt:{infix:"<",result:b},
            Gte:{infix:"≥",result:b}, Lte:{infix:"≤",result:b}, And:{infix:qsTr("and"),result:b}, Or:{infix:qsTr("or"),result:b},
            Not:{prefix:qsTr("not"),result:b}, True:{prefix:qsTr("true"),result:b}, False:{prefix:qsTr("false"),result:b},
            CurrentTime:{prefix:qsTr("current"),result:n,enumArg:{index:0,options:timeOptions}},
            ListItem:{prefix:qsTr("item"),infix:qsTr("of"),result:t,enumArg:{index:1,source:"lists"},oneBased:[0]},
            ListItemNumber:{prefix:qsTr("item # of"),infix:qsTr("in"),result:n,enumArg:{index:1,source:"lists"}},
            ListAmount:{prefix:qsTr("amount of"),infix:qsTr("in"),result:n,enumArg:{index:1,source:"lists"}},
            ListLength:{prefix:qsTr("length of"),result:n,enumArg:{index:0,source:"lists"}},
            ListContains:{infix:qsTr("contains"),result:b,enumArg:{index:0,source:"lists"}},
            ListItemExists:{prefix:qsTr("item"),infix:qsTr("exists in"),result:b,enumArg:{index:1,source:"lists"},oneBased:[0]},
            ListIsEmpty:{prefix:qsTr("is"),suffix:qsTr("empty?"),result:b,enumArg:{index:0,source:"lists"}},
            ListAsJson:{suffix:qsTr("as JSON"),result:t,enumArg:{index:0,source:"lists"}},
            DictValue:{prefix:qsTr("value"),infix:qsTr("in"),result:t,enumArg:{index:1,source:"dicts"}},
            DictHasKey:{infix:qsTr("has key"),result:b,enumArg:{index:0,source:"dicts"}},
            DictSize:{prefix:qsTr("size of"),result:n,enumArg:{index:0,source:"dicts"}},
            DictKeys:{prefix:qsTr("keys of"),result:t,enumArg:{index:0,source:"dicts"}},
            DictAsJson:{suffix:qsTr("as JSON"),result:t,enumArg:{index:0,source:"dicts"}},
            DictIsEmpty:{prefix:qsTr("is"),suffix:qsTr("empty?"),result:b,enumArg:{index:0,source:"dicts"}}
        };
    }

    // kind -> operator descriptor from blockstitch-core: arity, each slot's
    // type and default, and the result. The built-ins are generated below;
    // a host adds its registered operators with registerDescriptors.
    property var descriptors: indexDescriptors(builtinDescriptors)
    function indexDescriptors(list) {
        const map = {};
        for (const d of list || []) map[d.kind] = d;
        return map;
    }
    function registerDescriptors(list) { descriptors = Object.assign({}, descriptors, indexDescriptors(list)); }
    function descriptor(kind) { return (kind && descriptors[kind]) || null; }
    // A fresh operator value for a palette entry, or null for a kind with no
    // descriptor. An empty list/dict slot takes the first name (from `names`,
    // {lists, dicts}, when given), and an empty dropdown slot its first choice.
    function freshValue(kind, names) {
        const d = descriptor(kind);
        if (!d) return null;
        const spec = operator(d.op);
        const enumArg = spec && spec.enumArg;
        const args = d.args.map((arg, i) => {
            const v = JSON.parse(JSON.stringify(arg.default));
            if (v.kind !== "Text" || v.value !== "") return v;
            if (arg.names === "list" || arg.names === "dict")
                return { kind: "Text", value: ((arg.names === "list" ? (names && names.lists) || listNames
                                                                     : (names && names.dicts) || dictNames)[0]) || "" };
            if (enumArg && enumArg.index === i)
                return { kind: "Text", value: (enumChoices(enumArg)[0] || { value: "" }).value };
            return v;
        });
        return { kind: "Op", op: d.op, args: args, saved: { kind: "Number", value: 0 } };
    }
    // A fresh call's arguments for a custom block definition: each input's
    // default, else its typed blank. A boolean default becomes True/False.
    // Optional custom groups use ordinary boolean arguments, visible to the body.
    function componentEnabled(value) {
        return !!value && ((value.kind === "Op" && value.op === "True")
            || (value.kind === "Number" && value.value !== 0));
    }
    function componentValue(enabled) {
        return {kind: "Op", op: enabled ? "True" : "False", args: [], saved: {kind: "Bool"}};
    }
    property var valueReplacementHandler: null
    function replaceValue(location, value) {
        if (valueReplacementHandler) valueReplacementHandler(location, value);
    }
    function customCallRow(definition) {
        if (!definition || !(definition.pieces || []).some(p => p.component)) return null;
        const spec = {head: [], components: []};
        let group = null, arg = 0, branched = false;
        for (const p of definition.pieces || []) {
            if (p.kind === "Branch") { group = null; branched = true; continue; }
            if (p.kind === "Input" && p.component) {
                const index = arg++;
                group = {key: p.id, index: index, label: p.name, pieces: [{kind: "label", text: p.name}],
                    present: instruction => registry.componentEnabled((instruction.args || [])[index])};
                spec.components.push(group);
                continue;
            }
            const piece = p.kind === "Input"
                ? {kind: "value", key: "args", index: arg, field: "CallArg:" + arg++, bool: p.value_type === "Bool"}
                : p.kind === "Label" && !branched ? {kind: "label", text: p.text} : null;
            if (piece) (group ? group.pieces : spec.head).push(piece);
        }
        return spec;
    }
    function freshCallArgs(definition) {
        return ((definition && definition.pieces) || []).filter(p => p.kind === "Input").map(p => {
            const d = p.default_value, bool = p.value_type === "Bool";
            if (d && bool && d.kind === "Bool" && typeof d.value === "boolean")
                return { kind: "Op", op: d.value ? "True" : "False", args: [], saved: { kind: "Number", value: 0 } };
            if (d && !bool && ((d.kind === "Number" && typeof d.value === "number" && isFinite(d.value))
                               || (d.kind === "Text" && typeof d.value === "string")))
                return JSON.parse(JSON.stringify(d));
            return bool ? { kind: "Bool" } : { kind: "Number", value: 0 };
        });
    }
    // BEGIN GENERATED operator descriptors (blockstitch-core tests/descriptors.rs; BLESS=1 rewrites)
    readonly property var builtinDescriptors: [
        {"kind":"Add","op":"Add","arity":2,"result":"number","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Sub","op":"Sub","arity":2,"result":"number","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Mul","op":"Mul","arity":2,"result":"number","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Div","op":"Div","arity":2,"result":"number","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Mod","op":"Mod","arity":2,"result":"number","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Round","op":"Round","arity":1,"result":"number","args":[{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Math","op":"Math","arity":2,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":"Abs"},"choices":["Abs","Floor","Ceiling","Sign","Sqrt","Sin","Cos","Tan","Asin","Acos","Atan","Ln","Log","Log2","EPower","TenPower"]},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Random","op":"Random","arity":2,"result":"number","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Join","op":"Join","arity":2,"result":"text","args":[{"type":"text","default":{"kind":"Text","value":""}},{"type":"text","default":{"kind":"Text","value":""}}]},
        {"kind":"Join3","op":"Join","arity":3,"result":"text","args":[{"type":"text","default":{"kind":"Text","value":""}},{"type":"text","default":{"kind":"Text","value":""}},{"type":"text","default":{"kind":"Text","value":""}}]},
        {"kind":"NewLine","op":"NewLine","arity":0,"result":"text","args":[]},
        {"kind":"Tab","op":"Tab","arity":0,"result":"text","args":[]},
        {"kind":"IndexOf","op":"IndexOf","arity":2,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":""}},{"type":"text","default":{"kind":"Text","value":""}}]},
        {"kind":"LastIndexOf","op":"LastIndexOf","arity":2,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":""}},{"type":"text","default":{"kind":"Text","value":""}}]},
        {"kind":"LetterOf","op":"LetterOf","arity":2,"result":"text","args":[{"type":"number","default":{"kind":"Number","value":1.0},"one_based":true},{"type":"text","default":{"kind":"Text","value":""}}]},
        {"kind":"Length","op":"Length","arity":1,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":""}}]},
        {"kind":"Case","op":"Case","arity":2,"result":"text","args":[{"type":"text","default":{"kind":"Text","value":""}},{"type":"text","default":{"kind":"Text","value":"Upper"},"choices":["Upper","Lower"]}]},
        {"kind":"Eq","op":"Eq","arity":2,"result":"bool","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Neq","op":"Neq","arity":2,"result":"bool","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Gt","op":"Gt","arity":2,"result":"bool","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Lt","op":"Lt","arity":2,"result":"bool","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Gte","op":"Gte","arity":2,"result":"bool","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"Lte","op":"Lte","arity":2,"result":"bool","args":[{"type":"number","default":{"kind":"Number","value":0.0}},{"type":"number","default":{"kind":"Number","value":0.0}}]},
        {"kind":"And","op":"And","arity":2,"result":"bool","args":[{"type":"bool","default":{"kind":"Bool"}},{"type":"bool","default":{"kind":"Bool"}}]},
        {"kind":"Or","op":"Or","arity":2,"result":"bool","args":[{"type":"bool","default":{"kind":"Bool"}},{"type":"bool","default":{"kind":"Bool"}}]},
        {"kind":"Not","op":"Not","arity":1,"result":"bool","args":[{"type":"bool","default":{"kind":"Bool"}}]},
        {"kind":"True","op":"True","arity":0,"result":"bool","args":[]},
        {"kind":"False","op":"False","arity":0,"result":"bool","args":[]},
        {"kind":"CurrentTime","op":"CurrentTime","arity":1,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":"Year"},"choices":["Year","Month","Date","DayOfWeek","Hour","Minute","Second"]}]},
        {"kind":"ListItem","op":"ListItem","arity":2,"result":"text","args":[{"type":"number","default":{"kind":"Number","value":1.0},"one_based":true},{"type":"text","default":{"kind":"Text","value":""},"names":"list"}]},
        {"kind":"ListItemNumber","op":"ListItemNumber","arity":2,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":"thing"}},{"type":"text","default":{"kind":"Text","value":""},"names":"list"}]},
        {"kind":"ListAmount","op":"ListAmount","arity":2,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":"thing"}},{"type":"text","default":{"kind":"Text","value":""},"names":"list"}]},
        {"kind":"ListLength","op":"ListLength","arity":1,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"list"}]},
        {"kind":"ListContains","op":"ListContains","arity":2,"result":"bool","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"list"},{"type":"text","default":{"kind":"Text","value":"thing"}}]},
        {"kind":"ListItemExists","op":"ListItemExists","arity":2,"result":"bool","args":[{"type":"number","default":{"kind":"Number","value":1.0},"one_based":true},{"type":"text","default":{"kind":"Text","value":""},"names":"list"}]},
        {"kind":"ListIsEmpty","op":"ListIsEmpty","arity":1,"result":"bool","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"list"}]},
        {"kind":"ListAsJson","op":"ListAsJson","arity":1,"result":"text","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"list"}]},
        {"kind":"DictValue","op":"DictValue","arity":2,"result":"text","args":[{"type":"text","default":{"kind":"Text","value":"key"}},{"type":"text","default":{"kind":"Text","value":""},"names":"dict"}]},
        {"kind":"DictHasKey","op":"DictHasKey","arity":2,"result":"bool","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"dict"},{"type":"text","default":{"kind":"Text","value":"key"}}]},
        {"kind":"DictSize","op":"DictSize","arity":1,"result":"number","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"dict"}]},
        {"kind":"DictKeys","op":"DictKeys","arity":1,"result":"text","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"dict"}]},
        {"kind":"DictAsJson","op":"DictAsJson","arity":1,"result":"text","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"dict"}]},
        {"kind":"DictIsEmpty","op":"DictIsEmpty","arity":1,"result":"bool","args":[{"type":"text","default":{"kind":"Text","value":""},"names":"dict"}]}
    ]
    // END GENERATED operator descriptors

    function operator(op) { return (op && operators[op]) || null; }
    // `result` may be a function(value) for an operator whose type depends on
    // the value (a host's generic operator).
    function isBoolOp(op, value) {
        const s = operator(op);
        if (!s) return false;
        return (typeof s.result === "function" ? s.result(value) : s.result) === "bool";
    }

    // The {value,label} choices an enum slot offers right now.
    function choices(options, instruction) {
        revision;
        if (options === "lists") return listChoices(listNames);
        if (options === "dicts") return nameChoices(dictNames, "dict");
        if (typeof options === "function") return options(instruction) || [];
        return options || [];
    }
    function listChoices(names) {
        const options = nameChoices(names, "list");
        return listCreationHandler ? options.concat([{ label: qsTr("Make a List…"), action: listCreationHandler }]) : options;
    }
    function nameChoices(names, fallback) {
        const list = (names || []).map(n => ({ value: n, label: n }));
        return list.length ? list : [{ value: "", label: fallback }];
    }
    function enumChoices(enumArg) {
        if (!enumArg) return [];
        return choices(enumArg.source || enumArg.options, null);
    }

    function row(type) { return rows[type] || null; }

    // Components, see `rows`. Pieces are cached on the spec so a row keeps
    // the same piece objects, and so its controls, between edits.
    function hasComponent(component, instruction) {
        if (!instruction) return false;
        if (component.present) return !!component.present(instruction);
        const v = instruction[component.key];
        return v !== undefined && v !== null;
    }
    function missingComponents(spec, instruction) {
        return ((spec && spec.components) || []).filter(c => !hasComponent(c, instruction)
            && !(c.conflicts || []).some(key => instruction && instruction[key] !== undefined && instruction[key] !== null));
    }
    function componentPieces(component) {
        if (!component._pieces) component._pieces = (component.pieces || []).concat([{ kind: "removeComponent", component: component }]);
        return component._pieces;
    }
    function addComponentPiece(spec) {
        if (!spec._addPiece) spec._addPiece = { kind: "addComponent" };
        return spec._addPiece;
    }
    // The pieces a row draws for one instruction, components included.
    function rowPieces(spec, instruction, palette) {
        if (!spec || !spec.head) return [];
        const head = typeof spec.head === "function" ? spec.head(instruction) : spec.head;
        const comps = spec.components || [];
        let extra = [];
        for (const c of comps) if (hasComponent(c, instruction)) extra = extra.concat(componentPieces(c));
        if (comps.length && missingComponents(spec, instruction).length) extra.push(addComponentPiece(spec));
        const all = [];
        let placed = false;
        for (const p of (head || [])) {
            if (p.kind === "components") { all.push(...extra); placed = true; }
            else all.push(p);
        }
        if (!placed) all.push(...extra);
        return all.filter(p => !p.when || p.when(instruction));
    }
    // The instruction with a component added or taken off.
    function withComponent(component, instruction) {
        const n = JSON.parse(JSON.stringify(instruction));
        if (component.index !== undefined) {
            n.args[component.index] = componentValue(true);
            return n;
        }
        for (const key of (component.conflicts || [])) delete n[key];
        n[component.key] = typeof component.fresh === "function" ? component.fresh(instruction) : JSON.parse(JSON.stringify(component.fresh === undefined ? "" : component.fresh));
        return n;
    }
    function withoutComponent(component, instruction) {
        const n = JSON.parse(JSON.stringify(instruction));
        if (component.index !== undefined) n.args[component.index] = componentValue(false);
        else delete n[component.key];
        return n;
    }
    // type -> Lucide icon for a type drawn without a registered row.
    property var icons: ({})
    function registerIcons(map) { icons = Object.assign({}, icons, map); }
    readonly property var builtinIcons: ({Wait:"clock",SetVariable:"equal",ChangeVariable:"trending-up",AddToList:"plus",DeleteOfList:"trash",DeleteAllOfList:"trash",ShiftList:"arrow-left",InsertIntoList:"plus",ReplaceItemOfList:"repeat",ReverseList:"rotate",Return:"undo",If:"git-branch",IfElse:"git-fork",Repeat:"repeat",Forever:"infinity",While:"rotate",EscapeLoop:"log-out",ContinueLoop:"skip-forward",BlockHeader:"blocks",CallBlock:"blocks",BranchCallBlock:"blocks",RunBranch:"git-branch"})
    function iconFor(type) {
        const r = row(type);
        if (r && r.icon) return r.icon;
        return icons[type] || builtinIcons[type] || "blocks";
    }

    // ---- shapes ----
    function mouthKeys(type) {
        const r = row(type);
        if (r && r.mouths) return r.mouths;
        if (type === "IfElse") return ["then_body", "else_body"];
        if (["If", "Repeat", "Forever", "While"].indexOf(type) >= 0) return ["body"];
        return [];
    }
    function isWrap(type) { return type === "BranchCallBlock" || mouthKeys(type).length > 0; }
    function isHeader(type) {
        const r = row(type);
        if (r && r.shape) return r.shape === "header";
        return type === "BlockHeader" || (!!type && type.indexOf("When") === 0);
    }
    function isCapType(type) {
        const r = row(type);
        if (r && r.shape) return r.shape === "cap";
        return ["Return", "EscapeLoop", "ContinueLoop"].indexOf(type) >= 0;
    }
    // A call to a custom block shaped "Ending" also ends its stack.
    function isCap(ins, defs) {
        if (!ins) return false;
        if (isCapType(ins.type)) return true;
        if ((ins.type === "CallBlock" || ins.type === "BranchCallBlock") && defs) {
            for (let i = 0; i < defs.length; ++i)
                if (defs[i].id === ins.block_id) return defs[i].shape === "Ending";
        }
        return false;
    }
    function slotCount(ins) {
        if (!ins) return 0;
        if (ins.type === "BranchCallBlock") return (ins.branches || []).length;
        return mouthKeys(ins.type).length;
    }
    function body(ins, slot) {
        if (!ins) return [];
        if (ins.type === "BranchCallBlock") return (ins.branches || [])[slot] || [];
        const key = mouthKeys(ins.type)[slot || 0];
        return (key && ins[key]) || [];
    }
    function separator(type, slot) {
        const r = row(type);
        if (r && r.separators) return r.separators[slot] || "";
        return type === "IfElse" ? "else" : "";
    }

    // A fresh instruction of `type` for the palette, or null when the host
    // registered none.
    function prefab(type) {
        const p = prefabs[type];
        if (!p) return null;
        const i = JSON.parse(JSON.stringify(typeof p === "function" ? p() : p));
        i.type = type;
        if (!i.id) i.id = "palette-" + type;
        return i;
    }
}
