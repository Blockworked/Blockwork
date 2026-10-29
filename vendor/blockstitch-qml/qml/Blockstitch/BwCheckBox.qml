import QtQuick
import QtQuick.Controls

CheckBox {
    id: control
    implicitHeight: 28
    spacing: 8
    font.pixelSize: 12
    HoverHandler { cursorShape: control.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor }
    indicator: Rectangle {
        implicitWidth: 17; implicitHeight: 17
        x: control.leftPadding; y: Math.round((control.height-height)/2)
        radius: 4
        color: control.checked ? Theme.accent : Theme.field
        border.color: control.checked ? Theme.accentHover : (control.activeFocus ? Theme.accent : Theme.border)
        Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
        Behavior on border.color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
        // The tick pops in when checked and shrinks away when cleared.
        LucideIcon {
            anchors.centerIn: parent; width: 12; height: 12; name: "check"; color: "white"; strokeWidth: 2.6
            opacity: control.checked ? 1 : 0; scale: control.checked ? 1 : 0.4; visible: opacity > 0
            Behavior on opacity { enabled: Motion.enabled; NumberAnimation { duration: Motion.quick } }
            Behavior on scale { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutBack } }
        }
    }
    contentItem: Text {
        leftPadding: control.indicator.width + control.spacing
        text: control.text; font: control.font; color: Theme.textDim; opacity: control.enabled ? 1 : .45
        verticalAlignment: Text.AlignVCenter
    }
}
