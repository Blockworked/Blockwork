import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Keep standard buttons in the selected app language, including when Qt's
// own translation catalogs aren't installed on the user's computer.
BwDialog {
    id: root
    // Keep dimming inside the window frame, clear of its transparent shadow padding.
    Overlay.modal: Item {
        readonly property var appWindow: root.Overlay.overlay ? root.Overlay.overlay.Window.window : null
        Rectangle {
            anchors.fill: parent
            anchors.margins: parent.appWindow && parent.appWindow.shadowMargin !== undefined ? parent.appWindow.shadowMargin : 0
            radius: parent.appWindow && parent.appWindow.cornerRadius !== undefined ? parent.appWindow.cornerRadius : 0
            color: "#a3101113"
            antialiasing: true
        }
        Behavior on opacity { enabled: Motion.enabled; NumberAnimation { duration: Motion.slow } }
    }
    property string acceptText: qsTr("OK")
    readonly property var buttonLabels: [
        [Dialog.Ok, acceptText], [Dialog.Cancel, qsTr("Cancel")],
        [Dialog.Yes, qsTr("Yes")], [Dialog.No, qsTr("No")],
        [Dialog.Close, qsTr("Close")]
    ]

    function updateButtonLabels() {
        for (const entry of buttonLabels) {
            const button = standardButton(entry[0]);
            if (button) button.text = entry[1];
        }
    }
    onButtonLabelsChanged: Qt.callLater(updateButtonLabels)
    onStandardButtonsChanged: Qt.callLater(updateButtonLabels)
    Component.onCompleted: Qt.callLater(updateButtonLabels)
}
