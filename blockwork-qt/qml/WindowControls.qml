import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0 as Blockwork

// Minimize / maximize / close for the frameless window (Linux and Windows).
Row {
    id: root
    readonly property var win: Window.window
    readonly property bool maximized: !!win && win.visibility === Window.Maximized
    readonly property alias minimizeButton: minimizeControl
    readonly property alias maximizeButton: maximizeControl
    readonly property alias closeButton: closeControl
    spacing: 4

    component Control: ToolButton {
        id: control
        property string iconName
        property bool close: false
        property string label
        signal activated()
        width: 34; height: 34; padding: 9
        HoverHandler { cursorShape: control.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor }
        onClicked: activated()
        background: Rectangle {
            radius: 8
            border.width: control.visualFocus ? 1 : 0
            border.color: Theme.accent
            color: control.hovered || control.down ? (control.close ? "#e5484d" : Theme.hover) : "transparent"
            Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
        }
        contentItem: Blockwork.LucideIcon { name: control.iconName; color: Theme.text }
        ToolTip.visible: hovered
        ToolTip.text: label
        Accessible.name: label
    }
    Control { id: minimizeControl; iconName: "minus"; label: qsTr("Minimize"); onActivated: if (root.win) root.win.showMinimized() }
    Control {
        id: maximizeControl
        iconName: root.maximized ? "copy" : "square"
        label: root.maximized ? qsTr("Restore") : qsTr("Maximize")
        onActivated: if (root.win) root.maximized ? root.win.showNormal() : root.win.showMaximized()
    }
    Control { id: closeControl; iconName: "x"; label: qsTr("Close"); close: true; onActivated: if (root.win) root.win.close() }
}
