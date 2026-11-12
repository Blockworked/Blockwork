import QtQuick

// Register toolbar space and interactive controls with the window agent.
Item {
    id: root
    readonly property var win: Window.window
    readonly property var chrome: win && win.windowChrome ? win.windowChrome : null
    property var interactiveItems: []
    property var windowButtons: null
    property var nativeButtons: null
    function registerChrome() {
        if (chrome && chrome.ready && visible && windowButtons)
            chrome.registerTitleBar(dragArea, interactiveItems, windowButtons, nativeButtons);
    }
    onChromeChanged: Qt.callLater(registerChrome)
    onVisibleChanged: Qt.callLater(registerChrome)
    Component.onCompleted: Qt.callLater(registerChrome)
    Connections {
        target: root.chrome
        function onReadyChanged() { root.registerChrome(); }
    }
    // Leave the visible-frame resize grips outside QWindowKit's drag region.
    Item {
        id: dragArea
        objectName: "windowDragArea"
        anchors.fill: parent
        anchors.topMargin: Qt.platform.os === "linux" ? 7 : 0
        anchors.leftMargin: anchors.topMargin; anchors.rightMargin: anchors.topMargin
    }
}
