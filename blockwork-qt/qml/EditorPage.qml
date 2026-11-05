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
    // Preserve metadata identity so block moves do not refresh palette controls.
    readonly property string blockDefinitionsKey: JSON.stringify(root.macro && root.macro.block_defs ? root.macro.block_defs : [])
    readonly property var macroBlockDefinitions: JSON.parse(blockDefinitionsKey)
    readonly property string macroVariablesKey: JSON.stringify(root.macro && root.macro.variables ? root.macro.variables : [])
    readonly property var macroVariables: JSON.parse(macroVariablesKey)
    readonly property string macroListsKey: JSON.stringify(root.macro && root.macro.lists ? root.macro.lists : [])
    readonly property var macroLists: JSON.parse(macroListsKey)
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

    // Blockwork's blocks: palette sections, defaults, reporters, rows and help.
    readonly property BlockCatalog catalog: BlockCatalog { page: root }
    function uuid() { return BlockRegistry.newId(); }
    function numberValue(n) { return catalog.numberValue(n); }
    function textValue(t) { return catalog.textValue(t); }
    function defaultInstruction(type) { return catalog.defaultInstruction(type); }
    readonly property var extraOperators: catalog.extraOperators
    function registerBlocks() { catalog.register(); }
    function userVariableNames() {
        const packNames = new Set((root.macro && root.macro.pack_data || [])
            .reduce((names, pack) => names.concat((pack.variables || []).map(v => v.name)), []));
        return BlockRegistry.sortedVariables(root.macroVariables.filter(name => !packNames.has(name)));
    }
    function firstVariableName() { return BlockRegistry.defaultVariableName(userVariableNames()); }
    function firstListName() { return root.macro && root.macro.lists && root.macro.lists.length ? root.macro.lists[0].name : ""; }
    function customEntry(definition) { return { kind: "custom", definition: definition, instruction: palette.customInstruction(definition) }; }
    // The palette drag controller, for tests.
    property alias paletteDrags: dragController
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
    // In-memory copy/paste buffer (not the OS clipboard): a fragment from
    // copy_instructions, the blocks plus the notes on them.
    property var clipboard: null
    function copyBlocks(strandId, path, count, then) {
        root.daemonCall("copy_instructions", { strandId: strandId, path: path, count: count }, fragment => {
            if (!fragment) return;
            root.clipboard = fragment;
            if (then) then();
        });
    }
    function totalBlocks() { return (root.macro ? root.macro.strands : []).reduce((n, st) => n + st.instructions.length, 0); }
    function nextPath(path) { const p=JSON.parse(JSON.stringify(path));p[p.length-1].index++;return p; }
    function showDetails(type,fallback) {
        const def = blockDefById(type);
        if (def) {
            const name = (def.pieces || []).map(p => p.kind === "Label" ? p.text : "(" + p.name + ")").join(" ");
            const explainer = def.read_only ? qsTr("A community block from %1.").arg(def.pack_name) : fallback;
            detailsDialog.show(name, def.identifier || def.id, def.description || explainer || qsTr("A custom block defined in this macro."));
        } else detailsDialog.show(catalog.help.title(type),type,catalog.help.explainer(type)||fallback||qsTr("A Blockwork block."));
    }
    // ---- daemon commands whose result QML needs ----
    property int callSeq: 0
    property var callbacks: ({})
    function daemonCall(command, args, done) {
        const key = "call" + (++callSeq);
        callbacks[key] = done;
        root.appBridge.callCommand(key, command, JSON.stringify(args || {}));
    }
    Component {
        id: headerPlacementPreview
        InstructionBlock { visible: false; locked: true }
    }
    function createBlock(pieces, shape, color, icon, identifier, description) {
        const definition = {id: "placement-preview", pieces: pieces, shape: shape, color: color, icon: icon};
        const preview = headerPlacementPreview.createObject(canvas, {
            instruction: {id: "placement-preview-header", type: "BlockHeader", block_id: definition.id},
            blockDefinitions: (root.macro ? root.macro.block_defs : []).concat([definition])
        });
        const position = canvas.findEmptyPosition(preview.implicitWidth, preview.implicitHeight);
        preview.destroy();
        root.invoke("create_block", {pieces: pieces, shape: shape, color: color, icon: icon,
            identifier: identifier, description: description, x: position.x, y: position.y});
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
    // { kind: "image" | "region", strandId, path, instruction }, { kind: "value", location, value, point }
    property var pickRequest: null
    property int savedVisibility: Window.Windowed
    // Gives the compositor a moment to take a window off screen.
    function afterUnmap(action) { unmapDelay.action = action; unmapDelay.restart(); }
    Timer { id: unmapDelay; interval: 300; property var action: null; onTriggered: if (action) action() }
    // Blocks whose region is in screen coordinates rather than a window's.
    readonly property var screenRegionTypes: ["FindImage", "ClickImage", "WaitForImage"]
    function startScreenPick(request) {
        const win = Window.window;
        if (pickRequest || !win) return;
        pickRequest = request;
        // Out of the way, so whatever is behind the editor can be picked.
        savedVisibility = win.visibility;
        win.hide();
        const onScreen = request.kind === "region" && root.screenRegionTypes.indexOf((request.instruction || {}).type) >= 0;
        const hint = onScreen ? qsTr("Drag out the part of the screen to search")
            : request.kind === "value" ? (request.point ? qsTr("Click the point to read") : qsTr("Drag out the part of the screen to read"))
            : { image: qsTr("Drag around the image to look for"), region: qsTr("Drag out the part of the window to search") }[request.kind];
        const regionOnly = request.kind === "value" ? !request.point : true;
        root.afterUnmap(() => screenOverlay.begin(regionOnly, hint));
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
            if (request && request.kind === "value") { root.setValueRegion(request, x, y, width, height); return; }
            // Capture once the overlay itself is gone.
            root.afterUnmap(() => root.daemonCall("screen_grab", { x: x, y: y, width: width, height: height }, (result, error) => {
                root.restoreWindow();
                if (!result) { pickError.show(error); return; }
                if (request) root.setBlockImage(request, result.image);
            }));
        }
    }
    // A screen reporter's x, y (and width, height) from a pick, in screen coordinates.
    function setValueRegion(request, x, y, width, height) {
        root.restoreWindow();
        const n = v => numberValue(Math.round(v));
        const updated = JSON.parse(JSON.stringify(request.value));
        updated.args = request.point ? [n(x), n(y)] : [n(x), n(y), n(width), n(height)];
        root.invoke("put_value", { location: request.location, value: updated });
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
    // An image block's is plain screen coordinates.
    function setBlockRegion(request, x, y, width, height) {
        root.restoreWindow();
        const ins = request.instruction || {};
        if (root.screenRegionTypes.indexOf(ins.type) >= 0) {
            const updated = JSON.parse(JSON.stringify(ins));
            updated.region = [x, y, width, height].map(Math.round).join(",");
            root.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
            return;
        }
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
        else if (action === "pickApp") root.openAppPicker(strandId, path, instruction);
        else if (action === "launchOptions") launchDialog.openFor(request);
        else if (action === "pickRegion") root.startScreenPick(Object.assign(request, { kind: "region" }));
        else if (action === "clearRegion") {
            const updated = JSON.parse(JSON.stringify(instruction || {}));
            updated.region = "";
            root.invoke("edit_instruction", { strandId: strandId, path: path, instruction: updated });
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
    // ---- palette drops: the shared controller's edits as daemon commands ----
    // Keep palette edits and give the inserted block its own id.
    function paletteInstruction(spec) {
        if (spec.kind !== "instruction" || !spec.type) return null;
        if (!spec.instruction) return defaultInstruction(spec.type);
        const instruction = JSON.parse(JSON.stringify(spec.instruction));
        instruction.id = uuid();
        return instruction;
    }
    function headerStrand(strandId) {
        const strand = (root.macro ? root.macro.strands : []).find(s => s.id === strandId);
        const first = strand ? strand.instructions[0] : null;
        return first && first.type === "BlockHeader" ? first : null;
    }
    function submitEdit(edit, reply) {
        const call = (command, args) => {
            if (edit.variableToCreate) args.variableToCreate = edit.variableToCreate;
            root.daemonCall(command, args, (result, error) => {
                if (error) root.appBridge.lastError = error;
                reply(!error, error);
            });
        };
        if (edit.kind === "insert_instruction") call("add_instruction", { strandId: edit.strandId, path: edit.path, instruction: edit.instruction, targetY: edit.targetY });
        else if (edit.kind === "add_strand") call("add_strand", { x: edit.x, y: edit.y, instruction: edit.instruction });
        else if (edit.kind === "put_value") call("put_value", { location: edit.location, value: edit.value });
        else if (edit.kind === "create_floating_value") call("create_floating_value", { x: edit.x, y: edit.y, value: edit.value, originBlockId: edit.originBlockId || null });
        else if (edit.kind === "remove_instructions") call("remove_instruction", { strandId: edit.strandId, path: edit.path, count: edit.count });
        else if (edit.kind === "remove_strand") {
            // A custom block's header row takes its whole definition with it.
            const header = headerStrand(edit.strandId);
            if (header) call("delete_block", { blockId: header.block_id });
            else call("remove_strand", { strandId: edit.strandId });
        } else if (edit.kind === "take_value") call("take_value", { location: edit.location });
        else if (edit.kind === "remove_floating_value") call("remove_floating_value", { floatingId: edit.floatingId });
        else reply(false, "Unknown edit " + edit.kind);
    }
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
        const header = path.length === 1 && path[0].index === 0 ? headerStrand(strandId) : null;
        if (header) root.invoke("delete_block", { blockId: header.block_id });
        else root.invoke("remove_instruction", { strandId: strandId, path: path });
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
            Item { anchors.fill: parent
                Item {
                    id: sidebarHost
                    // Overlay the sidebar so resizing it never moves the workspace.
                    width: root.sidebarCollapsed ? 0 : (palette.layout === "flyout" ? palette.railWidth : root.sidebarWidth)
                    anchors.top: parent.top; anchors.bottom: parent.bottom
                    clip: palette.layout !== "flyout"
                    z: 5
                    // Animate only the collapse toggle, not drag-resizing.
                    Behavior on width {
                        enabled: root.sidebarToggling
                        NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic; onRunningChanged: if (!running) root.sidebarToggling = false }
                    }
                    // Empty palette space must not pan or click the covered canvas.
                    MouseArea {
                        width: palette.width; height: parent.height
                        visible: palette.visible
                        acceptedButtons: Qt.AllButtons
                        onWheel: wheel => wheel.accepted = true
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
                    onDragStarted: (spec, sx, sy, ox, oy) => dragController.begin(spec, sx, sy, ox, oy)
                    onDragMoved: (sx, sy) => dragController.move(sx, sy)
                    onDragEnded: (sx, sy) => dragController.end(sx, sy)
                    onDragCanceled: dragController.cancel()
                    sectionsListBlocks: true
                    listInstructionTypes: root.catalog.listInstructionTypes
                    extraOperators: root.browserShown ? root.catalog.extraOperators : root.catalog.extraOperators.filter(o => root.catalog.categoryOf(o[0]) !== "browser")
                    sections: root.browserShown ? root.catalog.paletteSections : root.catalog.paletteSections.filter(s => s.category !== "browser")
                    appCategories: [{ id: "screen", name: qsTr("Screen"), icon: "monitor" }].concat(root.browserShown ? [{ id: "browser", name: qsTr("Browser"), icon: "panels-top-left" }] : [])
                    // Leaves the Browser category when switching to a macro that hides it.
                    onAppCategoriesChanged: if (category === "browser" && !root.browserShown) category = layout === "accordion" ? "input" : "all"
                    variables: root.userVariableNames()
                    lists: root.macroLists
                    blockDefinitions: root.macroBlockDefinitions
                    keyCapture: appState.key_capture
                    standaloneKey: root.sidebarKey
                    onBlockActivated: (type, instruction) => dragController.addAt({ kind: "instruction", type: type, instruction: instruction }, 120, 120)
                    onCustomBlockActivated: definition => dragController.addAt(root.customEntry(definition), 120, 120)
                    onMakeVariableRequested: variableDialog.openForCreate()
                    onSectionActionRequested: action => { if (action === "makeGlobal") globalPicker.openForCreate(null); }
                    onRenameVariableRequested: name => variableDialog.openForRename(name)
                    offerSetVariableValue: true
                    onSetVariableValueRequested: name => variableValueDialog.openFor(name)
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
                    onPieceActionRequested: (action, instruction) => { if (action === "pickApp") root.openPaletteAppPicker(instruction); }
                    onDetailsRequested:(name,identifier,explainer)=>root.blockDefById(identifier)||name===identifier?root.showDetails(identifier,explainer):detailsDialog.show(name,identifier,explainer)
                    onValueActivated: value => dragController.addAt({ kind: "value", value: value }, 160, 140)
                }
                }
                BlockCanvas {
                    id: canvas
                    // Covered space belongs to the sidebar, including for drag releases.
                    containmentMask: QtObject {
                        function contains(point: point): bool {
                            if (point.x < 0 || point.y < 0 || point.x >= canvas.width || point.y >= canvas.height) return false;
                            const scene = canvas.mapToItem(null, point.x, point.y);
                            return !dragController.inTrash(scene.x, scene.y);
                        }
                    }
                    textMenu: Component { Editing.TextEditingMenu {} }
                    anchors.fill: parent
                    strands: root.macro ? root.macro.strands : []
                    recordingTargetId: root.macro && root.macro.recording_target_strand_id ? root.macro.recording_target_strand_id : ""
                    comments: root.macro ? root.macro.comments : []
                    floatingValues: root.macro ? root.macro.floating_values : []
                    variables: root.macroVariables
                    lists: root.macroLists
                    blockDefinitions: root.macroBlockDefinitions
                    keyCapture: appState.key_capture
                    locked: root.recordingBusy
                    onStrandMoved: (strandId, x, y) => root.invoke("move_strand", { strandId: strandId, x: x, y: y })
                    onInstructionSplit: (strandId, path, x, y) => root.invoke("split_strand", { strandId: strandId, path: path, x: x, y: y })
                    onStrandsMerged: (draggedId, targetId, path, targetY) => root.invoke("merge_strand", { draggedId: draggedId, targetId: targetId, path: path, targetY: targetY })
                    onTailMerged: (strandId, path, targetId, targetPath, targetY) => root.invoke("merge_tail", { strandId: strandId, path: path, targetId: targetId, targetPath: targetPath, targetY: targetY })
                    onInstructionRemoved: (strandId, path) => root.removeInstruction(strandId, path)
                    canPaste: !!root.clipboard && root.clipboard.instructions.length > 0
                    clearIcon: "trash"
                    clearText: qsTr("Delete %n block(s)", "", root.totalBlocks())
                    onCanvasMenuAction: (action, x, y) => {
                        if (action === "paste" && root.clipboard) root.invoke("paste_fragment", { x: x, y: y, fragment: root.clipboard });
                    }
                    onBlockMenuAction: (action, strandId, path, instruction) => {
                        if (action === "edit_block") root.editBlockById(instruction.block_id);
                        else if (action === "breakpoint") root.invoke("toggle_breakpoint", { blockId: instruction.id });
                        else if (action === "run_strand") root.invoke("run_strand", { strandId: strandId });
                        else if (action === "copy") root.copyBlocks(strandId, path, 1);
                        else if (action === "copy_all") root.copyBlocks(strandId, path, null);
                        else if (action === "cut") root.copyBlocks(strandId, path, 1, () => root.removeInstruction(strandId, path));
                    }
                    onInstructionDuplicated: (strandId, path) => root.invoke("duplicate_instruction", { strandId: strandId, path: path })
                    onInstructionEdited: (strandId, path, instruction) => root.invoke("edit_instruction", { strandId: strandId, path: path, instruction: instruction })
                    onRunBranchRequested: (strandId, path, name) => root.invoke("add_instruction", { strandId: strandId, path: root.nextPath(path), instruction: { id: root.uuid(), type: "RunBranch", name: name } })
                    onValueEdited: (location, text) => root.invoke("edit_value_field", { location: location, text: text })
                    onValueTakeRequested: location => root.invoke("take_value", { location: location })
                    onValuePutRequested: (location, value) => root.invoke("put_value", { location: location, value: value })
                    onValueCreateRequested: (x, y, value, originBlockId) => root.invoke("create_floating_value", { x: x, y: y, value: value, originBlockId: originBlockId || null })
                    onValueMoveRequested: (from, to, x, y, originBlockId) => root.invoke("move_value", { from: from, to: to, x: x, y: y, originBlockId: originBlockId || null })
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
                    onClearRequested: clearInstructionsDialog.open()
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
                    id: speed; from: .1; to: 10; value: savedSpeed; Layout.fillWidth: true; implicitHeight: 30
                    Accessible.name: qsTr("Playback speed")
                    readonly property real savedSpeed: Math.max(from, Math.min(to, root.macro ? (root.macro.speed_multiplier ?? 1) : 1))
                    property bool readyForEdits: false
                    Component.onCompleted: readyForEdits = true
                    onValueChanged: {
                        if (readyForEdits && root.macro && value !== savedSpeed)
                            root.invoke("set_macro_speed_multiplier", { multiplier: value });
                    }
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
                BwTextField { id: localUrl; Layout.fillWidth: true; placeholderText: root.catalog.defaultDetector; onAccepted: useUrl.clicked() }
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
        objectName: "globalPicker"
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
        id: launchDialog; title: qsTr("Launch options"); standardButtons: Dialog.Ok | Dialog.Cancel; acceptText: qsTr("Save")
        property var request: null
        readonly property bool mac: Qt.platform.os === "osx"
        function openFor(req) {
            request = req;
            const l = (req.instruction && req.instruction.launch) || {};
            launchArgs.text = l.args || "";
            launchDir.text = l.working_dir || "";
            launchEnv.clear();
            for (const v of (l.env || [])) launchEnv.append({ name: v.name || "", value: v.value || "" });
            launchNewCopy.checked = !!l.new_instance;
            open();
            launchArgs.forceActiveFocus();
        }
        onAccepted: {
            const req = request;
            request = null;
            if (!req) return;
            const env = [];
            for (let i = 0; i < launchEnv.count; ++i) {
                const row = launchEnv.get(i);
                if (row.name.trim()) env.push({ name: row.name.trim(), value: row.value });
            }
            const updated = JSON.parse(JSON.stringify(req.instruction || {}));
            updated.launch = { args: launchArgs.text.trim(), working_dir: launchDir.text.trim(), env: env, new_instance: launchNewCopy.checked };
            root.invoke("edit_instruction", { strandId: req.strandId, path: req.path, instruction: updated });
        }
        ListModel { id: launchEnv }
        FolderDialog {
            id: launchFolder; title: qsTr("Choose a working folder")
            currentFolder: StandardPaths.writableLocation(StandardPaths.HomeLocation)
            onAccepted: {
                const text = decodeURIComponent(selectedFolder.toString());
                launchDir.text = Qt.platform.os === "windows" ? text.replace(/^file:\/\/\//, "") : text.replace(/^file:\/\//, "");
            }
        }
        Column { width: 440; spacing: 8
            Text { text: qsTr("Arguments"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
            BwTextField { id: launchArgs; width: parent.width; placeholderText: "--incognito" }
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
                text: qsTr("Added after the app's own command, the way you'd type them in a terminal. Put quotes around an argument with spaces.") }
            Item { width: 1; height: 4 }
            Text { text: qsTr("Working folder"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
            RowLayout { width: parent.width; spacing: 8
                BwTextField { id: launchDir; Layout.fillWidth: true; placeholderText: qsTr("Leave empty to use the default") }
                BwButton { text: qsTr("Browse..."); iconName: "folder-open"; onClicked: launchFolder.open() }
            }
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
                text: launchDialog.mac ? qsTr("macOS always starts apps in their own folder, so this is only used on Windows and Linux.")
                                       : qsTr("The folder the app starts in. ~ means your home folder.") }
            Item { width: 1; height: 4 }
            Text { text: qsTr("Environment variables"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
            Text { visible: launchEnv.count === 0; width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
                text: qsTr("None. These are set for this app only.") }
            Repeater {
                model: launchEnv
                delegate: RowLayout {
                    required property int index
                    required property string name
                    required property string value
                    width: 440; spacing: 6
                    BwTextField { Layout.preferredWidth: 150; text: name; placeholderText: qsTr("NAME"); onTextEdited: launchEnv.setProperty(index, "name", text) }
                    Text { text: "="; color: Theme.textDim }
                    BwTextField { Layout.fillWidth: true; text: value; placeholderText: qsTr("value"); onTextEdited: launchEnv.setProperty(index, "value", text) }
                    BwButton { iconName: "x"; implicitWidth: 32; onClicked: launchEnv.remove(index) }
                }
            }
            BwButton { text: qsTr("Add variable"); iconName: "plus"; onClicked: launchEnv.append({ name: "", value: "" }) }
            Item { width: 1; height: 4 }
            BwCheckBox { id: launchNewCopy; text: qsTr("Open a new copy even if the app is already running") }
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
                text: qsTr("Only changes anything on macOS. On Windows and Linux the app decides whether a second launch opens a new window.") }
        }
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
                        readonly property int savedSeconds: Math.max(from, Math.min(to, appState.recording_countdown_secs ?? 3))
                        property bool readyForEdits: false
                        Component.onCompleted: readyForEdits = true
                        value: savedSeconds
                        onValueChanged: {
                            if (readyForEdits && value !== savedSeconds)
                                root.invoke("set_recording_countdown", { seconds: Math.round(value) });
                        }
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
        objectName: "variableDialog"
        property string renameTarget: ""
        property var selectCreated: null
        property string errorText: ""
        title: renameTarget.length ? qsTr("Rename Variable") : qsTr("Make a Variable")
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openForCreate(select) { renameTarget = ""; selectCreated = select || null; errorText = ""; variableName.text = ""; open(); variableName.forceActiveFocus(); }
        function openForRename(name) { selectCreated = null; errorText = ""; renameTarget = name; variableName.text = name; open(); variableName.forceActiveFocus(); variableName.selectAll(); }
        Column { width: 340; spacing: 8
            Text { text: qsTr("Variable name"); color: Theme.text }
            BwTextField { id: variableName; objectName: "variableName"; width: parent.width; placeholderText: qsTr("score"); onAccepted: variableDialog.accept() }
            Text { width: parent.width; visible: text.length > 0; text: variableDialog.errorText; color: Theme.text; wrapMode: Text.WordWrap }
        }
        onRejected: selectCreated = null
        onAccepted: {
            const name = variableName.text.trim();
            if (!name.length) {
                errorText = qsTr("Enter a variable name.");
                Qt.callLater(() => { open(); variableName.forceActiveFocus(); });
                return;
            }
            const select = selectCreated;
            const macroId = root.macro ? root.macro.id : "";
            const command = renameTarget.length ? "rename_variable" : "create_variable";
            const args = renameTarget.length ? { oldName: renameTarget, newName: name } : { name: name };
            if (renameTarget === name) { selectCreated = null; return; }
            root.daemonCall(command, args, (result, error) => {
                if (!root.macro || root.macro.id !== macroId) return;
                if (error) {
                    errorText = error;
                    open(); variableName.forceActiveFocus();
                    return;
                }
                selectCreated = null;
                renameTarget = ""; variableName.clear();
                if (select) select(name);
            });
        }
    }
    AppDialog {
        id: variableValueDialog
        objectName: "variableValueDialog"
        property string target: ""
        property string errorText: ""
        title: qsTr("Set Value")
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openFor(name) {
            target = name; errorText = ""; variableValue.text = ""; open(); variableValue.forceActiveFocus();
            // Starts from what the variable holds now.
            root.daemonCall("preview_value", { value: { kind: "Var", name: name } }, (result, error) => {
                if (error || target !== name || variableValue.text.length) return;
                variableValue.text = String(result); variableValue.selectAll();
            });
        }
        Column { width: 340; spacing: 8
            Text { width: parent.width; text: qsTr("Value of %1").arg(variableValueDialog.target); color: Theme.text; elide: Text.ElideRight }
            BwTextField { id: variableValue; objectName: "variableValue"; width: parent.width; placeholderText: qsTr("0"); onAccepted: variableValueDialog.accept() }
            Text { width: parent.width; visible: text.length > 0; text: variableValueDialog.errorText; color: Theme.text; wrapMode: Text.WordWrap }
        }
        onAccepted: {
            const name = target, macroId = root.macro ? root.macro.id : "";
            root.daemonCall("set_variable_value", { name: name, value: variableValue.text }, (result, error) => {
                if (!root.macro || root.macro.id !== macroId || !error) return;
                errorText = error;
                open(); variableValue.forceActiveFocus();
            });
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
        objectName: "listDialog"
        property string renameTarget: ""
        property var selectCreated: null
        property string errorText: ""
        title: renameTarget.length ? qsTr("Rename List") : qsTr("Make a List")
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openForCreate(select) { renameTarget = ""; selectCreated = select || null; errorText = ""; listName.text = ""; open(); listName.forceActiveFocus(); }
        function openForRename(name) { selectCreated = null; errorText = ""; renameTarget = name; listName.text = name; open(); listName.forceActiveFocus(); listName.selectAll(); }
        Column { width: 340; spacing: 8
            Text { text: qsTr("List name"); color: Theme.text }
            BwTextField { id: listName; objectName: "listName"; width: parent.width; placeholderText: qsTr("items"); onAccepted: listDialog.accept() }
            Text { width: parent.width; visible: text.length > 0; text: listDialog.errorText; color: Theme.text; wrapMode: Text.WordWrap }
        }
        onRejected: selectCreated = null
        onAccepted: {
            const name = listName.text.trim();
            if (!name.length) {
                errorText = qsTr("Enter a list name.");
                Qt.callLater(() => { open(); listName.forceActiveFocus(); });
                return;
            }
            const select = selectCreated;
            const macroId = root.macro ? root.macro.id : "";
            const command = renameTarget.length ? "rename_list" : "create_list";
            const args = renameTarget.length ? { oldName: renameTarget, newName: name } : { name: name };
            if (renameTarget === name) { selectCreated = null; return; }
            root.daemonCall(command, args, (result, error) => {
                if (!root.macro || root.macro.id !== macroId) return;
                if (error) {
                    errorText = error;
                    open(); listName.forceActiveFocus();
                    return;
                }
                selectCreated = null;
                renameTarget = ""; listName.clear();
                if (select) select(name);
            });
        }
    }
    AppDialog {
        id: clearInstructionsDialog
        objectName: "clearInstructionsDialog"
        title: qsTr("Delete all blocks?")
        width: 400
        standardButtons: Dialog.NoButton
        property string macroId: ""
        onOpened: macroId = root.macro ? root.macro.id : ""
        Text {
            width: parent.width
            text: qsTr("Delete all blocks from this macro? You can undo this action.")
            color: Theme.text
            wrapMode: Text.WordWrap
        }
        footer: DialogButtonBox {
            alignment: Qt.AlignRight
            spacing: 8
            padding: 18; topPadding: 8
            background: null
            BwButton { text: qsTr("Cancel"); onClicked: clearInstructionsDialog.reject() }
            BwButton { text: qsTr("Delete"); danger: true; onClicked: clearInstructionsDialog.accept() }
        }
        onAccepted: {
            if (root.macro && root.macro.id === macroId) root.invoke("clear_instructions");
        }
    }
    AppDialog {
        id: deleteListDialog
        objectName: "deleteListDialog"
        title: qsTr("Delete list?"); standardButtons: Dialog.Yes | Dialog.Cancel
        property string listName: ""
        Text { text: qsTr("Delete \"%1\" and its saved items?").arg(deleteListDialog.listName); color: Theme.text }
        onAccepted: root.invoke("delete_list", { name: deleteListDialog.listName })
    }
    AppDialog {
        id:detailsDialog;standardButtons:Dialog.NoButton;width:Math.min(430, parent.width - 32);padding:0;topPadding:0;bottomPadding:0;showClose:false
        property string detailName:"";property string detailIdentifier:"";property string detailExplainer:""
        function show(name,identifier,explainer){detailName=name;detailIdentifier=identifier;detailExplainer=explainer;open()}
        contentItem:Column{spacing:12;padding:18
            Row {
                width: parent.width - 36
                spacing: 9
                LucideIcon { name: "info"; width: 20; height: 20; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
                Text {
                    width: parent.width - 29
                    text: detailsDialog.detailName
                    color: Theme.text; font.pixelSize: 17; font.weight: Font.Bold
                    wrapMode: Text.Wrap
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
            Rectangle {
                width: parent.width - 36
                height: Math.max(30, identifierText.implicitHeight + 12)
                radius: 5; color: Theme.field; border.color: Theme.border
                Text {
                    id: identifierText
                    anchors.centerIn: parent
                    width: parent.width - 16
                    text: detailsDialog.detailIdentifier
                    color: Theme.accent; font.family: "monospace"; font.pixelSize: 12
                    wrapMode: Text.WrapAnywhere
                    horizontalAlignment: Text.AlignHCenter
                }
            }
            Text{width:parent.width-36;text:detailsDialog.detailExplainer;color:Theme.textDim;font.pixelSize:13;wrapMode:Text.Wrap}
            Row{anchors.right:parent.right;anchors.rightMargin:18;BwButton{text:qsTr("Close");onClicked:detailsDialog.close()}}
        }
    }
    MakeBlockDialog {
        id:blockDialog
        objectName: "blockDialog"
        onCreateRequested:(pieces,shape,color,icon,identifier,description)=>root.createBlock(pieces,shape,color,icon,identifier,description)
        onEditRequested:(blockId,pieces,shape,color,icon,identifier,description)=>root.invoke("edit_block",{blockId:blockId,pieces:pieces,shape:shape,color:color,icon:icon,identifier:identifier,description:description})
    }

    // Browser blocks stay out of the palette until a browser has paired once,
    // unless the open macro already uses them.
    readonly property bool browserShown: !!(appState.browser && appState.browser.unlocked) || !!appState.macro_uses_browser
    function variableChoices() {
        return BlockRegistry.variableChoices(userVariableNames())
            .concat([{ label: qsTr("Make a Variable…"), action: select => variableDialog.openForCreate(select) }]);
    }
    function globalChoices() {
        return (root.appState.globals || []).map(g => ({ value: g.name, label: g.name }))
            .concat([{ label: qsTr("Make a Global Variable…"), action: (select, location) => globalPicker.openForCreate(location
                ? name => root.invoke("put_value", { location: location, value: root.textValue(name) }) : select) }]);
    }
    // What the open macro may do without asking; null when it's trusted.
    readonly property var permissions: appState.permissions === undefined ? null : appState.permissions
    readonly property var guardedAbilities: [
        { kind: "commands", label: qsTr("Run commands and open or close apps") }, { kind: "network", label: qsTr("Go online") },
        { kind: "files_write", label: qsTr("Write to files") }, { kind: "secrets", label: qsTr("Read your saved secrets") },
        { kind: "screen", label: qsTr("Read the screen") }, { kind: "files_read", label: qsTr("Read files and folders") },
        { kind: "input", label: qsTr("Press keys and move the mouse") }, { kind: "clipboard", label: qsTr("Read the clipboard") },
        { kind: "macros", label: qsTr("Run or stop your other macros") }, { kind: "globals", label: qsTr("Read or change global variables") },
        { kind: "browser", label: qsTr("Control your web browser") }]
    // The variable dropdowns read macro state through a function, so nudge them.
    readonly property string variablesKey: JSON.stringify([root.macro ? root.macro.variables : [], root.macro ? root.macro.signals : [], appState.secret_names || [], appState.globals || []])
    Binding { target: BlockRegistry; property: "optionKey"; value: root.variablesKey }

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

    // Right-click entries the canvas adds to every block. Run strand only
    // shows on a strand's top block, and not on a custom block's definition.
    function blockMenu() {
        return [{ id: "run_strand", text: qsTr("Run strand"), icon: "play",
                  when: (instruction, strandId, path) => path.length === 1 && path[0].index === 0 && !!instruction && instruction.type !== "BlockHeader" },
                { id: "breakpoint", text: qsTr("Toggle breakpoint"), icon: "circle-dot" }];
    }
    // Click-to-preview on operators: the daemon evaluates, the chip shows the answer.
    // Block labels are read once, so register them again in a new language.
    readonly property string uiLanguage: appState.language || ""
    onUiLanguageChanged: Qt.callLater(() => { BlockRegistry.resetOperators(); registerBlocks(); BlockRegistry.blockMenu = root.blockMenu(); })
    property var createListFromDropdown: (select, location) => listDialog.openForCreate(location
        ? name => root.invoke("edit_value_field", { location: location, text: name }) : select)
    property var replaceComponentValue: (location, value) => root.invoke("put_value", { location: location, value: value })
    Component.onDestruction: {
        if (BlockRegistry.listCreationHandler === root.createListFromDropdown) BlockRegistry.listCreationHandler = null;
        if (BlockRegistry.valueReplacementHandler === root.replaceComponentValue) BlockRegistry.valueReplacementHandler = null;
    }
    Component.onCompleted: {
        BlockRegistry.listCreationHandler = root.createListFromDropdown;
        BlockRegistry.valueReplacementHandler = root.replaceComponentValue;
        registerBlocks();
        BlockRegistry.blockMenu = root.blockMenu();
        BlockRegistry.previewHandler = (key, value) => root.appBridge.previewValue(key, JSON.stringify(value));
        // Name slots share the same picker in instructions and reporters.
        BlockRegistry.pickHandler = (action, location, value, anchor) => {
            if (action === "pickUsb") usbDialog.openFor({ kind: "value", location: location, value: value });
            else if (action === "pickGlobal") globalPicker.openFor(name => root.invoke("put_value", { location: location, value: root.textValue(name) }), value && value.kind === "Text" ? value.value : "");
            else if (action === "pickSignal") root.pickSignal(location, value);
            else if (action === "pickTime") timePicker.openFor(location, value, anchor);
            else if (action === "pickDateTime") dateTimePicker.openFor(location, value, anchor);
            else if (action === "pickSchedule") scheduleDialog.openFor(location, value);
            else if (action === "pickScreenRegion" || action === "pickScreenPoint")
                root.startScreenPick({ kind: "value", location: location, value: value, point: action === "pickScreenPoint" });
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

    PaletteDragController {
        id: dragController
        anchors.fill: parent
        canvas: canvas
        variables: root.userVariableNames()
        lists: root.macroLists
        blockDefinitions: root.macroBlockDefinitions
        // Counting down or recording: nothing lands on the canvas.
        locked: root.recordingBusy
        documentId: root.macro ? root.macro.id : ""
        document: root.macro ? [root.macro.strands, root.macro.floating_values] : null
        trashContains: (sx, sy) => !root.sidebarCollapsed && palette.contains(palette.mapFromItem(null, sx, sy))
        instructionFor: spec => root.paletteInstruction(spec)
        submit: (edit, reply) => root.submitEdit(edit, reply)
    }
}
