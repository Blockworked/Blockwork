import QtQuick
import QtQuick.Controls

// Themed slider. With stepSize set and ticks on, it draws a notch per step
// like a stepped system setting (e.g. KDE's animation speed).
Slider {
    id: control
    property bool ticks: false
    implicitWidth: 220
    implicitHeight: 26
    snapMode: Slider.SnapAlways
    HoverHandler { cursorShape: control.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor }

    background: Item {
        x: control.leftPadding; y: control.topPadding + control.availableHeight / 2 - height / 2
        width: control.availableWidth; height: 4
        Rectangle { anchors.fill: parent; radius: 2; color: Theme.field; border.color: Theme.border }
        Rectangle {
            width: 8 + control.visualPosition * (parent.width - 16); height: parent.height; radius: 2
            color: Theme.accent
        }
        Repeater {
            model: control.ticks && control.stepSize > 0 ? Math.round((control.to - control.from) / control.stepSize) + 1 : 0
            Rectangle {
                required property int index
                readonly property real pos: index * control.stepSize / (control.to - control.from)
                x: Math.round(8 + pos * (parent.width - 16)); y: parent.height + 5
                width: 1; height: 5; color: Theme.border
            }
        }
    }
    handle: Rectangle {
        x: control.leftPadding + control.visualPosition * (control.availableWidth - width)
        y: control.topPadding + control.availableHeight / 2 - height / 2
        width: 16; height: 16; radius: 8
        color: control.pressed ? "#e2e2e4" : "#f4f4f5"
        border.color: control.pressed || control.hovered || control.activeFocus ? Theme.accent : Theme.border
        Behavior on border.color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
    }
}
