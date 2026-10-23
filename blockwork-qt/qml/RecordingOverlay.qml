import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Covers the editor while a recording counts down or runs. The canvas stays
// visible behind it but takes no input; a card says what is happening, where
// the recorded blocks go and how to stop.
Item {
    id: root
    property bool recording: false
    // Seconds left before recording starts; 0 when not counting down.
    property int countdown: 0
    property bool mouseMovement: false
    // The Stop Recording hotkey, or "" when none is set.
    property string shortcut: ""
    signal stopRequested()

    readonly property bool active: recording || countdown > 0
    visible: opacity > 0
    opacity: active ? 1 : 0
    Behavior on opacity { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal } }

    property real startedAt: 0
    property int elapsed: 0
    onRecordingChanged: { startedAt = Date.now(); elapsed = 0; }
    Timer {
        running: root.recording; interval: 250; repeat: true
        onTriggered: root.elapsed = Math.floor((Date.now() - root.startedAt) / 1000)
    }
    function clock(secs) { return Math.floor(secs / 60) + ":" + String(secs % 60).padStart(2, "0"); }

    Rectangle { anchors.fill: parent; color: "#c41a1b1d" }
    // Swallow clicks, drags and wheel turns so nothing behind can be edited.
    MouseArea {
        anchors.fill: parent; enabled: root.active
        hoverEnabled: true; acceptedButtons: Qt.AllButtons; preventStealing: true
        onWheel: wheel => wheel.accepted = true
    }

    Rectangle {
        id: card
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 420)
        height: content.implicitHeight + 48
        radius: 12; color: Theme.panel
        border.color: root.recording ? Theme.dangerBorder : Theme.borderSoft
        Behavior on border.color { enabled: Motion.enabled; ColorAnimation { duration: Motion.normal } }
        scale: root.active ? 1 : 0.96
        Behavior on scale { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }

        Column {
            id: content
            x: 24; y: 24; width: card.width - 48
            spacing: 16

            // Countdown: a big number, then what to do meanwhile.
            Column {
                visible: !root.recording
                width: parent.width; spacing: 4
                Text { text: qsTr("Recording starts in"); color: Theme.textDim; font.pixelSize: 13 }
                Text {
                    text: root.countdown; color: Theme.text
                    font.pixelSize: 56; font.weight: Font.Bold; font.features: { "tnum": 1 }
                }
                Text {
                    width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 14
                    text: qsTr("Switch to the app you want to record.")
                }
            }

            // Recording: a pulsing dot, the elapsed time and where the blocks go.
            Item {
                visible: root.recording
                width: parent.width; height: 28
                Row {
                    anchors.verticalCenter: parent.verticalCenter; spacing: 10
                    Item {
                        width: 14; height: 14; anchors.verticalCenter: parent.verticalCenter
                        Rectangle {
                            id: pulse
                            anchors.centerIn: parent; width: 14; height: 14; radius: 7
                            color: Theme.danger; opacity: 0
                            SequentialAnimation on scale {
                                running: root.recording && Motion.enabled; loops: Animation.Infinite
                                NumberAnimation { from: 1; to: 2.2; duration: 1100; easing.type: Easing.OutCubic }
                                PauseAnimation { duration: 300 }
                            }
                            SequentialAnimation on opacity {
                                running: root.recording && Motion.enabled; loops: Animation.Infinite
                                NumberAnimation { from: 0.45; to: 0; duration: 1100; easing.type: Easing.OutCubic }
                                PauseAnimation { duration: 300 }
                            }
                        }
                        Rectangle { anchors.centerIn: parent; width: 12; height: 12; radius: 6; color: Theme.danger }
                    }
                    Text { text: qsTr("Recording"); color: Theme.text; font.pixelSize: 18; font.weight: Font.Bold }
                }
                Text {
                    anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                    text: root.clock(root.elapsed); color: Theme.textDim
                    font.pixelSize: 15; font.features: { "tnum": 1 }
                }
            }
            Text {
                visible: root.recording
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 14; lineHeight: 1.15
                text: root.mouseMovement
                    ? qsTr("Your keys, clicks and mouse movement are being recorded. When you stop, they're added as blocks to the end of the recording target (the strand with the red dot).")
                    : qsTr("Your keys and clicks are being recorded. When you stop, they're added as blocks to the end of the recording target (the strand with the red dot).")
            }
            // The Stop Recording hotkey works from any app, unlike the button.
            Text {
                visible: root.recording && root.shortcut.length > 0
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 13
                textFormat: Text.StyledText
                text: qsTr("Press %1 in any app to stop.").arg("<font color=\"" + Theme.text + "\"><b>" + root.shortcut.replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</b></font>")
            }

            Rectangle { width: parent.width; height: 1; color: Theme.borderSoft }

            Item {
                width: parent.width; height: stopButton.implicitHeight
                Text {
                    anchors.left: parent.left; anchors.right: stopButton.left; anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                    text: qsTr("Editing is paused while recording.")
                }
                BwButton {
                    id: stopButton
                    anchors.right: parent.right
                    implicitHeight: 34
                    danger: root.recording
                    iconName: root.recording ? "square" : "x"; iconFilled: root.recording
                    text: root.recording ? qsTr("Stop recording") : qsTr("Cancel")
                    onClicked: root.stopRequested()
                }
            }
        }
    }
}
