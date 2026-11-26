import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockstitch 1.0 as Stitch

AppDialog {
    id: root
    objectName: "windowDesigner"
    required property Item page
    title: qsTr("Design Window")
    width: Math.min(1080, parent ? parent.width - 32 : 1080)
    height: Math.min(720, parent ? parent.height - 32 : 720)
    property var request: null
    property var draft: null
    property string original: ""
    property string macroId: ""
    property string selectedId: ""
    property string errorText: ""
    property bool saving: false
    readonly property var rows: draft ? outline(draft.body, []) : []
    readonly property var selected: rows.find(row => row.block.id === selectedId)
    readonly property var block: selected ? selected.block : draft
    readonly property var addParentPath: selected
        ? (isContainer(selected.block) ? selected.path : selected.path.slice(0, -1)) : []
    readonly property var addParent: draft ? at(draft, addParentPath) : null
    readonly property var addChoices: addParent?.layout === "tabs" ? [
        {name: qsTr("Tab"), type: "WindowLayout", layout: "group", icon: "layout-panel-top", group: qsTr("Layout")}
    ] : parts
    readonly property var moveChoices: selected ? destinations(selected) : []
    readonly property var variables: page.macroVariables
    readonly property var lists: page.macroLists.map(list => list.name)
    readonly property var parts: [
        { name: qsTr("Column"), type: "WindowLayout", layout: "column", icon: "panel-left", group: qsTr("Layout") },
        { name: qsTr("Row"), type: "WindowLayout", layout: "row", icon: "panel-bottom", group: qsTr("Layout") },
        { name: qsTr("Grid"), type: "WindowLayout", layout: "grid", icon: "layout-grid", group: qsTr("Layout") },
        { name: qsTr("Scroll area"), type: "WindowLayout", layout: "scroll", icon: "list", group: qsTr("Layout") },
        { name: qsTr("Group"), type: "WindowLayout", layout: "group", icon: "folder", group: qsTr("Layout") },
        { name: qsTr("Spacer"), type: "WindowLayout", layout: "spacer", icon: "minus", group: qsTr("Layout") },
        { name: qsTr("Tabs"), type: "WindowLayout", layout: "tabs", icon: "layout-panel-top", group: qsTr("Layout") },
        { name: qsTr("Split panes"), type: "WindowLayout", layout: "split", icon: "columns-2", group: qsTr("Layout") },
        { name: qsTr("Heading"), type: "WindowDisplay", display: "heading", icon: "type", group: qsTr("Display") },
        { name: qsTr("Subheading"), type: "WindowDisplay", display: "subheading", icon: "heading-2", group: qsTr("Display") },
        { name: qsTr("Text"), type: "WindowDisplay", display: "body", icon: "file-text", group: qsTr("Display") },
        { name: qsTr("Caption"), type: "WindowDisplay", display: "caption", icon: "info", group: qsTr("Display") },
        { name: qsTr("Divider"), type: "WindowDisplay", display: "divider", icon: "minus", group: qsTr("Display") },
        { name: qsTr("Icon"), type: "WindowDisplay", display: "icon", icon: "star", group: qsTr("Display") },
        { name: qsTr("Table"), type: "WindowDisplay", display: "table", icon: "layout-grid", group: qsTr("Display") },
        { name: qsTr("Progress bar"), type: "WindowDisplay", display: "progress", icon: "gauge", group: qsTr("Display") },
        { name: qsTr("Bar chart"), type: "WindowDisplay", display: "chart", icon: "sliders-horizontal", group: qsTr("Display") },
        { name: qsTr("Image"), type: "WindowDisplay", display: "image", icon: "image", group: qsTr("Display") },
        { name: qsTr("Label"), type: "WindowNote", icon: "file-text", group: qsTr("Display") },
        { name: qsTr("Text box"), type: "WindowTextBox", icon: "text-cursor-input", group: qsTr("Input") },
        { name: qsTr("Checkbox"), type: "WindowCheckbox", icon: "circle-check", group: qsTr("Input") },
        { name: qsTr("Drop-down"), type: "WindowChoice", icon: "chevron-down", group: qsTr("Input") },
        { name: qsTr("Slider"), type: "WindowInput", input: "slider", icon: "sliders-horizontal", group: qsTr("Input") },
        { name: qsTr("Multiline text"), type: "WindowInput", input: "multiline", icon: "file-text", group: qsTr("Input") },
        { name: qsTr("Selectable list"), type: "WindowInput", input: "list", icon: "list", group: qsTr("Input") },
        { name: qsTr("Selectable table"), type: "WindowInput", input: "table", icon: "table", group: qsTr("Input") },
        { name: qsTr("File picker"), type: "WindowInput", input: "file", icon: "folder-open", group: qsTr("Input") },
        { name: qsTr("Folder picker"), type: "WindowInput", input: "folder", icon: "folder", group: qsTr("Input") },
        { name: qsTr("Button"), type: "WindowButton", icon: "square-mouse-pointer", group: qsTr("Actions") },
        { name: qsTr("When edited"), type: "WindowOnChange", icon: "refresh-cw", group: qsTr("Actions") }
    ]
    readonly property var fields: block ? [
        { key: "label", name: qsTr("Label") }, { key: "title", name: qsTr("Window title") },
        { key: "text", name: qsTr("Text") }, { key: "window", name: qsTr("Window ID") },
        { key: "width", name: qsTr("Width") }, { key: "height", name: qsTr("Height") },
        { key: "key", name: qsTr("Component ID") },
        { key: "columns", name: qsTr("Grid columns") }, { key: "data", name: qsTr("Data") },
        { key: "options", name: qsTr("List options") },
        { key: "size", name: qsTr("Size") }, { key: "icon", name: qsTr("Icon") }
    ].filter(field => block[field.key] !== undefined
        && (field.key !== "columns" || block.layout === "grid")
        && (field.key !== "data" || (block.display !== "heading" && block.display !== "divider"))
        && (field.key !== "options" || block.input === "list" || block.input === "table")
        && (field.key !== "size" || block.type === "WindowLayout" || ["WindowDisplay", "WindowInput", "WindowTextBox", "WindowCheckbox", "WindowChoice", "WindowButton"].includes(block.type))
        && (field.key !== "icon" || framed(block)
            || (block.type === "WindowDisplay" && block.display !== "divider" && block.display !== "icon")
            || ["WindowInput", "WindowButton"].includes(block.type))) : []

    function clone(value) { return JSON.parse(JSON.stringify(value)); }
    function openFor(target) {
        request = target;
        original = JSON.stringify(target.instruction);
        draft = JSON.parse(original);
        macroId = page.macro.id;
        selectedId = "";
        errorText = "";
        previewHost.input = Object.create(null);
        open();
    }
    function outline(body, path, collapsed) {
        const result = [];
        for (let i = 0; i < body.length; ++i) {
            const block = body[i], next = path.concat(i);
            result.push({block, path: next, siblings: body.length});
            if (isContainer(block) && !collapsed?.[block.id])
                result.push(...outline(block.body, next, collapsed));
        }
        return result;
    }
    function at(window, path) { return path.reduce((parent, index) => parent.body[index], window); }
    function isContainer(block) { return block.type === "WindowLayout" && block.layout !== "spacer"; }
    // Layouts that draw a label header, so icon and frame have somewhere to go.
    function framed(block) {
        return !!block && block.type === "WindowLayout"
            && ["column", "row", "grid", "group"].indexOf(block.layout) >= 0;
    }
    function partFor(block) {
        return parts.find(item => item.type === block.type && item.layout === block.layout
            && item.display === block.display && item.input === block.input);
    }
    function partName(block) { return partFor(block)?.name ?? block.type; }
    // A standalone icon display keeps its glyph in `data`, not `icon`.
    function iconKey(block) {
        return block?.type === "WindowDisplay" && block.display === "icon" ? "data" : "icon";
    }
    function componentName(block) {
        return String(displayValue(block.label ?? block.text) || partName(block));
    }
    function destinations(source) {
        const currentParent = source.path.slice(0, -1);
        const choices = currentParent.length ? [{id: "", name: qsTr("Window"), icon: "app-window", depth: 0}] : [];
        for (const row of rows) {
            if (!isContainer(row.block) || row.path.join("/") === currentParent.join("/")) continue;
            if (source.path.every((index, depth) => index === row.path[depth])) continue;
            if (row.block.layout === "tabs" && source.block.layout !== "group") continue;
            choices.push({id: row.block.id, name: componentName(row.block), depth: row.path.length,
                icon: partFor(row.block)?.icon ?? "folder"});
        }
        return choices;
    }
    function moveInto(id) {
        if (!selected || !moveChoices.some(choice => choice.id === id)) return;
        const next = clone(draft), source = selected.path;
        const [item] = at(next, source.slice(0, -1)).body.splice(source[source.length - 1], 1);
        const destination = id ? outline(next.body, []).find(row => row.block.id === id) : null;
        at(next, destination ? destination.path : []).body.push(item);
        draft = next;
    }
    function displayValue(value, fallback) {
        if (typeof value === "string") return value;
        if (value && (value.kind === "Text" || value.kind === "Number")) return value.value;
        return fallback ?? (value && value.kind === "Var" ? value.name : "");
    }
    function fieldText(key) {
        const current = block[key];
        return String(displayValue(current, current && current.kind === "Var" ? current.name : ""));
    }
    function setField(key, value) {
        if (!draft || JSON.stringify(block[key]) === JSON.stringify(value)) return;
        const next = clone(draft);
        at(next, selected ? selected.path : [])[key] = value;
        draft = next;
    }
    function setLiteral(key, text) {
        const current = block[key];
        if (typeof current === "string") setField(key, text);
        else if (["width", "height", "columns"].includes(key)) {
            if (text.trim() && Number.isFinite(Number(text))) setField(key, page.numberValue(Number(text)));
        } else setField(key, page.textValue(text));
    }
    function add(part) {
        const next = clone(draft);
        const insertingInto = selected && isContainer(block);
        const parent = at(next, addParentPath);
        if (parent.layout === "tabs" && part.layout !== "group") return;
        const child = page.defaultInstruction(part.type);
        for (const key of ["layout", "display", "input"]) {
            if (part[key]) child[key] = part[key];
        }
        if (part.display || part.input || child.layout === "group")
            child.label = page.textValue(part.name);
        const index = selected && !insertingInto ? selected.path[selected.path.length - 1] + 1 : parent.body.length;
        parent.body.splice(index, 0, child);
        draft = next;
        selectedId = child.id;
    }
    function reposition(offset) {
        if (!selected) return;
        const next = clone(draft);
        const body = at(next, selected.path.slice(0, -1)).body;
        const index = selected.path[selected.path.length - 1], target = index + offset;
        if (offset && (target < 0 || target >= body.length)) return;
        const [item] = body.splice(index, 1);
        if (offset) body.splice(target, 0, item);
        else selectedId = "";
        draft = next;
    }
    function previewControls(body) {
        return body.map(previewControl).filter(node => node !== null);
    }
    function previewControl(item) {
        const node = {id: item.id, label: String(displayValue(item.label ?? item.text)), name: item.name || "",
            size: displayValue(item.size, ""), icon: displayValue(item.icon, ""), tone: item.tone || "",
            variant: item.variant || "", style: item.style || ""};
        switch (item.type) {
        case "WindowLayout":
            return Object.assign(node, {kind: item.layout, columns: Number(displayValue(item.columns, 2)), children: previewControls(item.body)});
        case "WindowDisplay": {
            const samples = {progress: 68, table: '[["Name","Value"],["Sample",42]]', chart: "[3,7,4,8]", icon: "star"};
            const value = displayValue(item.data, samples[item.display] ?? "");
            return Object.assign(node, {kind: item.display, value: value === "" ? samples[item.display] ?? "" : value});
        }
        case "WindowNote": return Object.assign(node, {kind: "note"});
        case "WindowButton": return Object.assign(node, {kind: "button", close: item.close});
        case "WindowTextBox": return Object.assign(node, {kind: "text", value: ""});
        case "WindowCheckbox": return Object.assign(node, {kind: "check", value: false});
        case "WindowChoice": {
            const list = page.macroLists.find(list => list.name === item.list);
            return Object.assign(node, {kind: "choice", options: list ? list.items.map(item => String(item.value ?? "")) : []});
        }
        case "WindowInput": {
            const value = item.input === "slider" ? 50 : "";
            const data = displayValue(item.options, "[]");
            if (item.input === "list" || item.input === "table")
                return Object.assign(node, {kind: item.input === "table" ? "table-input" : "list", value, data});
            let options = [];
            try { options = JSON.parse(data); } catch (_) {}
            return Object.assign(node, {kind: item.input, value, options: Array.isArray(options) ? options.filter(entry => typeof entry === "string") : []});
        }
        default: return null;
        }
    }
    function save() {
        if (saving || !draft) return;
        contentItem.forceActiveFocus();
        if (!page.macro || page.macro.id !== macroId || page.recordingBusy
            || JSON.stringify(page.instructionForRequest(request)) !== original) {
            errorText = qsTr("The window block changed. Reopen it to continue editing.");
            return;
        }
        if (JSON.stringify(draft) === original) { close(); return; }
        saving = true;
        page.daemonCall("edit_instruction", {strandId: request.strandId, path: request.path, instruction: draft}, (_, error) => {
            saving = false;
            if (error) errorText = String(error);
            else close();
        });
    }
    showClose: !saving
    closePolicy: saving ? Popup.NoAutoClose : Popup.CloseOnEscape
    onClosed: {
        request = null;
        draft = null;
    }

    component FieldLabel: Label {
        color: Theme.text
        font.pixelSize: 13
        textFormat: Text.PlainText
        Layout.fillWidth: true
        wrapMode: Text.Wrap
    }

    contentItem: ColumnLayout {
        enabled: !root.saving
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 14
            WindowDesignerOutline {
                designer: root
                Layout.preferredWidth: 222
                Layout.maximumWidth: 250
                Layout.minimumWidth: 155
                Layout.fillHeight: true
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumWidth: 180
                color: Theme.panel
                border.color: Theme.borderSoft
                radius: Theme.radius
                ScrollView {
                    id: preview
                    anchors.fill: parent
                    anchors.margins: 14
                    contentWidth: availableWidth
                    clip: true
                    ColumnLayout {
                        width: preview.availableWidth
                        FieldLabel { text: root.draft ? root.displayValue(root.draft.title) : ""; font.bold: true }
                        MacroFormLayout {
                            Layout.fillWidth: true
                            Layout.leftMargin: 3
                            Layout.rightMargin: 3
                            form: previewHost
                            availableHeight: preview.availableHeight
                            nodes: root.draft ? root.previewControls(root.draft.body) : []
                        }
                        QtObject {
                            id: previewHost
                            property var formUpdates: ({})
                            property var input: Object.create(null)
                            readonly property var selectionPath: root.selected ? root.selected.path.map((_, depth) =>
                                root.at(root.draft, root.selected.path.slice(0, depth + 1)).id) : []
                            readonly property var request: ({live: true, preview: true})
                            function properties(node) { return ({}); }
                            function controlValue(node) { return node.id in input ? input[node.id] : node.value; }
                            function setFormValue(node, value) { input = Object.assign(Object.create(null), input, {[node.id]: value}); }
                            function click(node) { root.selectedId = node.id; }
                            function localPath(url) { return ""; }
                        }
                    }
                }
            }
            ScrollView {
                id: inspector
                Layout.preferredWidth: 225
                Layout.maximumWidth: 260
                Layout.minimumWidth: 160
                Layout.fillHeight: true
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: inspector.availableWidth
                    spacing: 8
                    FieldLabel { font.bold: true; text: root.selected ? root.partName(root.block) : qsTr("Window") }
                    Repeater {
                        model: root.fields
                        delegate: ColumnLayout {
                            id: propertyEditor
                            required property var modelData
                            readonly property string key: modelData.key
                            readonly property var current: root.block ? root.block[key] : null
                            readonly property bool literal: typeof current === "string" || (current && ["Text", "Number"].includes(current.kind))
                            Layout.fillWidth: true
                            spacing: 4
                            FieldLabel { text: propertyEditor.modelData.name }
                            StitchTextField {
                                Layout.fillWidth: true
                                visible: propertyEditor.literal
                                text: root.block ? root.fieldText(propertyEditor.key) : ""
                                onEditingFinished: root.setLiteral(propertyEditor.key, text)
                            }
                            FieldLabel {
                                visible: !propertyEditor.literal
                                text: propertyEditor.current && propertyEditor.current.kind === "Var"
                                    ? propertyEditor.current.name : qsTr("Expression from blocks")
                                color: Theme.textDim
                            }
                            StitchComboBox {
                                visible: ["label", "text", "data", "options"].includes(propertyEditor.key)
                                    && (propertyEditor.literal || propertyEditor.current?.kind === "Var") && root.variables.length > 0
                                Layout.fillWidth: true
                                model: root.variables
                                displayText: qsTr("Bind to variable…")
                                currentIndex: -1
                                onActivated: index => {
                                    root.setField(propertyEditor.key, {kind: "Var", name: model[index]});
                                    currentIndex = -1;
                                }
                            }
                            StitchButton {
                                visible: propertyEditor.current?.kind === "Var"
                                text: qsTr("Use a fixed value")
                                onClicked: root.setLiteral(propertyEditor.key, "")
                            }
                        }
                    }
                    FieldLabel { visible: !!root.selected && typeof root.block?.name === "string"; text: qsTr("Variable") }
                    StitchComboBox {
                        visible: !!root.selected && typeof root.block?.name === "string"
                        Layout.fillWidth: true
                        model: root.variables
                        currentIndex: root.block ? root.variables.indexOf(root.block.name) : -1
                        onActivated: index => root.setField("name", model[index])
                    }
                    FieldLabel { visible: root.block?.type === "WindowChoice"; text: qsTr("List") }
                    StitchComboBox {
                        visible: root.block?.type === "WindowChoice"
                        Layout.fillWidth: true
                        model: root.lists
                        currentIndex: root.block ? root.lists.indexOf(root.block.list) : -1
                        onActivated: index => root.setField("list", model[index])
                    }
                    FieldLabel { visible: root.block?.type === "WindowButton"; text: qsTr("Look") }
                    StitchComboBox {
                        visible: root.block?.type === "WindowButton"
                        Layout.fillWidth: true
                        model: [qsTr("auto"), qsTr("primary"), qsTr("quiet"), qsTr("danger")]
                        currentIndex: ["", "primary", "quiet", "danger"].indexOf(root.block?.variant ?? "")
                        onActivated: index => root.setField("variant", ["", "primary", "quiet", "danger"][index])
                    }
                    FieldLabel { visible: root.block?.type === "WindowDisplay" && ["heading", "subheading", "body", "caption", "icon"].includes(root.block?.display); text: qsTr("Tone") }
                    StitchComboBox {
                        visible: root.block?.type === "WindowDisplay" && ["heading", "subheading", "body", "caption", "icon"].includes(root.block?.display)
                        Layout.fillWidth: true
                        model: [qsTr("default"), qsTr("muted"), qsTr("accent"), qsTr("success"), qsTr("warning"), qsTr("danger")]
                        currentIndex: ["", "muted", "accent", "success", "warning", "danger"].indexOf(root.block?.tone ?? "")
                        onActivated: index => root.setField("tone", ["", "muted", "accent", "success", "warning", "danger"][index])
                    }
                    FieldLabel { visible: framed(root.block); text: qsTr("Frame") }
                    StitchComboBox {
                        visible: framed(root.block)
                        Layout.fillWidth: true
                        model: [qsTr("plain"), qsTr("card"), qsTr("panel")]
                        currentIndex: ["", "card", "panel"].indexOf(root.block?.style ?? "")
                        onActivated: index => root.setField("style", ["", "card", "panel"][index])
                    }
                    StitchButton {
                        visible: root.block?.type === "WindowButton" || root.block?.type === "WindowDisplay" || root.block?.type === "WindowInput"
                        Layout.fillWidth: true
                        text: qsTr("Pick icon…")
                        onClicked: {
                            iconHost.blockIcon = root.displayValue(root.block?.[root.iconKey(root.block)], "");
                            iconHost.iconSearch = "";
                            iconPicker.open();
                        }
                    }
                    StitchCheckBox {
                        visible: root.block?.type === "WindowButton"
                        text: qsTr("Close window on click")
                        checked: root.block?.close === true
                        onToggled: root.setField("close", checked)
                    }
                    FieldLabel {
                        visible: root.block?.type === "WindowButton" || root.block?.type === "WindowOnChange"
                        color: Theme.textDim
                        text: qsTr("%n action block(s). Edit actions on the block canvas.", "", root.block?.body?.length ?? 0)
                    }
                }
            }
        }
        Item {
            id: iconHost
            visible: false
            width: 0
            height: 0
            property string iconSearch: ""
            readonly property var allIcons: Object.keys(Stitch.Icons.paths).sort()
            property string blockIcon: ""
        }
        BlockIconPopup {
            id: iconPicker
            editor: iconHost
            onClosed: {
                const key = root.iconKey(root.block);
                if (iconHost.blockIcon && iconHost.blockIcon !== root.displayValue(root.block?.[key], ""))
                    root.setField(key, page.textValue(iconHost.blockIcon));
            }
        }
        FieldLabel { visible: !!root.errorText; text: root.errorText; color: Theme.danger }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            StitchButton { text: qsTr("Cancel"); enabled: !root.saving; onClicked: root.close() }
            StitchButton { text: qsTr("Apply"); primary: true; enabled: !root.saving; onClicked: root.save() }
        }
    }
}
