import QtQuick

// One draggable value/operator chip in the palette.
Item {
    id: root
    property var valueData
    property var spec: ({})
    property bool editable: true
    property color referenceColor: Theme.accent
    property bool forceBoolean: false
    property string callLabel: qsTr("custom block")
    property var blockDefinitions: []
    signal dragStarted(var spec, real sceneX, real sceneY, real offsetX, real offsetY)
    signal dragMoved(real sceneX, real sceneY)
    signal dragEnded(real sceneX, real sceneY)
    signal dragCanceled()
    signal activated()
    signal detailsRequested(string kind)
    property var extraMenu: []
    signal extraMenuTriggered(string id)
    // The prefab was edited in place (a typed number, a chosen dropdown entry).
    signal valueEdited(var value)
    implicitWidth: chip.implicitWidth
    implicitHeight: chip.implicitHeight
    width: implicitWidth; height: implicitHeight
    z: chip.previewShown ? 100 : 0

    BlockDragArea {
        anchors.fill: parent
        onDragBegan: (sx, sy, ox, oy) => root.dragStarted(root.spec, sx, sy, ox, oy)
        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
        onDragCanceled: root.dragCanceled()
        onActivated: root.activated()
        onTapped: { if (Theme.touchMode) root.activated(); else chip.requestPreview(); }
        onContextRequested: (x, y) => chip.showContextMenu(x, y)
    }
    ValueChip {
        id: chip
        valueData: root.valueData; editable: root.editable; boxed: true
        referenceColor: root.referenceColor
        forceBoolean: root.forceBoolean; callDisplayLabel: root.callLabel
        blockDefinitions: root.blockDefinitions; paletteMode: true
        onDetailsRequested: kind => root.detailsRequested(kind)
        extraMenu: root.extraMenu
        onExtraMenuTriggered: id => root.extraMenuTriggered(id)
        onLeafEdited: (path, leaf) => {
            if (!path.length) { root.valueData = leaf; root.valueEdited(leaf); return; }
            const next = JSON.parse(JSON.stringify(root.valueData));
            let v = next;
            for (let i = 0; i < path.length - 1; ++i) v = v.args[path[i]];
            v.args[path[path.length - 1]] = leaf;
            root.valueData = next;
            root.valueEdited(next);
        }
    }
}
