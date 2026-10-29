import QtQuick
import QtQuick.Controls

Control {
    id: root
    property bool checked: false
    signal toggled(bool checked)
    implicitWidth: row.implicitWidth
    implicitHeight: 28

    contentItem: Row {
        id: row
        spacing: 9
        anchors.verticalCenter: parent.verticalCenter
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 38; height: 21; radius: 11
            color: root.checked ? Theme.accent : Theme.panelRaised
            border.color: root.checked ? Theme.accentHover : Theme.border
            Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.normal } }
            Behavior on border.color { enabled: Motion.enabled; ColorAnimation { duration: Motion.normal } }
            Rectangle {
                x: root.checked ? 19 : 2; y: 2
                width: 17; height: 17; radius: 9
                color: "#f4f4f5"
                Behavior on x { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }
            }
            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.checked = !root.checked; root.toggled(root.checked) } }
        }
        Text { text: root.Accessible.name; color: Theme.text; font.pixelSize: 14; anchors.verticalCenter: parent.verticalCenter }
    }
}
