// A window that shows and logs the input it gets, to check what reached the
// desktop. Every key, click and scroll is printed as "input-target: ...".
import QtQuick
import QtQuick.Window

Window {
    id: root
    width: 640
    height: 360
    x: 1200
    y: 600
    visible: true
    title: "Input target"
    color: "#202830"

    function note(line) {
        console.log("input-target: " + line)
        log.text = (line + "\n" + log.text).split("\n").slice(0, 8).join("\n")
    }

    TextEdit {
        id: typed
        focus: true
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 16 }
        height: 120
        color: "white"
        font.pixelSize: 28
        wrapMode: TextEdit.Wrap
        onTextChanged: root.note("text " + JSON.stringify(text))
        Keys.onPressed: event => root.note("key " + event.key + " " + JSON.stringify(event.text))
    }

    Text {
        id: log
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 16 }
        color: "#9fb4c8"
        font.pixelSize: 16
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onPressed: mouse => { typed.forceActiveFocus(); root.note("press " + mouse.button + " at " + mouse.x + "," + mouse.y) }
        onWheel: wheel => root.note("wheel " + wheel.angleDelta.y)
    }
}
