import QtQuick

// Press-and-drag surface for blocks. Reports the pointer in *scene*
// coordinates so callers can map it into whatever (scaled, scrolled) space they
// need, and never relies on Qt's DragHandler translation, which is reset by the
// time a drag ends.
Item {
    id: area
    property bool dragEnabled: true
    property real threshold: 5
    readonly property bool dragging: internal.dragging
    readonly property bool hovered: hover.hovered
    // A canvas gesture disables every nested drag before the view moves.
    readonly property bool touchBlocked: {
        let item = parent;
        while (item) {
            if (item.isBlockCanvas) return item.touchNavigationActive;
            item = item.parent;
        }
        return false;
    }
    onTouchBlockedChanged: if (touchBlocked) cancelPointer()

    signal dragBegan(real sceneX, real sceneY, real offsetX, real offsetY)
    signal dragMoved(real sceneX, real sceneY)
    signal dragEnded(real sceneX, real sceneY)
    signal dragCanceled()
    signal contextRequested(real x, real y)
    signal activated()
    // A left press and release that never became a drag.
    signal tapped()

    function cancelPointer() {
        internal.armed = false;
        if (internal.dragging) { internal.dragging = false; area.dragCanceled(); }
    }

    QtObject {
        id: internal
        property bool held: false
        property bool armed: false
        property bool dragging: false
        property point pressScene
        property point pressLocal
    }
    HoverHandler {
        id: hover
        cursorShape: internal.dragging ? Qt.ClosedHandCursor : (area.dragEnabled ? Qt.OpenHandCursor : Qt.ArrowCursor)
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        preventStealing: true
        // Touch is handled below. Never also run the synthesized mouse path.
        onPressed: mouse => {
            if (mouse.source !== Qt.MouseEventNotSynthesized) { mouse.accepted = false; return; }
            if (mouse.button === Qt.RightButton) { area.contextRequested(mouse.x, mouse.y); return; }
            internal.pressScene = area.mapToItem(null, mouse.x, mouse.y);
            internal.pressLocal = Qt.point(mouse.x, mouse.y);
            internal.armed = area.dragEnabled;
        }
        onPositionChanged: mouse => {
            if (!internal.armed) return;
            const p = area.mapToItem(null, mouse.x, mouse.y);
            if (!internal.dragging) {
                if (Math.hypot(p.x - internal.pressScene.x, p.y - internal.pressScene.y) < area.threshold) return;
                internal.dragging = true;
                area.dragBegan(internal.pressScene.x, internal.pressScene.y, internal.pressLocal.x, internal.pressLocal.y);
            }
            area.dragMoved(p.x, p.y);
        }
        onReleased: mouse => {
            if (!internal.armed) return;
            internal.armed = false;
            if (!internal.dragging) { if (mouse.button === Qt.LeftButton) area.tapped(); return; }
            internal.dragging = false;
            const p = area.mapToItem(null, mouse.x, mouse.y);
            area.dragEnded(p.x, p.y);
        }
        onCanceled: area.cancelPointer()
        onDoubleClicked: mouse => { if (mouse.button === Qt.LeftButton) area.activated(); }
    }
    TapHandler {
        id: touchTap
        acceptedDevices: PointerDevice.TouchScreen
        enabled: !area.touchBlocked
        gesturePolicy: TapHandler.DragThreshold
        onPressedChanged: if (!pressed) internal.held = false
        onTapped: if (!internal.held) area.tapped()
        onLongPressed: {
            internal.held = true;
            area.cancelPointer();
            area.contextRequested(point.position.x, point.position.y);
        }
    }
    DragHandler {
        id: touchDrag
        target: null
        acceptedDevices: PointerDevice.TouchScreen
        enabled: area.dragEnabled && !area.touchBlocked && !internal.held
        minimumPointCount: 1; maximumPointCount: 1
        grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.ApprovesTakeOverByAnything
        onActiveChanged: {
            if (active) {
                const p = centroid.scenePressPosition;
                const local = area.mapFromItem(null, p.x, p.y);
                internal.dragging = true;
                area.dragBegan(p.x, p.y, local.x, local.y);
            } else if (internal.dragging) {
                // Qt may cancel the grab just after activeChanged. Defer the
                // commit until the cancellation/two-contact handlers ran.
                const p = centroid.scenePosition;
                Qt.callLater(() => {
                    if (!internal || !internal.dragging) return;
                    internal.dragging = false;
                    area.dragEnded(p.x, p.y);
                });
            }
        }
        onCentroidChanged: if (active && internal.dragging) area.dragMoved(centroid.scenePosition.x, centroid.scenePosition.y)
        onCanceled: area.cancelPointer()
    }
    Component.onDestruction: area.cancelPointer()
}
