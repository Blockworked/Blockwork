import QtQuick
import QtQuick.Controls

MenuItem {
    id: control
    property string iconName: ""
    property bool danger: false
    implicitWidth: 210
    implicitHeight: Math.max(34, Theme.minimumTarget)
    height: visible ? implicitHeight : 0
    leftPadding: 10; rightPadding: 12
    HoverHandler { cursorShape: Qt.PointingHandCursor }
    contentItem: Row {
        spacing: 9
        LucideIcon { name: control.iconName; visible: control.iconName.length>0; width: 15; height: 15; color: control.danger?Theme.danger:Theme.textDim; anchors.verticalCenter: parent.verticalCenter }
        Text { text: control.text; color: control.danger?Theme.danger:Theme.text; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter; verticalAlignment: Text.AlignVCenter }
    }
    background: Rectangle {
        radius: 5; color: control.highlighted ? Theme.hover : "transparent"
        Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
    }
}
