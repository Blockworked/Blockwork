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
    readonly property var macro: appState.current_macro
    readonly property bool recording: appState.recording_phase && appState.recording_phase.phase === "Active"
    property string sidebarKey: "a"
    property real sidebarWidth: 348
    property bool sidebarCollapsed: false
    property bool sidebarToggling: false
    readonly property real minSidebarWidth: 180
    readonly property real maxSidebarWidth: 600
    readonly property var instructionTypes: ["WhenRan", "WhenBatteryDischargedTo", "WhenBatteryChargedTo", "WhenTime", "WhenPowerPluggedIn", "WhenPowerUnplugged", "WhenClipboardChanged", "WhenKey", "Wait", "Text", "Key", "Button", "MoveMouse", "Scroll", "Command", "OpenApp", "CloseApp", "SetVariable", "ChangeVariable", "SetClipboard", "Return", "If", "IfElse", "Repeat", "Forever", "While", "EscapeLoop", "ContinueLoop"]
    readonly property var listInstructionTypes: ["AddToList", "DeleteOfList", "DeleteAllOfList", "ShiftList", "InsertIntoList", "ReplaceItemOfList", "ReverseList"]

    function uuid() { return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, c => { const r = Math.random() * 16 | 0; return (c === "x" ? r : (r & 3 | 8)).toString(16); }); }
    function numberValue(n) { return { kind: "Number", value: n }; }
    function textValue(t) { return { kind: "Text", value: t }; }
    function defaultInstruction(type) {
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
    function nextPath(path) { const p=JSON.parse(JSON.stringify(path));p[p.length-1].index++;return p; }
    function instructionExplainer(type) {
        const x={WhenRan:"Runs this strand when the macro starts.",Key:"Presses, releases, or clicks a key or key combo.",WhenKey:"Runs when a key or key combo is pressed or released.",Wait:"Pauses this strand for the given milliseconds.",Text:"Types the given text.",SetVariable:"Sets a variable to a value.",ChangeVariable:"Adds a number to a variable.",SetClipboard:"Replaces the clipboard text.",If:"Runs its body when the condition is true.",IfElse:"Chooses one of two branches.",Repeat:"Runs its body a fixed number of times.",Forever:"Repeats until stopped.",While:"Repeats while its condition is true."};
        return x[type]||"A Blockwork instruction block.";
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
            if (spec.kind === "custom") {
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
        if (spec.kind === "custom") root.addCustomBlock(spec.definition, x, y);
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
        if (path.length === 1 && path[0].index === 0) { root.invoke("remove_strand", { strandId: strandId }); return; }
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
                BwComboBox {
                    id: macroPicker
                    Layout.preferredWidth: Math.max(90, Math.min(200, toolbar.width * 0.17)); Layout.preferredHeight: 34; model: appState.macro_names || []
                    currentIndex: appState.macro_selected === null || appState.macro_selected === undefined ? -1 : appState.macro_selected
                    onActivated: index => root.invoke("select_macro", { index: index })
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
                        implicitWidth: 190
                        BwMenuItem { iconName: "sliders"; text: "Macro settings"; enabled: !!root.macro; onTriggered: macroSettings.open() }
                        BwMenuItem { iconName: "plus"; text: "New macro"; onTriggered: root.invoke("new_macro") }
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
                    onBlockMenuAction: (action, strandId, path, instruction) => {
                        if (action === "edit_block") root.editBlockById(instruction.block_id);
                    }
                    onInstructionDuplicated: (strandId, path, instruction) => root.invoke("add_instruction", { strandId: strandId, path: root.nextPath(path), instruction: root.refreshIds(instruction) })
                    onInstructionEdited: (strandId, path, instruction) => root.invoke("edit_instruction", { strandId: strandId, path: path, instruction: instruction })
                    onRunBranchRequested: (strandId, path, name) => root.invoke("add_instruction", { strandId: strandId, path: root.nextPath(path), instruction: { id: root.uuid(), type: "RunBranch", name: name } })
                    onValueEdited: (location, text) => root.invoke("edit_value_field", { location: location, text: text })
                    onValueTakeRequested: location => root.invoke("take_value", { location: location })
                    onValuePutRequested: (location, value) => root.invoke("put_value", { location: location, value: value })
                    onValueCreateRequested: (x, y, value) => root.invoke("create_floating_value", { x: x, y: y, value: value, originBlockId: null })
                    onValueDragOutside: (location, value, sx, sy) => root.trashDraggedValue(location, value, sx, sy)
                    onCommentForInstructionRequested: instruction => root.invoke("create_attached_comment", { instructionId: instruction.id, dx: 48, dy: 18, text: "" })
                    onRecordingTargetRequested: strandId => root.invoke("set_recording_target", { strandId: strandId })
                    onKeyCaptureRequested:(strandId,path)=>root.invoke("start_key_capture",{strandId:strandId,path:path})
                    onAppPickerRequested:(strandId,path,instruction)=>root.openAppPicker(strandId,path,instruction)
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
        id: removeDialog; title: "Delete macro?"; standardButtons: Dialog.Yes | Dialog.Cancel
        Text { text: "This removes the selected macro from Blockwork."; color: "#e7e7e8" }
        onAccepted: root.invoke("remove_macro")
    }
    BwDialog {
        id: macroSettings; title: "Macro settings"; standardButtons: Dialog.Close
        Column { spacing: 14; width: 360
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
        onCreateRequested:(pieces,shape,color)=>root.invoke("create_block",{pieces:pieces,shape:shape,color:color})
        onEditRequested:(blockId,pieces,shape,color)=>root.invoke("edit_block",{blockId:blockId,pieces:pieces,shape:shape,color:color})
    }

    // Click-to-preview on operators: the daemon evaluates, the chip shows the answer.
    Component.onCompleted: BlockRegistry.previewHandler = (key, value) => root.appBridge.previewValue(key, JSON.stringify(value))
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
