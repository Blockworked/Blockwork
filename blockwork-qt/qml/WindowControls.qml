import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Minimize / maximize / close for the frameless window (Linux and Windows).
Row {
    id: root
    readonly property var win: Window.window
    readonly property bool maximized: !!win && win.visibility === Window.Maximized
    spacing: 4

    component Control: Rectangle {
        id: control
        property string iconName
        property bool close: false
        signal activated()
        width: 34; height: 34; radius: 8
        color: hover.hovered ? (close ? "#e5484d" : "#3b3c40") : "transparent"
        HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: control.activated() }
        LucideIcon { anchors.centerIn: parent; width: 16; height: 16; name: control.iconName; color: Theme.text }
    }
    Control { iconName: "minus"; onActivated: if (root.win) root.win.showMinimized() }
    Control {
        iconName: root.maximized ? "copy" : "square"
        onActivated: if (root.win) root.maximized ? root.win.showNormal() : root.win.showMaximized()
    }
    Control { iconName: "x"; close: true; onActivated: if (root.win) root.win.close() }
}
