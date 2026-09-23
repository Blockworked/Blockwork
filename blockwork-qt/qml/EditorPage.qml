import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke
    readonly property var macro: appState.current_macro
    readonly property bool recording: appState.recording_phase && appState.recording_phase.phase === "Active"
    property string sidebarKey: "a"
    property real sidebarWidth: 348
    readonly property real minSidebarWidth: 180
    readonly property real maxSidebarWidth: 600
    readonly property var instructionTypes: ["WhenRan", "WhenBatteryDischargedTo", "WhenBatteryChargedTo", "WhenTime", "WhenPowerPluggedIn", "WhenPowerUnplugged", "WhenClipboardChanged", "Wait", "Text", "Key", "Button", "MoveMouse", "Scroll", "Command", "OpenApp", "CloseApp", "SetVariable", "ChangeVariable", "SetClipboard", "Return", "If", "IfElse", "Repeat", "Forever", "While", "EscapeLoop", "ContinueLoop"]
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
        else if (type === "Key") { i.key = root.sidebarKey; i.direction = "Click"; }
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
        const x={WhenRan:"Runs this strand when the macro starts.",Key:"Presses, releases, or clicks a keyboard key.",Wait:"Pauses this strand for the given milliseconds.",Text:"Types the given text.",SetVariable:"Sets a variable to a value.",ChangeVariable:"Adds a number to a variable.",SetClipboard:"Replaces the clipboard text.",If:"Runs its body when the condition is true.",IfElse:"Chooses one of two branches.",Repeat:"Runs its body a fixed number of times.",Forever:"Repeats until stopped.",While:"Repeats while its condition is true."};
        return x[type]||"A Blockwork instruction block.";
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
    }
    function endPaletteDrag(sx, sy) {
        const drag = paletteDrag;
        cancelPaletteDrag();
        if (!drag || recording) return;
        const at = canvas.workspacePoint(sx, sy);
        if (!at) return;
        const x = Math.round(at.x - drag.offsetX / canvas.zoom), y = Math.round(at.y - drag.offsetY / canvas.zoom);
        const spec = drag.spec;
        if (spec.kind === "value") root.invoke("create_floating_value", { x: x, y: y, value: spec.value, originBlockId: null });
        else if (spec.kind === "custom") root.addCustomBlock(spec.definition, x, y);
        else root.addBlock(spec.type, x, y);
    }
    function cancelPaletteDrag() { paletteDrag = null; ghost.visible = false; ghost.spec = null; }
    // A canvas block dropped on the sidebar deletes it and everything below it in its stack.
    function trashDraggedBlocks(strandId, path, tailCount, sx, sy) {
        if (!palette.contains(palette.mapFromItem(null, sx, sy))) return;
        if (path.length === 1 && path[0].index === 0) { root.invoke("remove_strand", { strandId: strandId }); return; }
        for (let i = 0; i < tailCount; ++i) root.invoke("remove_instruction", { strandId: strandId, path: path });
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
            Layout.fillWidth: true; Layout.preferredHeight: 104; color: "#292a2d"; border.color: "#38393d"
            RowLayout {
                anchors.fill: parent; anchors.margins: 14; spacing: 10
                ColumnLayout {
                    Layout.preferredWidth: 280; spacing: 5
                    Text { text: "Select macro"; color: "#a1a2a7"; font.pixelSize: 12 }
                    BwComboBox {
                        id: macroPicker
                        Layout.fillWidth: true; model: appState.macro_names || []
                        currentIndex: appState.macro_selected === null || appState.macro_selected === undefined ? -1 : appState.macro_selected
                        onActivated: index => root.invoke("select_macro", { index: index })
                    }
                }
                BwButton { iconName: "sliders"; text: ""; enabled: !!root.macro; onClicked: macroSettings.open() }
                Item { Layout.fillWidth: true }
                BwButton { iconName: "plus"; text: "New macro"; onClicked: root.invoke("new_macro") }
                BwButton { iconName: "trash"; text: "Delete"; danger: true; enabled: !!root.macro; onClicked: removeDialog.open() }
                BwButton { iconName: "settings"; text: "Settings"; onClicked: root.invoke("open_settings") }
            }
        }
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 66; color: "#292a2d"; border.color: "#38393d"
            RowLayout { anchors.fill: parent; anchors.margins: 14
                BwButton { primary: true; iconName: appState.loop_mode_enabled ? "repeat" : "play"; text: appState.loop_mode_enabled ? "Start loop" : "Run macro"; enabled: appState.macro_selected !== null; onClicked: root.invoke("run_macro") }
                BwSwitch { Accessible.name: "Loop mode"; checked: appState.loop_mode_enabled; onToggled: checked => root.invoke("toggle_loop_mode", { enabled: checked }) }
                Item { Layout.fillWidth: true }
                BwButton { danger: true; iconName: root.recording ? "square" : "circle"; iconFilled: !root.recording; text: root.recordingLabel(); enabled: appState.macro_selected !== null; onClicked: root.invoke(root.recording ? "stop_recording" : "start_recording") }
                BwButton { iconName: "sliders"; text: ""; onClicked: recordingSettings.open() }
            }
        }
        Rectangle {
            visible: !!root.macro
            Layout.fillWidth: true; Layout.preferredHeight: 54; color: "#292a2d"; border.color: "#38393d"
            RowLayout { anchors.fill: parent; anchors.margins: 12; spacing: 10
                Text { text: "Title"; color: "#a1a2a7"; font.pixelSize: 13 }
                BwTextField { Layout.fillWidth: true; text: root.macro ? root.macro.name : ""; onEditingFinished: root.invoke("set_title", { title: text }) }
                Text { text: "Speed"; color: "#a1a2a7"; font.pixelSize: 13 }
                Slider {
                    id:speed;from:.1;to:10;value:root.macro?root.macro.speed_multiplier:1;Layout.preferredWidth:160;implicitHeight:30
                    onMoved:root.invoke("set_macro_speed_multiplier",{multiplier:value})
                    HoverHandler{cursorShape:Qt.PointingHandCursor}
                    background:Rectangle{x:speed.leftPadding;y:speed.topPadding+speed.availableHeight/2-height/2;width:speed.availableWidth;height:5;radius:3;color:"#45474d";Rectangle{width:speed.visualPosition*parent.width;height:parent.height;radius:3;color:Theme.accent}}
                    handle:Rectangle{x:speed.leftPadding+speed.visualPosition*(speed.availableWidth-width);y:speed.topPadding+speed.availableHeight/2-height/2;width:16;height:16;radius:8;color:"white";border.width:2;border.color:Theme.accent}
                }
                BwTextField { text: Number(speed.value).toFixed(2); Layout.preferredWidth: 62; horizontalAlignment: Text.AlignRight; onEditingFinished: root.invoke("set_macro_speed_multiplier", { multiplier: Number(text) }) }
                Text { text: "x"; color: "#a1a2a7" }
            }
        }
        Item {
            visible: !!root.macro
            Layout.fillWidth: true; Layout.fillHeight: true
            RowLayout { anchors.fill: parent; spacing: 0
                PalettePanel {
                    id: palette
                    trashArmed: canvas.dragging && palette.contains(palette.mapFromItem(null, canvas.dragSceneX, canvas.dragSceneY))
                    Layout.preferredWidth: root.sidebarWidth; Layout.fillHeight: true
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
                    onMakeVariableRequested: variableDialog.open()
                    onMakeListRequested: listDialog.openForCreate()
                    onListEditorStateRequested: (name, visible, x, y) => root.invoke("set_list_editor_state", { name: name, visible: visible, x: x, y: y })
                    onRenameListRequested: name => listDialog.openForRename(name)
                    onDeleteListRequested: name => { deleteListDialog.listName = name; deleteListDialog.open(); }
                    onMakeBlockRequested: blockDialog.open()
                    onStandaloneKeyCaptureRequested: root.invoke("start_standalone_key_capture")
                    onDetailsRequested:(name,identifier,explainer)=>detailsDialog.show(name,identifier,explainer)
                    onValueActivated: value => root.invoke("create_floating_value", { x: 160, y: 140, value: value, originBlockId: null })
                }
                BlockCanvas {
                    id: canvas
                    Layout.fillWidth: true; Layout.fillHeight: true
                    strands: root.macro ? root.macro.strands : []
                    comments: root.macro ? root.macro.comments : []
                    floatingValues: root.macro ? root.macro.floating_values : []
                    variables: root.macro ? root.macro.variables : []
                    lists: root.macro && root.macro.lists ? root.macro.lists : []
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                    keyCapture: appState.key_capture
                    locked: root.recording
                    onStrandMoved: (strandId, x, y) => root.invoke("move_strand", { strandId: strandId, x: x, y: y })
                    onInstructionSplit: (strandId, path, x, y) => root.invoke("split_strand", { strandId: strandId, path: path, x: x, y: y })
                    onBlockDragOutside: (strandId, path, tailCount, sx, sy) => root.trashDraggedBlocks(strandId, path, tailCount, sx, sy)
                    onInstructionRemoved: (strandId, path) => root.invoke("remove_instruction", { strandId: strandId, path: path })
                    onInstructionDuplicated: (strandId, path, instruction) => root.invoke("add_instruction", { strandId: strandId, path: root.nextPath(path), instruction: root.refreshIds(instruction) })
                    onInstructionEdited: (strandId, path, instruction) => root.invoke("edit_instruction", { strandId: strandId, path: path, instruction: instruction })
                    onRunBranchRequested: (strandId, path, name) => root.invoke("add_instruction", { strandId: strandId, path: root.nextPath(path), instruction: { id: root.uuid(), type: "RunBranch", name: name } })
                    onValueEdited: (location, text) => root.invoke("edit_value_field", { location: location, text: text })
                    onCommentForInstructionRequested: instruction => root.invoke("create_attached_comment", { instructionId: instruction.id, dx: 48, dy: 18, text: "" })
                    onRecordingTargetRequested: strandId => root.invoke("set_recording_target", { strandId: strandId })
                    onKeyCaptureRequested:(strandId,path)=>root.invoke("start_key_capture",{strandId:strandId,path:path})
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
        Rectangle {
            visible: !!root.macro
            Layout.fillWidth: true; Layout.preferredHeight: 58; color: "#292a2d"; border.color: "#38393d"
            Row { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; anchors.leftMargin: 14; spacing: 8
                BwButton { iconName: "undo"; text: "Undo"; enabled: appState.can_undo; onClicked: root.invoke("undo") }
                BwButton { iconName: "redo"; text: "Redo"; enabled: appState.can_redo; onClicked: root.invoke("redo") }
                BwButton { iconName: "save"; text: "Save macro"; primary: true; onClicked: root.invoke("save_macro") }
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
        id: variableDialog; title: "Make a Variable"; standardButtons: Dialog.Ok | Dialog.Cancel
        Column { width: 340; spacing: 8
            Text { text: "Variable name"; color: "#e7e7e8" }
            BwTextField { id: variableName; width: parent.width; placeholderText: "score" }
        }
        onAccepted: {
            const name = variableName.text.trim();
            if (name.length) root.invoke("create_variable", { name: name });
            variableName.clear();
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
    MakeBlockDialog { id:blockDialog; onCreateRequested:(pieces,shape,color)=>root.invoke("create_block",{pieces:pieces,shape:shape,color:color}) }

    // Follows the pointer while a palette entry is dragged.
    Item {
        id: ghostLayer
        anchors.fill: parent; z: 500; enabled: false
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
                }
            }
        }
    }
}
