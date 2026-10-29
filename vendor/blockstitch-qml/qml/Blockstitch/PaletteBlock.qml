import QtQuick

// One draggable instruction block in the palette. Dragging reports a "spec" that
// the owner turns into a ghost and, on drop, into a new block on the canvas.
Item {
    id: root
    property var instruction
    property var spec: ({})
    property var variables: []
    property var lists: []
    property var blockDefinitions: []
    property var keyCapture: null
    property color blockColor: Theme.block
    signal dragStarted(var spec, real sceneX, real sceneY, real offsetX, real offsetY)
    signal dragMoved(real sceneX, real sceneY)
    signal dragEnded(real sceneX, real sceneY)
    signal dragCanceled()
    signal activated()
    signal keyCaptureRequested()
    signal appPickerRequested(var instruction)
    // A registered row's `palette` button was clicked.
    signal pieceActionRequested(string action, var instruction)
    signal detailsRequested(string type)
    property var extraMenu: []
    signal extraMenuTriggered(string id)
    // The prefab was edited in place; the palette keeps its own copy.
    signal instructionEdited(var instruction)
    implicitWidth: block.implicitWidth
    implicitHeight: block.implicitHeight
    width: implicitWidth; height: implicitHeight

    InstructionBlock {
        id: block
        instruction: root.instruction; variables: root.variables; lists: root.lists; blockDefinitions: root.blockDefinitions
        keyCapture: root.keyCapture; blockColor: root.blockColor; paletteMode: true; locked: true
        onKeyCaptureRequested: root.keyCaptureRequested()
        onAppPickerRequested: (sid, p, i) => root.appPickerRequested(i)
        onPieceActionRequested: (a, sid, p, i) => root.pieceActionRequested(a, i)
        onDetailsRequested: type => root.detailsRequested(type)
        extraMenu: root.extraMenu
        onExtraMenuTriggered: id => root.extraMenuTriggered(id)
        onDragBegan: (sid, p, tail, sx, sy, ox, oy) => root.dragStarted(root.spec, sx, sy, ox, oy)
        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
        onDragCanceled: root.dragCanceled()
        onActivated: root.activated()
        onTapped: if (Theme.touchMode) root.activated()
        onInstructionEdited: (sid, p, next) => { root.instruction = next; root.instructionEdited(next); }
    }
}
