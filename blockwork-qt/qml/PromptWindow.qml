import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// One macro dialog (`blockwork --prompt`): ask, yes/no, choose, message or
// permission. Closing the window answers "cancel".
ApplicationWindow {
    id: root
    readonly property var request: { try { return JSON.parse(bridge.requestJson); } catch (e) { return ({}); } }
    readonly property string kind: request.kind || "message"
    Binding { target: Theme; property: "mode"; value: blockworkAppearance }
    Binding { target: Motion; property: "durationScale"; value: root.request.animation_speed ?? 1 }
    width: 440
    height: Math.min(560, content.implicitHeight + 40)
    minimumWidth: 320
    visible: true
    title: kind === "permission" ? qsTr("Blockwork permission") : (request.title || "Blockwork")
    flags: Qt.Dialog | Qt.WindowStaysOnTopHint
    color: Theme.panel
    palette {
        window: Theme.panel; windowText: Theme.text; base: Theme.field; text: Theme.text
        button: Theme.panelRaised; buttonText: Theme.text; highlight: Theme.accent; highlightedText: "white"
        placeholderText: Theme.textDim; mid: Theme.border; dark: Theme.borderSoft; light: Theme.panelRaised
    }

    PromptBridge { id: bridge }

    property bool answered: false
    function answer(button, text) {
        if (answered) return;
        answered = true;
        bridge.finish(button, text || "");
    }
    onClosing: answer(kind === "message" ? "ok" : "cancel", "")
    Component.onCompleted: {
        raise(); requestActivate(); if (kind === "text") field.forceActiveFocus();
    }
    // The daemon starts dialogs in the background, where some systems
    // (Windows) won't let them take focus by asking; keep pulling until it's ours.
    Timer {
        property int tries: 0
        interval: 100
        repeat: true
        running: !root.active && tries < 20
        onTriggered: { tries++; bridge.bringToFront(); }
    }
    // Permission questions are worded here so they follow the UI language.
    readonly property string message: {
        if (kind !== "permission" || !request.ability) return request.message || "";
        const name = request.subject || "";
        const phrases = {
            commands: qsTr("\"%1\" wants to run commands and open or close apps."),
            network: qsTr("\"%1\" wants to go online."),
            files_write: qsTr("\"%1\" wants to write to files."),
            secrets: qsTr("\"%1\" wants to read your saved secrets."),
            screen: qsTr("\"%1\" wants to read the screen.")
        };
        return phrases[request.ability] ? phrases[request.ability].arg(name) : (request.message || "");
    }

    // [label, answer, primary] per kind, left to right.
    readonly property var buttons: ({
        text: [[qsTr("Cancel"), "cancel", false], [qsTr("OK"), "ok", true]],
        confirm: [[qsTr("No"), "no", false], [qsTr("Yes"), "yes", true]],
        choice: [[qsTr("Cancel"), "cancel", false], [qsTr("OK"), "ok", true]],
        message: [[qsTr("OK"), "ok", true]],
        permission: [[qsTr("Deny"), "deny", false], [qsTr("Allow this time"), "once", false], [qsTr("Always allow"), "always", true]]
    })[kind] || [[qsTr("OK"), "ok", true]]
    function accept() {
        if (kind === "choice") { if (choices.currentIndex >= 0) answer("ok", request.options[choices.currentIndex]); }
        else if (kind === "text") answer("ok", field.text);
        else answer(buttons[buttons.length - 1][1], "");
    }
    Shortcut { sequences: ["Return", "Enter"]; onActivated: root.accept() }
    Shortcut { sequence: "Escape"; onActivated: root.close() }

    ColumnLayout {
        id: content
        anchors.fill: parent; anchors.margins: 20
        spacing: 14
        RowLayout {
            spacing: 12
            LucideIcon {
                Layout.alignment: Qt.AlignTop
                width: 22; height: 22
                name: root.kind === "permission" ? "lock" : root.kind === "message" ? "message-square" : "info"
                color: root.kind === "permission" ? Theme.warning : Theme.accent
            }
            Text {
                Layout.fillWidth: true
                text: root.message
                wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 14
            }
        }
        Text {
            visible: root.kind === "permission"
            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Macros you import ask before running commands, going online, writing files, reading secrets or reading the screen. Change what this macro may do in its Macro settings.")
        }
        BwTextField {
            id: field
            visible: root.kind === "text"
            Layout.fillWidth: true
            text: root.request.default || ""
            onAccepted: root.accept()
        }
        ListView {
            id: choices
            visible: root.kind === "choice"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(240, contentHeight)
            clip: true; currentIndex: 0
            model: root.request.options || []
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            WheelPan { flickable: choices }
            delegate: Rectangle {
                required property string modelData
                required property int index
                width: choices.width; height: 34; radius: 6
                color: ListView.isCurrentItem ? Theme.selection : hover.hovered ? Theme.panelRaised : "transparent"
                Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                HoverHandler { id: hover }
                Text { anchors.verticalCenter: parent.verticalCenter; x: 10; width: parent.width - 20; text: modelData; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 13 }
                TapHandler { onTapped: choices.currentIndex = index; onDoubleTapped: root.answer("ok", modelData) }
            }
            Keys.onUpPressed: decrementCurrentIndex()
            Keys.onDownPressed: incrementCurrentIndex()
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Item { Layout.fillWidth: true }
            Repeater {
                model: root.buttons
                delegate: BwButton {
                    required property var modelData
                    text: modelData[0]; primary: modelData[2]
                    onClicked: modelData[1] === "ok" ? root.accept() : root.answer(modelData[1], "")
                }
            }
        }
    }
}
