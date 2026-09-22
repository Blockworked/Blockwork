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
    readonly property var instructionTypes: ["WhenRan", "WhenBatteryDischargedTo", "WhenBatteryChargedTo", "WhenTime", "WhenPowerPluggedIn", "WhenPowerUnplugged", "Wait", "Text", "Key", "Button", "MoveMouse", "Scroll", "Command", "OpenApp", "CloseApp", "If", "IfElse", "Repeat", "Forever", "While", "EscapeLoop", "ContinueLoop"]

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
        else if (type === "Key") { i.key = "a"; i.direction = "Click"; }
        else if (type === "Button") { i.button = "Left"; i.direction = "Click"; }
        else if (type === "MoveMouse") { i.x = numberValue(0); i.y = numberValue(0); i.coordinate = "Relative"; }
        else if (type === "Scroll") { i.amount = numberValue(4); i.axis = "Vertical"; }
        else if (type === "Command") i.command = "";
        else if (type === "OpenApp" || type === "CloseApp") { i.command = ""; i.name = ""; i.icon = null; }
        else if (type === "If") { i.condition = { kind: "Bool" }; i.body = []; }
        else if (type === "IfElse") { i.condition = { kind: "Bool" }; i.then_body = []; i.else_body = []; }
        else if (type === "Repeat") { i.count = numberValue(10); i.body = []; }
        else if (type === "Forever") i.body = [];
        else if (type === "While") { i.condition = { kind: "Bool" }; i.body = []; }
        return i;
    }
    function addBlock(type, x, y) { root.invoke("add_strand", { x: x, y: y, instruction: defaultInstruction(type) }); }
    function addCustomBlock(definition, x, y) {
        const args = (definition.pieces || []).filter(piece => piece.kind === "Input")
            .map(piece => piece.value_type === "Bool" ? { kind: "Bool" } : numberValue(0));
        root.invoke("add_strand", { x: x, y: y, instruction: { id: uuid(), type: "CallBlock", block_id: definition.id, args: args } });
    }
    function recordingLabel() {
        const phase = appState.recording_phase || { phase: "Idle" };
        if (phase.phase === "Countdown") return "Recording in " + phase.countdown + "s…";
        if (phase.phase === "Active") return "Stop recording";
        return "Record";
    }

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 104; color: "#292a2d"; border.color: "#38393d"
            RowLayout {
                anchors.fill: parent; anchors.margins: 14; spacing: 10
                ColumnLayout {
                    Layout.preferredWidth: 300; spacing: 5
                    Text { text: "Select macro"; color: "#a1a2a7"; font.pixelSize: 12 }
                    ComboBox {
                        id: macroPicker
                        Layout.fillWidth: true; model: appState.macro_names || []
                        currentIndex: appState.macro_selected === null || appState.macro_selected === undefined ? -1 : appState.macro_selected
                        onActivated: index => root.invoke("select_macro", { index: index })
                    }
                }
                BwButton { text: "⌘"; enabled: !!root.macro; onClicked: macroSettings.open() }
                Item { Layout.fillWidth: true }
                BwButton { glyph: "+"; text: "New macro"; onClicked: root.invoke("new_macro") }
                BwButton { glyph: "⌫"; text: "Delete"; danger: true; enabled: !!root.macro; onClicked: removeDialog.open() }
                BwButton { glyph: "⚙"; text: "Settings"; onClicked: root.invoke("open_settings") }
            }
        }
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 66; color: "#292a2d"; border.color: "#38393d"
            RowLayout { anchors.fill: parent; anchors.margins: 14
                BwButton { primary: true; glyph: appState.loop_mode_enabled ? "⟳" : "▷"; text: appState.loop_mode_enabled ? "Start loop" : "Run macro"; enabled: appState.macro_selected !== null; onClicked: root.invoke("run_macro") }
                BwSwitch { Accessible.name: "Loop mode"; checked: appState.loop_mode_enabled; onToggled: checked => root.invoke("toggle_loop_mode", { enabled: checked }) }
                Item { Layout.fillWidth: true }
                BwButton { danger: true; glyph: root.recording ? "■" : "●"; text: root.recordingLabel(); enabled: appState.macro_selected !== null; onClicked: root.invoke(root.recording ? "stop_recording" : "start_recording") }
                BwButton { text: "⌘"; onClicked: recordingSettings.open() }
            }
        }
        Rectangle {
            visible: !!root.macro
            Layout.fillWidth: true; Layout.preferredHeight: 54; color: "#292a2d"; border.color: "#38393d"
            RowLayout { anchors.fill: parent; anchors.margins: 12; spacing: 10
                Text { text: "Title"; color: "#a1a2a7"; font.pixelSize: 13 }
                TextField { Layout.fillWidth: true; text: root.macro ? root.macro.name : ""; selectByMouse: true; onEditingFinished: root.invoke("set_title", { title: text }) }
                Text { text: "Speed"; color: "#a1a2a7"; font.pixelSize: 13 }
                Slider { id: speed; from: 0.1; to: 10; value: root.macro ? root.macro.speed_multiplier : 1; Layout.preferredWidth: 160; onMoved: root.invoke("set_macro_speed_multiplier", { multiplier: value }) }
                TextField { text: Number(speed.value).toFixed(2); Layout.preferredWidth: 62; horizontalAlignment: Text.AlignRight; onEditingFinished: root.invoke("set_macro_speed_multiplier", { multiplier: Number(text) }) }
                Text { text: "x"; color: "#a1a2a7" }
            }
        }
        Item {
            visible: !!root.macro
            Layout.fillWidth: true; Layout.fillHeight: true
            RowLayout { anchors.fill: parent; spacing: 0
                PalettePanel {
                    Layout.preferredWidth: 330; Layout.fillHeight: true
                    instructionTypes: root.instructionTypes
                    variables: root.macro ? root.macro.variables : []
                    blockDefinitions: root.macro ? root.macro.block_defs : []
                    onBlockActivated: type => root.addBlock(type, 120, 120)
                    onCustomBlockActivated: definition => root.addCustomBlock(definition, 120, 120)
                    onMakeVariableRequested: variableDialog.open()
                    onMakeBlockRequested: blockDialog.open()
                }
                BlockCanvas {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    strands: root.macro ? root.macro.strands : []
                    comments: root.macro ? root.macro.comments : []
                    floatingValues: root.macro ? root.macro.floating_values : []
                    locked: root.recording
                    onStrandMoved: (strandId, x, y) => root.invoke("move_strand", { strandId: strandId, x: x, y: y })
                    onBlockDropped: (type, x, y) => root.addBlock(type, x, y)
                    onInstructionRemoved: (strandId, path) => root.invoke("remove_instruction", { strandId: strandId, path: path })
                    onCommentMoved: (commentId, x, y) => root.invoke("move_comment", { commentId: commentId, x: x, y: y })
                    onCommentEdited: (commentId, text) => root.invoke("edit_comment_text", { commentId: commentId, text: text })
                    onCommentCollapseChanged: (commentId, collapsed) => root.invoke("set_comment_collapsed", { commentId: commentId, collapsed: collapsed })
                    onCommentRemoved: commentId => root.invoke("remove_comment", { commentId: commentId })
                    onFloatingValueMoved: (floatingId, x, y) => root.invoke("move_floating_value", { floatingId: floatingId, x: x, y: y })
                    onFloatingValueRemoved: floatingId => root.invoke("remove_floating_value", { floatingId: floatingId })
                }
            }
            Rectangle {
                visible: root.recording
                anchors.fill: parent; color: "#d8222326"; z: 20
                Column { anchors.centerIn: parent; spacing: 10
                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: "●"; color: "#ff4a4a"; font.pixelSize: 45 }
                    Text { anchors.horizontalCenter: parent.horizontalCenter; text: "Recording…"; color: "#ededee"; font.pixelSize: 17; font.weight: Font.Bold }
                    Text { text: "Adding and removing instructions is disabled while recording."; color: "#a5a6ab"; font.pixelSize: 14; font.weight: Font.DemiBold }
                }
            }
        }
        Item {
            visible: !root.macro
            Layout.fillWidth: true; Layout.fillHeight: true
            Column { anchors.centerIn: parent; spacing: 8
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: "▦"; color: "#777980"; font.pixelSize: 48 }
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: "No macro selected"; color: "#e7e7e8"; font.pixelSize: 16; font.weight: Font.DemiBold }
                Text { text: "Choose a macro above or create a new one."; color: "#9fa0a6"; font.pixelSize: 13 }
            }
        }
        Rectangle {
            visible: !!root.macro
            Layout.fillWidth: true; Layout.preferredHeight: 58; color: "#292a2d"; border.color: "#38393d"
            Row { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; anchors.leftMargin: 14; spacing: 8
                BwButton { glyph: "↶"; text: "Undo"; enabled: appState.can_undo; onClicked: root.invoke("undo") }
                BwButton { glyph: "↷"; text: "Redo"; enabled: appState.can_redo; onClicked: root.invoke("redo") }
                BwButton { glyph: "▣"; text: "Save macro"; primary: true; onClicked: root.invoke("save_macro") }
                BwButton { glyph: "+"; text: "Note"; onClicked: root.invoke("create_comment", { x: 360, y: 180, text: "New note" }) }
            }
        }
    }

    Dialog {
        id: removeDialog; anchors.centerIn: parent; modal: true; title: "Delete macro?"; standardButtons: Dialog.Yes | Dialog.Cancel
        Text { text: "This removes the selected macro from Blockwork."; color: "#e7e7e8" }
        onAccepted: root.invoke("remove_macro")
    }
    Dialog {
        id: macroSettings; anchors.centerIn: parent; modal: true; title: "Macro settings"; standardButtons: Dialog.Close
        Column { spacing: 14; width: 360
            Text { text: "Controls behavior specific to this macro."; color: "#a5a6ab" }
            BwSwitch { Accessible.name: "Listen for triggers while Blockwork is open"; checked: root.macro && root.macro.settings ? root.macro.settings.always_listen : false; onToggled: checked => root.invoke("set_macro_always_listen", { enabled: checked }) }
        }
    }
    Dialog {
        id: recordingSettings; anchors.centerIn: parent; modal: true; title: "Recording settings"; standardButtons: Dialog.Close
        Column { spacing: 14; width: 390
            BwSwitch { Accessible.name: "Record mouse movement"; checked: appState.record_mouse_movement; onToggled: checked => root.invoke("toggle_record_mouse_movement", { enabled: checked }) }
            BwSwitch { Accessible.name: "Use relative mouse movement"; checked: appState.record_mouse_relative; onToggled: checked => root.invoke("toggle_record_mouse_relative", { relative: checked }) }
            Text { width: parent.width; wrapMode: Text.WordWrap; text: appState.absolute_mouse_position_available ? "Absolute positioning is available on this system." : "Absolute positioning is unavailable; relative movement will be used."; color: "#9fa0a6"; font.pixelSize: 12 }
        }
    }
    Dialog {
        id: variableDialog; anchors.centerIn: parent; modal: true; title: "Make a Variable"; standardButtons: Dialog.Ok | Dialog.Cancel
        Column { width: 340; spacing: 8
            Text { text: "Variable name"; color: "#e7e7e8" }
            TextField { id: variableName; width: parent.width; placeholderText: "score"; selectByMouse: true }
        }
        onAccepted: {
            const name = variableName.text.trim();
            if (name.length) root.invoke("create_variable", { name: name });
            variableName.clear();
        }
    }
    Dialog {
        id: blockDialog; anchors.centerIn: parent; modal: true; title: "Make a Block"; standardButtons: Dialog.Ok | Dialog.Cancel
        Column { width: 380; spacing: 10
            Text { text: "Block label"; color: "#e7e7e8" }
            TextField { id: blockName; width: parent.width; placeholderText: "do something"; selectByMouse: true }
            Text { text: "Shape"; color: "#e7e7e8" }
            ComboBox { id: blockShape; width: parent.width; model: ["Normal", "Ending", "ReturnsValue", "ReturnsBool"] }
            Text { text: "Accent color"; color: "#e7e7e8" }
            TextField { id: blockColor; width: parent.width; text: "#1594ff"; selectByMouse: true }
        }
        onAccepted: {
            const label = blockName.text.trim();
            if (label.length) root.invoke("create_block", {
                pieces: [{ kind: "Label", id: root.uuid(), text: label }],
                shape: blockShape.currentText,
                color: blockColor.text
            });
            blockName.clear();
        }
    }
}
