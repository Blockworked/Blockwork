import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Record button settings: mouse movement and the countdown.
AppDialog {
    id: recordingSettings; title: qsTr("Recording settings"); standardButtons: Dialog.Close
    required property Item page
    Column { spacing: 14; width: 390
        BwSwitch { Accessible.name: qsTr("Record mouse movement"); checked: appState.record_mouse_movement; onToggled: checked => recordingSettings.page.invoke("toggle_record_mouse_movement", { enabled: checked }) }
        BwSwitch { Accessible.name: qsTr("Use relative mouse movement"); checked: appState.record_mouse_relative; onToggled: checked => recordingSettings.page.invoke("toggle_record_mouse_relative", { relative: checked }) }
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
                            recordingSettings.page.invoke("set_recording_countdown", { seconds: Math.round(value) });
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
