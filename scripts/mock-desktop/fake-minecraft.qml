// A stand-in game window for the mock desktop: sky, terrain, a crosshair and
// a pure red marker at the center (what the live window tests look for), plus
// a blocky "player" that walks back and forth for detection runs.
import QtQuick
import QtQuick.Window

Window {
    width: 854
    height: 480
    visible: true
    title: "Minecraft* 1.21.4 - Singleplayer"
    color: "#7fa9ff"

    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: parent.height * 0.45
        color: "#5b8c32"
        Rectangle {
            anchors { left: parent.left; right: parent.right; top: parent.top }
            height: 12
            color: "#79c05a"
        }
    }

    Item {
        id: player
        width: 36
        height: 108
        y: parent.height * 0.55 - height
        NumberAnimation on x { from: 80; to: 680; duration: 4000; loops: Animation.Infinite; easing.type: Easing.InOutSine }
        Rectangle { x: 6; y: 0; width: 24; height: 24; color: "#c69c6d" }
        Rectangle { x: 0; y: 24; width: 36; height: 42; color: "#2ea8b5" }
        Rectangle { x: 6; y: 66; width: 24; height: 42; color: "#3b3f9e" }
    }

    // Crosshair, with the red marker a few pixels across at the exact center.
    Rectangle { anchors.centerIn: parent; width: 20; height: 2; color: "white" }
    Rectangle { anchors.centerIn: parent; width: 2; height: 20; color: "white" }
    Rectangle { anchors.centerIn: parent; width: 6; height: 6; color: "#ff0000" }
}
