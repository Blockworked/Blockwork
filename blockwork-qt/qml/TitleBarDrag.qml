import QtQuick

// Empty bar space drags the frameless window; double-click toggles maximize.
Item {
    readonly property var win: Window.window
    DragHandler {
        target: null
        onActiveChanged: if (active && parent.win) parent.win.startSystemMove()
    }
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onDoubleTapped: if (parent.win) parent.win.visibility === Window.Maximized ? parent.win.showNormal() : parent.win.showMaximized()
    }
}
