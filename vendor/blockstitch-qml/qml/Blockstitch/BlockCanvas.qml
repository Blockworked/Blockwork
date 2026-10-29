import QtQuick
import QtQuick.Controls
import QtQuick.Shapes

Rectangle {
    id: root
    readonly property bool isBlockCanvas: true
    readonly property var branchDragState: dragSession
    property var strands: []
    property string recordingTargetId: ""
    property var comments: []
    property Component textMenu: null
    property var floatingValues: []
    property var variables: []
    property var lists: []
    property var pendingListFocus: null
    property var dicts: []
    property var pendingDictFocus: null
    property var blockDefinitions: []
    property var keyCapture: null
    readonly property bool touchNavigationActive: screenPinch.active || twoFingerPan.active
    signal touchNavigationStarted()
    property var insertionPoint: null
    property string _insertionDocument: ""
    property bool locked: false
    // Whether the canvas menu offers "Delete all blocks".
    property bool offerClear: true
    property string clearText: qsTr("Delete all blocks")
    property string clearIcon: "trash"
    property bool canPaste: false
    Binding { target: BlockRegistry; property: "recordingTargetId"; value: root.recordingTargetId }
    Binding { target: BlockRegistry; property: "commentedIds"; value: (root.comments || []).filter(c => c.attached_to).map(c => c.attached_to) }
    property real zoom: 1.0
    signal strandMoved(string strandId, int x, int y)
    signal instructionSplit(string strandId, var path, int x, int y)
    signal strandsMerged(string draggedId, string targetId, var path, var targetY)
    signal tailMerged(string strandId, var path, string targetId, var targetPath, var targetY)
    signal blockDragOutside(string strandId, var path, int tailCount, real sceneX, real sceneY)
    signal instructionRemoved(string strandId, var path)
    signal instructionDuplicated(string strandId, var path, var instruction)
    signal instructionEdited(string strandId, var path, var instruction)
    signal runBranchRequested(string strandId, var path, string name)
    signal valueEdited(var location, string text)
    signal commentForInstructionRequested(var instruction)
    signal recordingTargetRequested(string strandId)
    signal canvasNoteRequested(int x, int y)
    signal clearRequested()
    signal commentMoved(string commentId, int x, int y)
    signal commentEdited(string commentId, string text)
    signal commentCollapseChanged(string commentId, bool collapsed)
    signal commentRemoved(string commentId)
    signal floatingValueMoved(string floatingId, int x, int y)
    signal floatingValueRemoved(string floatingId)
    signal listItemsEdited(string name, var items)
    signal listEditorStateChanged(string name, bool visible, int x, int y)
    signal dictEntriesEdited(string name, var entries)
    signal dictEditorStateChanged(string name, bool visible, int x, int y)
    signal blockMenuAction(string action, string strandId, var path, var instruction)
    signal canvasMenuAction(string action, int x, int y)
    // A parameter oval dragged out of a block header; the host runs it like a palette drag.
    signal paletteDragStarted(var spec, real sceneX, real sceneY, real offsetX, real offsetY)
    signal paletteDragMoved(real sceneX, real sceneY)
    signal paletteDragEnded(real sceneX, real sceneY)
    signal paletteDragCanceled()
    signal keyCaptureRequested(string strandId, var path)
    signal appPickerRequested(string strandId, var path, var instruction)
    signal pieceActionRequested(string action, string strandId, var path, var instruction)
    signal detailsRequested(string type)
    signal valueTakeRequested(var location)
    signal valuePutRequested(var location, var value)
    signal valueCreateRequested(int x, int y, var value, string originBlockId)
    // Moves a value in one edit: into `to`, or parked at x/y when `to` is null.
    signal valueMoveRequested(var from, var to, int x, int y, string originBlockId)
    signal valueDragOutside(var location, var value, real sceneX, real sceneY)
    color:Theme.canvas; clip:true
    // Reporter name dropdowns offer whatever this document declares.
    onListsChanged: {
        BlockRegistry.listNames = (lists || []).map(l => l.name);
        syncListEditors();
    }
    // Keep the popup and its pointer grab alive across daemon updates.
    ListModel { id: listEditorModel }
    ListModel { id: dictEditorModel }
    function syncListEditors() { syncCollectionEditors(listEditorModel, root.lists || []); }
    function syncDictEditors() { syncCollectionEditors(dictEditorModel, root.dicts || []); }
    function syncCollectionEditors(model, list) {
        for (let i = model.count - 1; i >= 0; --i) {
            if (!list.some(entry => entry.name === model.get(i).editorName))
                model.remove(i);
        }
        for (let i = 0; i < list.length; ++i) {
            const entry = list[i];
            const data = {editorName: entry.name, editorX: entry.editor_x ?? 36,
                editorY: entry.editor_y ?? 36, payload: JSON.stringify(entry)};
            let at = -1;
            for (let j = 0; j < model.count; ++j)
                if (model.get(j).editorName === entry.name) { at = j; break; }
            if (at < 0) model.insert(i, data);
            else {
                if (at !== i) model.move(at, i, 1);
                for (const role of ["editorX", "editorY", "payload"])
                    if (model.get(i)[role] !== data[role])
                        model.setProperty(i, role, data[role]);
            }
        }
    }
    onDictsChanged: {
        BlockRegistry.dictNames = (dicts || []).map(d => d.name);
        syncDictEditors();
    }
    // World coordinates have their origin at the center of the workspace:
    // world (0, 0) renders at workspace-local (originOffsetX, originOffsetY).
    // Strands, comments and floating values use world coordinates;
    // everything rendered inside `workspace` adds the offset, and every
    // coordinate emitted back to the backend subtracts it again.
    // The world grows symmetrically to fit its content: a fixed-size world
    // clips strands placed far from the origin (and hides them from scroll),
    // so expand it beyond the minimum whenever strands/comments/values
    // extend past it.
    readonly property real minWorldW: 2600
    readonly property real minWorldH: 1800
    // Horizontal gets extra trailing room so the canvas extends a good
    // distance past a strand's right edge (comfortable panning / drop
    // space), while vertical keeps the tighter bound.
    readonly property real canvasPadX: 800
    readonly property real canvasPadY: 400
    // Fallback width until a strand delegate reports its real implicitWidth.
    readonly property real strandEstW: 360
    // Live measured strand widths (sid -> implicitWidth), reported by the
    // strand delegates below. Reassigned as a whole on every report so
    // needHalfW re-evaluates. Without this the world is sized from a fixed
    // estimate, so wide strands eat the padding and the canvas ends right
    // at (or inside) their right edge.
    property var _strandWidths: ({})
    function noteStrandWidth(sid, w) {
        if (!sid || !(w > 0)) return;
        if (_strandWidths[sid] === w) return;
        var m = Object.assign({}, _strandWidths);
        m[sid] = w;
        _strandWidths = m;
    }
    function clearStrandWidth(sid) {
        if (!sid || !(_strandWidths && (sid in _strandWidths))) return;
        var m = Object.assign({}, _strandWidths);
        delete m[sid];
        _strandWidths = m;
    }
    function strandWidthFor(sid) {
        var w = _strandWidths && _strandWidths[sid];
        return (w > 0) ? w : root.strandEstW;
    }
    readonly property real needHalfW: {
        var hw = 0;
        var sw = root._strandWidths;
        var sl = root.strands || [];
        for (var i = 0; i < sl.length; ++i) {
            var sx = (sl[i] && sl[i].x) || 0;
            var w = (sw && sw[sl[i].id] > 0) ? sw[sl[i].id] : root.strandEstW;
            hw = Math.max(hw, Math.abs(sx), Math.abs(sx + w));
        }
        var fv = root.floatingValues || [];
        for (var j = 0; j < fv.length; ++j) {
            var fx = (fv[j] && fv[j].x) || 0;
            hw = Math.max(hw, Math.abs(fx), Math.abs(fx + 160));
        }
        var cm = root.comments || [];
        for (var k = 0; k < cm.length; ++k) {
            var cx = (cm[k] && cm[k].x) || 0;
            hw = Math.max(hw, Math.abs(cx), Math.abs(cx + 220));
        }
        return hw;
    }
    // Use measured strand heights so layout changes do not search every control.
    property var _strandHeights: ({})
    function noteStrandHeight(sid, h) {
        if (!sid || !(h >= 0) || _strandHeights[sid] === h) return;
        const heights = Object.assign({}, _strandHeights);
        heights[sid] = h;
        _strandHeights = heights;
    }
    function clearStrandHeight(sid) {
        if (!(sid in _strandHeights)) return;
        const heights = Object.assign({}, _strandHeights);
        delete heights[sid];
        _strandHeights = heights;
    }
    readonly property real needHalfH: {
        var hh = 0;
        var sl = root.strands || [];
        for (var i = 0; i < sl.length; ++i) {
            var sy = (sl[i] && sl[i].y) || 0;
            var sh = root._strandHeights[sl[i].id] ?? 58;
            hh = Math.max(hh, Math.abs(sy), Math.abs(sy + sh));
        }
        var fv = root.floatingValues || [];
        for (var j = 0; j < fv.length; ++j) {
            var fy = (fv[j] && fv[j].y) || 0;
            hh = Math.max(hh, Math.abs(fy), Math.abs(fy + 40));
        }
        var cm = root.comments || [];
        for (var k = 0; k < cm.length; ++k) {
            var cy = (cm[k] && cm[k].y) || 0;
            var chh = (cm[k] && cm[k].collapsed) ? 38 : 280;
            hh = Math.max(hh, Math.abs(cy), Math.abs(cy + chh));
        }
        return hh;
    }
    readonly property real worldW: Math.max(root.minWorldW, 2 * (root.needHalfW + root.canvasPadX))
    readonly property real worldH: Math.max(root.minWorldH, 2 * (root.needHalfH + root.canvasPadY))
    readonly property real originOffsetX: worldW / 2
    readonly property real originOffsetY: worldH / 2
    // Growing the world moves the origin, which shifts every item's
    // workspace position by half the growth. Compensate the scroll by the
    // same amount so the view stays visually still instead of jumping.
    property real _prevWorldW: 0
    property real _prevWorldH: 0
    property var _resizeViewX: null
    property var _resizeViewY: null
    property real _resizeScrollX: NaN
    property real _resizeScrollY: NaN
    function pauseResizeScroll() {
        if (canvasScroll.animX.running) _resizeScrollX = canvasScroll.targetX;
        if (canvasScroll.animY.running) _resizeScrollY = canvasScroll.targetY;
        canvasScroll.stop();
    }
    // Apply after Flickable updates its bounds, including automatic clamping.
    function applyWorldResize() {
        if (_resizeViewX !== null) {
            flick.contentX = Math.max(0, Math.min(Math.max(0, flick.contentWidth - flick.width), _resizeViewX));
            _resizeViewX = null;
        }
        if (_resizeViewY !== null) {
            flick.contentY = Math.max(0, Math.min(Math.max(0, flick.contentHeight - flick.height), _resizeViewY));
            _resizeViewY = null;
        }
        const tx = _resizeScrollX, ty = _resizeScrollY;
        _resizeScrollX = NaN; _resizeScrollY = NaN;
        if (!isNaN(tx) || !isNaN(ty)) canvasScroll.scrollTo(tx, ty, true);
    }
    // Drag anchors share workspace coordinates with the moving world origin.
    function shiftDragAnchors(dx, dy) {
        if (dragSession.active) {
            dragSession.startX += dx; dragSession.startY += dy;
            dragSession.originX += dx; dragSession.originY += dy;
        }
        if (dragSession.snapValid) {
            dragSession.snapX += dx; dragSession.snapY += dy;
        }
        if (valueDrag.active) {
            valueDrag.startX += dx; valueDrag.startY += dy;
            valueDrag.originX += dx; valueDrag.originY += dy;
        }
        if (valueDrag.settling) {
            valueDrag.settleX += dx; valueDrag.settleY += dy;
        }
    }
    onWorldWChanged: {
        if (_prevWorldW > 0 && flick) {
            shiftDragAnchors((worldW - _prevWorldW) / 2, 0);
            pauseResizeScroll();
            _resizeScrollX += (worldW - _prevWorldW) * root.zoom / 2;
            root._zoomAnchorX += (worldW - _prevWorldW) / 2;
            _resizeViewX = (_resizeViewX ?? flick.contentX) + (worldW - _prevWorldW) * root.zoom / 2;
            Qt.callLater(root.applyWorldResize);
        }
        _prevWorldW = worldW;
    }
    onWorldHChanged: {
        if (_prevWorldH > 0 && flick) {
            shiftDragAnchors(0, (worldH - _prevWorldH) / 2);
            pauseResizeScroll();
            _resizeScrollY += (worldH - _prevWorldH) * root.zoom / 2;
            root._zoomAnchorY += (worldH - _prevWorldH) / 2;
            _resizeViewY = (_resizeViewY ?? flick.contentY) + (worldH - _prevWorldH) * root.zoom / 2;
            Qt.callLater(root.applyWorldResize);
        }
        _prevWorldH = worldH;
    }
    property bool _centered: false
    // Zooming keeps one workspace point (the anchor) under one viewport point,
    // also on every frame of a glide, so the view never drifts while it scales.
    // A glide can also carry the anchor to another viewport point (resetView).
    property real _zoomAnchorX: 0
    property real _zoomAnchorY: 0
    property real _zoomFromViewX: 0
    property real _zoomFromViewY: 0
    property real _zoomViewX: 0
    property real _zoomViewY: 0
    property real _zoomFrom: 1
    property real _zoomTarget: 1
    property real _zoomProgress: 1
    NumberAnimation {
        id: zoomGlide; target: root; property: "_zoomProgress"; from: 0; to: 1; duration: Motion.scroll; easing.type: Easing.OutCubic
    }
    on_ZoomProgressChanged: if (zoomGlide.running) _applyZoom(_zoomProgress)
    function _applyZoom(t) {
        const z = root._zoomFrom + (root._zoomTarget - root._zoomFrom) * t;
        const vx = root._zoomFromViewX + (root._zoomViewX - root._zoomFromViewX) * t;
        const vy = root._zoomFromViewY + (root._zoomViewY - root._zoomFromViewY) * t;
        root.zoom = z;
        flick.contentX = Math.max(0, Math.min(Math.max(0, flick.contentWidth - flick.width), root._zoomAnchorX * z - vx));
        flick.contentY = Math.max(0, Math.min(Math.max(0, flick.contentHeight - flick.height), root._zoomAnchorY * z - vy));
    }
    // Zooms to newZoom while moving workspace point (ax, ay) to viewport
    // point (vx, vy). With smooth, it glides there, over ms if given.
    function _zoomTo(ax, ay, vx, vy, newZoom, smooth, ms) {
        zoomGlide.stop();
        canvasScroll.stop();
        root._zoomAnchorX = ax;
        root._zoomAnchorY = ay;
        root._zoomFromViewX = ax * root.zoom - flick.contentX;
        root._zoomFromViewY = ay * root.zoom - flick.contentY;
        root._zoomViewX = vx;
        root._zoomViewY = vy;
        root._zoomFrom = root.zoom;
        root._zoomTarget = newZoom;
        if (!smooth || !Motion.enabled) { _applyZoom(1); return; }
        zoomGlide.duration = ms !== undefined ? Motion.dur(ms) : Motion.scroll;
        root._zoomProgress = 0;
        zoomGlide.start();
    }
    // Zoom around a viewport point (vx, vy in Flickable viewport pixels) so
    // that point stays visually still. With smooth, the zoom glides there
    // (over ms if given).
    function zoomAt(vx, vy, newZoom, smooth, ms) {
        newZoom = Math.max(0.5, Math.min(1.8, newZoom));
        if (!isFinite(vx) || !isFinite(vy)) return;
        if (newZoom === (zoomGlide.running ? root._zoomTarget : root.zoom)) return;
        _zoomTo((flick.contentX + vx) / root.zoom, (flick.contentY + vy) / root.zoom, vx, vy, newZoom, smooth, ms);
    }
    // Scales the zoom by factor around a viewport point. Steps that land
    // mid-glide build on the glide's target, like wheel scrolling does.
    function zoomBy(vx, vy, factor, smooth, ms) {
        zoomAt(vx, vy, (zoomGlide.running ? root._zoomTarget : root.zoom) * factor, smooth, ms);
    }
    // Touchpad zoom (pinch, Ctrl+two-finger scroll) arrives as a stream of
    // small steps; a short glide per step smooths it without trailing behind.
    readonly property int _touchpadZoomMs: 80
    // Zoom around the middle of the view, by step (the zoom buttons).
    function stepZoom(step) {
        zoomAt(flick.width / 2, flick.height / 2, (zoomGlide.running ? root._zoomTarget : root.zoom) + step, true);
    }
    function setZoom(newZoom) { zoomAt(flick.width / 2, flick.height / 2, newZoom); }
    function centerOnOrigin() {
        zoomGlide.stop();
        canvasScroll.stop();
        flick.contentX = Math.max(0, (flick.contentWidth - flick.width) / 2);
        flick.contentY = Math.max(0, (flick.contentHeight - flick.height) / 2);
    }
    function tryInitialCenter() {
        if (_centered || flick.width <= 0 || flick.height <= 0) return;
        _centered = true;
        centerOnOrigin();
    }
    // Glides back to 100% with the workspace origin in the middle of the view.
    function resetView() {
        _zoomTo(root.worldW / 2, root.worldH / 2, flick.width / 2, flick.height / 2, 1, true);
    }
    function ensureFieldVisible(item) {
        if (!Theme.touchMode || !item) return;
        let ancestor = item;
        while (ancestor && ancestor !== workspace) ancestor = ancestor.parent;
        if (!ancestor) return;
        const p = item.mapToItem(flick, 0, 0);
        const bottom = p.y + item.height * root.zoom;
        const right = p.x + item.width * root.zoom;
        const dy = p.y < 16 ? p.y - 16 : Math.max(0, bottom - flick.height + 16);
        const dx = p.x < 16 ? p.x - 16 : Math.max(0, right - flick.width + 16);
        flick.contentX = Math.max(0, Math.min(Math.max(0, flick.contentWidth-flick.width), flick.contentX + dx));
        flick.contentY = Math.max(0, Math.min(Math.max(0, flick.contentHeight-flick.height), flick.contentY + dy));
    }
    Connections {
        target: root.Window.window
        function onActiveFocusItemChanged() { root.ensureFieldVisible(root.Window.window.activeFocusItem); }
    }
    onHeightChanged: Qt.callLater(() => { if (root.Window.window) root.ensureFieldVisible(root.Window.window.activeFocusItem); })
    function findBlockItem(item, id) {
        const kids = item.children || [];
        for (let i = 0; i < kids.length; ++i) {
            const k = kids[i];
            if (k.instruction && k.paletteMode === false && k.instruction.id === id) return k;
            const deep = findBlockItem(k, id);
            if (deep) return deep;
        }
        return null;
    }
    function commentAnchor(item) {
        if (!item) return Qt.point(0, 0);
        // Track ancestor moves and drag transforms, including nested blocks.
        let node = item;
        let x = item.width / 2;
        let y = Math.min(item.height, item.headHeight) / 2;
        while (node && node !== workspace) {
            x += node.x;
            y += node.y;
            if (node.dragOffsetX !== undefined) {
                x += node.dragOffsetX;
                y += node.dragOffsetY + node.displayedSnapShift;
            }
            node = node.parent;
        }
        return Qt.point(x, y);
    }
    // Scrolls the block with this instruction id into view and outlines it.
    function revealInstruction(id) {
        const item = findBlockItem(workspace, id);
        if (!item) return false;
        const p = item.mapToItem(workspace, 0, 0);
        const cx = (p.x + Math.min(item.width, flick.width / root.zoom) / 2) * root.zoom;
        const cy = (p.y + Math.min(item.height, 60) / 2) * root.zoom;
        zoomGlide.stop();
        canvasScroll.scrollTo(cx - flick.width / 2, cy - flick.height / 2, true);
        BlockRegistry.flash(id);
        return true;
    }

    // Drag session for blocks picked up on the canvas. Blocks read this to follow the pointer.
    // After a drop we keep a "settle" offset so the dragged blocks stay at
    // the drop point until the daemon round trip confirms the move. Without
    // this the transform snaps back to 0 on drop (visible revert) and the
    // block only teleports to its new position once the new state arrives.
    // snap* is the webapp-style attach preview: the nearest legal insertion
    // boundary within SNAP_THRESHOLD px, computed purely from the data model
    // (strand x/y + deterministic block heights, never live layout, so the
    // preview's own gap/shift can't feed back into the next measurement and
    // oscillate). Blocks shift/grow via transforms + effective mouth heights
    // to open a gap; the translucent snapPreview rect sits in that gap.
    // The same snap state doubles for palette drags (only one drag runs at a
    // time): updatePaletteSnap writes it while dragSession.active is false.
    QtObject {
        id: dragSession
        readonly property bool externalPreview: root.externalDragPreview
        property bool active: false
        property string strandId: ""
        property var path: []
        property int tailCount: 1
        property real startX: 0    // pointer, workspace coordinates, at pick-up
        property real startY: 0
        property real originX: 0   // dragged block top-left, workspace coordinates, at pick-up
        property real originY: 0
        property real dx: 0
        property real dy: 0
        property real sceneX: 0    // pointer, scene coordinates
        property real sceneY: 0
        property real draggedWidth: 200
        property real draggedHeight: 58
        // Silhouette of the dragged first block, so the preview is drawn
        // with the same tabs/notches/C-shape as a real block.
        property string draggedShape: "stack"
        property var draggedMouths: []
        property var draggedFlat: []
        property bool draggedIsCap: false
        // Attach preview (canvas drags and palette drags share it).
        // snapHeight is the preview block's own height; snapShiftAmt is how
        // far siblings after it move (height minus the 8px tab overlap that
        // interlocks stacked blocks); snapGrowAmt is how much a target wrap
        // mouth grows (same overlap logic, plus the empty-mouth fixed size).
        property var snapTargetY: null
        property bool snapValid: false
        property string snapTargetId: ""
        property var snapPath: []
        property real snapX: 0
        property real snapY: 0
        property real snapWidth: 200
        property real snapHeight: 58
        property real snapShiftAmt: 50
        property real snapGrowAmt: 50
        // Publish animation inputs together so a new mouth never uses the old gap size.
        property var snapLayout: null
        // Source collapse: the mouth the dragged tail is being pulled out of
        // renders as if the tail were already gone (the webapp splits the
        // strand on pickup; here the model only changes on drop). Shrink is
        // the mouth-height difference full-minus-prefix, computed from the
        // data model in beginDrag; cleared when the drag ends.
        property string sourceStrandId: ""
        property var sourceBasePath: []
        property real sourceShrink: 0
        // Set on drop, cleared once the new strands confirm the move (or on timeout).
        // An attach keeps the snap gap open while settling; a trash hides the tail.
        property bool settling: false
        property bool applyingDocument: false
        property bool settleHidden: false
        property string settleStrandId: ""
        property var settlePath: []
        property real settleDx: 0
        property real settleDy: 0
    }
    // A host overlay can draw active drags outside the clipped canvas.
    property bool externalDragPreview: false
    readonly property bool dragging: dragSession.active || valueDrag.active
    // Read access to the value-drag session (active/target/targetValid).
    // Blocks already receive dragSession the same way via dragState.
    readonly property alias valueDragState: valueDrag
    readonly property real dragSceneX: dragSession.active ? dragSession.sceneX : valueDrag.sceneX
    readonly property real dragSceneY: dragSession.active ? dragSession.sceneY : valueDrag.sceneY
    // Palette live preview reads these (same object as the canvas snap above).
    // What an overlay needs to redraw the dragged item outside this clipped canvas (scene coords).
    readonly property bool dragIsValue: valueDrag.active
    readonly property real dragGhostX: dragSession.active ? dragSession.sceneX - (dragSession.startX - dragSession.originX) * zoom : valueDrag.sceneX - (valueDrag.startX - valueDrag.originX) * zoom
    readonly property real dragGhostY: dragSession.active ? dragSession.sceneY - (dragSession.startY - dragSession.originY) * zoom : valueDrag.sceneY - (valueDrag.startY - valueDrag.originY) * zoom
    readonly property string dragValueOrigin: valueDrag.sourceOrigin
    readonly property var dragValue: valueDrag.sourceValue
    readonly property var dragInstructions: {
        if (!dragSession.active) return [];
        const strand = strandById(dragSession.strandId), p = dragSession.path || [];
        if (!strand || !p.length) return [];
        const list = listFor(strand, copyPath(p.slice(0, p.length - 1)));
        const idx = p[p.length - 1].index;
        return (list || []).slice(idx, idx + dragSession.tailCount);
    }
    readonly property bool paletteSnapValid: dragSession.snapValid
    readonly property string paletteSnapTargetId: dragSession.snapTargetId
    readonly property var paletteSnapPath: dragSession.snapPath
    readonly property var paletteSnapTargetY: dragSession.snapTargetY
    // ---- value (operator) drag session ----
    // Separate from the block/strand drag above: values can only land in
    // value slots (fields) or as new floating blocks, never in a strand.
    QtObject {
        id: valueDrag
        property bool active: false
        property var sourceLocation: null
        property var sourceValue: null
        property string sourceOrigin: ""
        property real startX: 0
        property real startY: 0
        property real originX: 0
        property real originY: 0
        property real dx: 0
        property real dy: 0
        property real sceneX: 0
        property real sceneY: 0
        property real draggedWidth: 120
        property real draggedHeight: 32
        property var target: null
        property bool targetValid: false
        property var highlightItem: null
        // After a drop the ghost stays at the landing spot (or hidden, for a
        // trash) and the source stays blank until the new state arrives.
        property bool settling: false
        property bool settleHidden: false
        property real settleX: 0
        property real settleY: 0
        property var settleSource: null
    }
    property string _lastDrop: ""
    // Highlight set by the sidebar while a fresh palette value hovers the
    // canvas (palette drags are owned by the editor, not by valueDrag).
    property var paletteValueTarget: null
    property var paletteHighlightItem: null
    function clearPaletteHighlight() {
        if (paletteHighlightItem && paletteHighlightItem.dropHighlighted !== undefined) paletteHighlightItem.dropHighlighted = false;
        paletteHighlightItem = null; paletteValueTarget = null;
    }
    function setPaletteHighlight(item, loc) {
        if (paletteHighlightItem && paletteHighlightItem !== item && paletteHighlightItem.dropHighlighted !== undefined) paletteHighlightItem.dropHighlighted = false;
        paletteHighlightItem = item;
        if (item && item.dropHighlighted !== undefined) item.dropHighlighted = true;
        paletteValueTarget = loc ? JSON.parse(JSON.stringify(loc)) : null;
    }
    // Returns the value-slot location under a scene point (for sidebar
    // palette drags), highlighting it. Pass null when the pointer leaves.
    // Targeting is proximity-based, not point-based: the nearest slot rect
    // to the dragged ghost rect within valueDropRadius wins (see
    // resolveValueSlot). The optional ghost scene rect (g-numbers) lets the
    // caller pass the dragged block's footprint; without it the pointer
    // alone is used as a zero-size ghost.
    function updatePaletteValueTarget(sx, sy) {
        if (sx === null || sx === undefined || sy === null || sy === undefined) { clearPaletteHighlight(); return null; }
        return updatePaletteValueTargetRect(sx, sy, sx, sy, sx, sy);
    }
    function updatePaletteValueTargetRect(gsx, gsy, gex, gey, sx, sy) {
        if (sx === null || sx === undefined || sy === null || sy === undefined) { clearPaletteHighlight(); return null; }
        if (!root.contains(root.mapFromItem(null, sx, sy))) { clearPaletteHighlight(); return null; }
        const tl = workspace.mapFromItem(null, gsx, gsy);
        const br = workspace.mapFromItem(null, gex, gey);
        const gx = Math.min(tl.x, br.x), gy = Math.min(tl.y, br.y);
        const gw = Math.abs(br.x - tl.x), gh = Math.abs(br.y - tl.y);
        const wp = workspace.mapFromItem(null, sx, sy);
        const found = resolveValueSlot(gx, gy, gw, gh, wp.x, wp.y);
        if (!found) { clearPaletteHighlight(); return null; }
        const loc = JSON.parse(JSON.stringify(found.loc));
        setPaletteHighlight(found.item, loc);
        return loc;
    }
    function branchContainers(node, out) {
        if (!node || node.visible === false) return;
        if (node.isReporterBranch) {
            const p = node.mapToItem(workspace, 0, 0);
            const list = node.branchList;
            out.push({ strandId: node.branchStrandId, basePath: [], x: p.x, y: p.y,
                list: list, heights: list.map(ins => blockHeight(ins)), owner: node.branchSource });
        }
        for (const child of node.children || []) branchContainers(child, out);
    }
    function strandById(id) {
        if (!id) return null;
        for (const strand of root.strands || []) if (strand.id === id) return strand;
        // Only reporter branch addresses can resolve outside the strand list.
        if (!id.startsWith("@reporter-branch:")) return null;
        const branches = [];
        branchContainers(workspace, branches);
        const branch = branches.find(b => b.strandId === id);
        return branch ? { id: id, x: branch.x - root.originOffsetX, y: branch.y - root.originOffsetY, instructions: branch.list } : null;
    }
    function selectInsertion(strandId, path) {
        const next = copyPath(path);
        if (!next.length) return;
        next[next.length - 1].index++;
        _insertionDocument = JSON.stringify(root.strands);
        const instruction = instructionAt(strandById(strandId), path);
        insertionPoint = { strandId: strandId, path: next, blockId: instruction ? instruction.id : "" };
    }
    function wireBranchInstruction(item) {
        item.variables = Qt.binding(() => root.variables);
        item.lists = Qt.binding(() => root.lists);
        item.keyCapture = Qt.binding(() => root.keyCapture);
        item.locked = Qt.binding(() => root.locked);
        item.dragState = Qt.binding(() => dragSession);
        item.removeRequested.connect(root.instructionRemoved);
        item.duplicateRequested.connect(root.instructionDuplicated);
        item.commentRequested.connect(root.commentForInstructionRequested);
        item.instructionEdited.connect(root.instructionEdited);
        item.valueEdited.connect(root.valueEdited);
        item.valueDragBegan.connect(root.beginValueDrag);
        item.valueDragMoved.connect(root.moveValueDrag);
        item.valueDragEnded.connect(root.endValueDrag);
        item.valueDragCanceled.connect(root.cancelValueDrag);
        item.runBranchRequested.connect(root.runBranchRequested);
        item.keyCaptureRequested.connect(root.keyCaptureRequested);
        item.appPickerRequested.connect(root.appPickerRequested);
        item.detailsRequested.connect(root.detailsRequested);
        item.menuActionRequested.connect(root.blockMenuAction);
        item.pieceActionRequested.connect(root.pieceActionRequested);
        item.paletteDragStarted.connect(root.paletteDragStarted);
        item.paletteDragMoved.connect(root.paletteDragMoved);
        item.paletteDragEnded.connect(root.paletteDragEnded);
        item.paletteDragCanceled.connect(root.paletteDragCanceled);
        item.insertionRequested.connect(root.selectInsertion);
        item.dragBegan.connect(root.beginDrag);
        item.dragMoved.connect(root.moveDrag);
        item.dragEnded.connect(root.endDrag);
        item.dragCanceled.connect(root.cancelDrag);
    }
    // ---- snap geometry (mirrors InstructionBlock row/wrap metrics) ----
    readonly property real snapThreshold: 36
    readonly property real snapSticky: 8
    function isWrapType(t) { return BlockRegistry.isWrap(t); }
    function isHeaderType(t) { return BlockRegistry.isHeader(t); }
    function isCapType(t) { return BlockRegistry.isCapType(t); }
    function isCapBlock(ins) { return BlockRegistry.isCap(ins, root.blockDefinitions); }
    function slotCountFor(ins) { return !ins ? 0 : ins.type === "BranchCallBlock" ? BlockRegistry.slotCount(ins) : Math.max(1, BlockRegistry.slotCount(ins)); }
    function bodyFor(ins, slot) { return BlockRegistry.body(ins, slot); }
    function blockHeight(ins) {
        if (!ins) return 58;
        const live = findBlockItem(workspace, ins.id);
        if (live) return live.implicitHeight;
        if (!isWrapType(ins.type)) return isCapBlock(ins) ? 50 : 58;
        const n = slotCountFor(ins);
        let total = 0;
        for (let k = 0; k < n; ++k) total += mouthHeightFor(bodyFor(ins, k));
        return 50 + total + Math.max(0, n - 1) * 34 + 26 + 8;
    }
    function contentHeightFor(list) {
        if (!list || !list.length) return 0;
        let sum = 0;
        for (let i = 0; i < list.length; ++i) sum += blockHeight(list[i]);
        return sum - 8 * (list.length - 1);
    }
    function mouthHeightFor(list) {
        const ch = contentHeightFor(list);
        if (ch === 0) return 26;
        const last = list[list.length - 1];
        const cap = last && isCapBlock(last);
        return Math.max(26, ch - (cap ? 0 : 8));
    }
    // Block silhouette of an instruction for the attach preview: same shape
    // kind, mouth heights and flat (cap) ends a real InstructionBlock would
    // report, so the preview draws identical tabs/notches/C-shape.
    function shapeInfoFor(ins) {
        if (!ins) return { shape: "stack", mouths: [], flat: [] };
        const t = ins.type || "";
        if (isHeaderType(t)) return { shape: "header", mouths: [], flat: [] };
        if (isCapBlock(ins)) return { shape: "cap", mouths: [], flat: [] };
        if (!isWrapType(t)) return { shape: "stack", mouths: [], flat: [] };
        const n = slotCountFor(ins);
        const mouths = [], flat = [];
        for (let k = 0; k < n; ++k) {
            const body = bodyFor(ins, k);
            mouths.push(mouthHeightFor(body));
            flat.push(body.length > 0 && isCapBlock(body[body.length - 1]));
        }
        return { shape: "wrap", mouths: mouths, flat: flat };
    }
    function listFor(strand, basePath) {
        let list = (strand && strand.instructions) || [];
        const base = basePath || [];
        for (let i = 0; i < base.length; ++i) {
            const st = base[i] || {};
            const blk = list[st.index];
            if (!blk) return [];
            list = bodyFor(blk, st.slot || 0);
        }
        return list || [];
    }
    function instructionAt(strand, path) {
        if (!strand || !path || !path.length) return null;
        let list = strand.instructions || [];
        for (let i = 0; i < path.length; ++i) {
            const step = path[i];
            if (!list || step.index < 0 || step.index >= list.length) return null;
            const block = list[step.index];
            if (i === path.length - 1) return block;
            list = bodyFor(block, step.slot || 0);
        }
        return null;
    }
    function copyPath(p) {
        if (!p) return [];
        const out = [];
        for (let i = 0; i < p.length; ++i) {
            const s = p[i] || {};
            if (s.slot === undefined || s.slot === null) out.push({ index: s.index });
            else out.push({ index: s.index, slot: s.slot });
        }
        return out;
    }
    function stepsEqual(a, b) {
        if (a.index !== b.index) return false;
        const sa = (a.slot === undefined || a.slot === null) ? null : a.slot;
        const sb = (b.slot === undefined || b.slot === null) ? null : b.slot;
        return sa === sb;
    }
    function baseEqual(a, b) {
        const aa = a || [], bb = b || [];
        if (aa.length !== bb.length) return false;
        for (let i = 0; i < aa.length; ++i) if (!stepsEqual(aa[i], bb[i])) return false;
        return true;
    }
    function collectContainers() {
        const out = [];
        const strands = root.strands || [];
        for (let s = 0; s < strands.length; ++s) {
            const st = strands[s];
            const topList = st.instructions || [];
            const topHeights = [];
            for (let i = 0; i < topList.length; ++i) topHeights.push(blockHeight(topList[i]));
            const lx = st.x + root.originOffsetX, ly = st.y + root.originOffsetY;
            out.push({ strandId: st.id, basePath: [], x: lx, y: ly, list: topList, heights: topHeights });
            let top = ly;
            for (let j = 0; j < topList.length; ++j) {
                collectChildren(st.id, topList[j], [{ index: j }], lx, top, 1, out);
                top += topHeights[j] - 8;
            }
        }
        const branches = [];
        branchContainers(workspace, branches);
        for (const branch of branches) {
            out.push(branch);
            let y = branch.y;
            for (let i = 0; i < branch.list.length; ++i) {
                collectChildren(branch.strandId, branch.list[i], [{ index: i }], branch.x, y, 1, out);
                y += branch.heights[i] - 8;
            }
        }
        return out;
    }
    function collectChildren(strandId, ins, pathToBlock, blockX, blockY, depth, out) {
        if (!ins || !isWrapType(ins.type)) return;
        const n = slotCountFor(ins);
        const mouthHs = [];
        for (let k = 0; k < n; ++k) mouthHs.push(mouthHeightFor(bodyFor(ins, k)));
        const live = findBlockItem(workspace, ins.id);
        let mTop = live ? live.headHeight : 50;
        for (let k = 0; k < n; ++k) {
            const cX = blockX + 20, cY = blockY + mTop;
            const body = bodyFor(ins, k);
            const base = copyPath(pathToBlock);
            base[base.length - 1].slot = k;
            const heights = [];
            for (let i = 0; i < body.length; ++i) heights.push(blockHeight(body[i]));
            out.push({ strandId: strandId, basePath: base, x: cX, y: cY, list: body, heights: heights });
            let childTop = cY;
            for (let ci = 0; ci < body.length; ++ci) {
                const childPath = copyPath(base);
                childPath.push({ index: ci });
                collectChildren(strandId, body[ci], childPath, cX, childTop, depth + 1, out);
                childTop += heights[ci] - 8;
            }
            mTop = live && k + 1 < n ? live.mouthTop(k + 1) : mTop + mouthHs[k] + 34;
        }
    }
    function isBaseInsideTail(containerBase, srcParentBase, srcIdx) {
        const cb = containerBase || [], pb = srcParentBase || [];
        if (baseEqual(cb, pb)) return false;
        if (cb.length <= pb.length) return false;
        for (let i = 0; i < pb.length; ++i) if (!stepsEqual(cb[i], pb[i])) return false;
        const next = cb[pb.length];
        return next.index >= srcIdx;
    }
    // Stacked blocks interlock by the 8px connector tab (Column spacing
    // -8): a dropped block's top sits exactly on the old row top it replaces
    // (or 8px above the old bottom edge at the end / on the mouth origin when
    // empty), and everything after it moves by height-8, not height.
    function findSnap(ghostLeft, ghostTop, sourceId, sourcePath, isWhole, prevSnap, stackHeight, stackEndsCap) {
        const containers = collectContainers();
        let best = null, bestEff = 1e9;
        const srcParentBase = (sourceId && sourcePath && sourcePath.length) ? sourcePath.slice(0, sourcePath.length - 1) : null;
        const srcIdx = (sourceId && sourcePath && sourcePath.length) ? sourcePath[sourcePath.length - 1].index : -1;
        for (let ci = 0; ci < containers.length; ++ci) {
            const c = containers[ci];
            if (sourceId && c.owner) {
                let own = false;
                for (let owner = c.owner; owner; owner = owner.parent) {
                    if (owner.strandId === sourceId && owner.path && sourcePath
                        && isBaseInsideTail(owner.path, srcParentBase || [], srcIdx)) { own = true; break; }
                }
                if (own) continue;
            }
            if (sourceId && isWhole && c.strandId === sourceId) continue;
            let list = c.list, heights = c.heights;
            if (sourceId && !isWhole && c.strandId === sourceId) {
                if (baseEqual(c.basePath, srcParentBase)) {
                    list = list.slice(0, Math.max(0, srcIdx));
                    heights = heights.slice(0, Math.max(0, srcIdx));
                } else if (isBaseInsideTail(c.basePath, srcParentBase, srcIdx)) {
                    continue;
                }
            }
            if (Math.abs(ghostLeft - c.x) > root.snapThreshold) continue;
            const isTop = c.basePath.length === 0;
            const headIsHeader = isTop && list.length > 0 && isHeaderType(list[0].type);
            const bounds = [];
            if (!list.length) {
                bounds.push({ index: 0, y: c.y });
            } else {
                let y = c.y;
                for (let i = 0; i < list.length; ++i) { bounds.push({ index: i, y: y }); y += heights[i] - 8; }
                bounds.push({ index: list.length, y: y });
            }
            for (let bi = 0; bi < bounds.length; ++bi) {
                const b = bounds[bi], idx = b.index;
                if (idx === 0 && headIsHeader) continue;
                if (idx > 0) {
                    const above = list[idx - 1];
                    if (!above || isCapBlock(above)) continue;
                }
                if (stackEndsCap && idx < list.length) continue;
                let dist = Math.abs(ghostTop - b.y);
                // Match the bottom connector only at the start of a body or stack.
                // Interior insertions follow the dragged top to avoid jumping ahead.
                // Bottom matches stay within half the mouth's 20px indentation.
                const alignedBottom = (isTop && !c.owner) || Math.abs(ghostLeft - c.x) < 10;
                const bottomDist = alignedBottom && idx === 0 && list.length > 0 && !stackEndsCap && stackHeight > 0
                    ? Math.abs(ghostTop + stackHeight - 8 - b.y) : Infinity;
                const prepend = isTop && !c.owner && idx === 0 && list.length > 0 && stackHeight > 0;
                // Prepending keeps the target fixed, so only its top connector
                // may meet the dragged stack's bottom connector.
                dist = prepend ? bottomDist : Math.min(dist, bottomDist);
                if (dist > root.snapThreshold) continue;
                const path = copyPath(c.basePath);
                path.push({ index: idx });
                const isCur = prevSnap && prevSnap.targetId === c.strandId && baseEqual(prevSnap.path, path);
                const distance = Math.hypot(ghostLeft - c.x, dist);
                const eff = isCur ? Math.max(0, distance - root.snapSticky) : distance;
                if (best === null || eff < bestEff) {
                    const shift = prepend ? stackHeight - 8 : 0;
                    best = { targetId: c.strandId, path: path, x: c.x, y: b.y - shift,
                        targetY: prepend ? Math.round(c.y - root.originOffsetY - shift) : null,
                        empty: list.length === 0, atEnd: idx === list.length };
                    bestEff = eff;
                }
            }
        }
        return best;
    }
    function applySnap(best, w, h, droppedIsCap, stackHeight) {
        if (!best) { clearSnap(); return; }
        dragSession.snapTargetId = best.targetId;
        dragSession.snapTargetY = best.targetY === undefined ? null : best.targetY;
        dragSession.snapPath = best.path;
        dragSession.snapX = best.x;
        dragSession.snapY = best.y;
        dragSession.snapWidth = w;
        dragSession.snapHeight = h;
        const gapHeight = stackHeight === undefined ? h : stackHeight;
        // Prepending keeps the target fixed; other insertions open a gap.
        // Empty mouths start at 26px, and cap blocks have no bottom overlap.
        dragSession.snapShiftAmt = dragSession.snapTargetY !== null ? 0 : Math.max(0, gapHeight - 8);
        if (best.empty) dragSession.snapGrowAmt = Math.max(0, Math.max(26, gapHeight - (droppedIsCap ? 0 : 8)) - 26);
        else if (best.atEnd && droppedIsCap) dragSession.snapGrowAmt = Math.max(0, gapHeight);
        else dragSession.snapGrowAmt = Math.max(0, gapHeight - 8);
        dragSession.snapValid = true;
        dragSession.snapLayout = {
            snapValid: true, snapTargetId: best.targetId, snapPath: best.path,
            snapShiftAmt: dragSession.snapShiftAmt, snapGrowAmt: dragSession.snapGrowAmt, snapWidth: w
        };
    }
    function clearSnap() {
        dragSession.snapLayout = null;
        dragSession.snapValid = false;
        dragSession.snapTargetY = null;
        dragSession.snapTargetId = "";
        dragSession.snapPath = [];
    }
    function prevSnapObj() {
        if (!dragSession.snapValid) return null;
        return { targetId: dragSession.snapTargetId, path: dragSession.snapPath };
    }
    // Stable strand model: a plain `Repeater { model: root.strands }` over a
    // JS array destroys and recreates *every* strand delegate whenever the
    // array identity changes (which is every daemon state push, since
    // appState is re-parsed from JSON). That reads as all blocks flickering
    // at once. Syncing into a ListModel keyed by strand id instead keeps
    // untouched strands' delegates (and their nested blocks) alive: moves
    // only touch x/y roles, content changes only touch that strand.
    // NOTE: the instruction list is stored as a JSON *string* role. Storing
    // the raw JS array in a ListModel role does not round-trip: reads come
    // back as non-array sequences (no .length, Array.isArray false) and
    // setProperty with an array wipes the role, emptying every strand.
    ListModel { id: strandModel }
    function syncStrands() {
        const list = root.strands || [];
        for (let i = strandModel.count - 1; i >= 0; --i) {
            const sid = strandModel.get(i).sid;
            let alive = false;
            for (let j = 0; j < list.length; ++j) if (list[j].id === sid) { alive = true; break; }
            if (!alive) strandModel.remove(i);
        }
        for (let j = 0; j < list.length; ++j) {
            const s = list[j];
            const payload = JSON.stringify(s.instructions || []);
            let at = -1;
            for (let i = 0; i < strandModel.count; ++i) if (strandModel.get(i).sid === s.id) { at = i; break; }
            if (at === -1) {
                strandModel.insert(j, { sid: s.id, sx: s.x, sy: s.y, payload: payload });
            } else {
                if (at !== j) { strandModel.move(at, j, 1); at = j; }
                if (strandModel.get(at).sx !== s.x) strandModel.setProperty(at, "sx", s.x);
                if (strandModel.get(at).sy !== s.y) strandModel.setProperty(at, "sy", s.y);
                if (strandModel.get(at).payload !== payload)
                    strandModel.setProperty(at, "payload", payload);
            }
        }
        while (strandModel.count > list.length) strandModel.remove(strandModel.count - 1);
    }
    function beginDrag(strandId, path, tailCount, sx, sy, ox, oy, bw, bh) {
        cancelValueDrag(); clearPaletteHighlight(); endSettle();
        const p = workspace.mapFromItem(null, sx, sy);
        dragSession.strandId = strandId; dragSession.path = path; dragSession.tailCount = tailCount;
        dragSession.startX = p.x; dragSession.startY = p.y; dragSession.originX = p.x - ox; dragSession.originY = p.y - oy;
        dragSession.sceneX = sx; dragSession.sceneY = sy;
        dragSession.dx = 0; dragSession.dy = 0; dragSession.active = true;
        dragSession.draggedWidth = bw || 200; dragSession.draggedHeight = bh || 58;
        const si = shapeInfoFor(instructionAt(strandById(strandId), path));
        dragSession.draggedShape = si.shape; dragSession.draggedMouths = si.mouths; dragSession.draggedFlat = si.flat;
        const di = instructionAt(strandById(strandId), path);
        dragSession.draggedIsCap = !!(di && isCapBlock(di));
        // Shrink the home mouth for the drag's duration (see sourceShrink).
        // Top-level tails have no mouth shell, so only nested paths shrink.
        dragSession.sourceStrandId = strandId;
        dragSession.sourceBasePath = copyPath(path.slice(0, path.length - 1));
        let shrink = 0;
        if (path.length > 1) {
            const strand = strandById(strandId);
            if (strand) {
                const full = listFor(strand, dragSession.sourceBasePath);
                const idx = path[path.length - 1].index;
                shrink = mouthHeightFor(full) - mouthHeightFor(full.slice(0, Math.max(0, idx)));
            }
        }
        dragSession.sourceShrink = Math.max(0, shrink);
        clearSnap();
    }
    function moveDrag(sx, sy) {
        if (!dragSession.active) return;
        const p = workspace.mapFromItem(null, sx, sy);
        dragSession.dx = p.x - dragSession.startX; dragSession.dy = p.y - dragSession.startY;
        dragSession.sceneX = sx; dragSession.sceneY = sy;
        updateSnap();
    }
    function updateSnap() {
        if (!dragSession.active) return;
        const sourcePath = dragSession.path || [];
        if (!sourcePath.length) { clearSnap(); return; }
        const isWhole = sourcePath.length === 1 && sourcePath[0].index === 0;
        const srcStrand = strandById(dragSession.strandId);
        if (isWhole && srcStrand) {
            const first = (srcStrand.instructions || [])[0];
            if (first && isHeaderType(first.type)) { clearSnap(); return; }
        } else if (!isWhole && srcStrand) {
            const firstTail = instructionAt(srcStrand, sourcePath);
            if (firstTail && isHeaderType(firstTail.type)) { clearSnap(); return; }
        }
        const ghostLeft = dragSession.originX + dragSession.dx;
        const ghostTop = dragSession.originY + dragSession.dy;
        const tail = listFor(srcStrand, sourcePath.slice(0, -1)).slice(sourcePath[sourcePath.length - 1].index);
        const endsCap = tail.length > 0 && isCapBlock(tail[tail.length - 1]);
        const stackHeight = contentHeightFor(tail);
        const best = findSnap(ghostLeft, ghostTop, dragSession.strandId, sourcePath, isWhole, prevSnapObj(), stackHeight, endsCap);
        let stackWidth = dragSession.draggedWidth;
        for (const instruction of tail) {
            const item = findBlockItem(workspace, instruction.id);
            if (item) stackWidth = Math.max(stackWidth, item.implicitWidth);
        }
        applySnap(best, stackWidth, dragSession.draggedHeight, endsCap, stackHeight);
    }
    // Palette (new-block) drags share the same preview: a header prefab can
    // never snap into an existing strand, anything else uses the same
    // boundaries/rules as a canvas tail (excludeId none).
    function updatePaletteSnap(ghostLeft, ghostTop, instruction, w, h) {
        endSettle();
        if (!instruction || isHeaderType(instruction.type)) { clearSnap(); return; }
        const si = shapeInfoFor(instruction);
        dragSession.draggedShape = si.shape; dragSession.draggedMouths = si.mouths; dragSession.draggedFlat = si.flat;
        dragSession.draggedIsCap = isCapBlock(instruction);
        dragSession.draggedWidth = w || 200; dragSession.draggedHeight = blockHeight(instruction) || h || 58;
        const best = findSnap(ghostLeft + root.originOffsetX, ghostTop + root.originOffsetY, null, [], false, prevSnapObj(), dragSession.draggedHeight, dragSession.draggedIsCap);
        applySnap(best, dragSession.draggedWidth, dragSession.draggedHeight, dragSession.draggedIsCap);
    }
    function endDrag(sx, sy) {
        if (!dragSession.active) return;
        moveDrag(sx, sy);
        const strandId = dragSession.strandId, path = dragSession.path, tail = dragSession.tailCount;
        const dx = dragSession.dx, dy = dragSession.dy, ox = dragSession.originX, oy = dragSession.originY;
        const snapValid = dragSession.snapValid, snapTargetId = dragSession.snapTargetId, snapPath = copyPath(dragSession.snapPath || []);
        const isWhole = path.length === 1 && path[0].index === 0;
        dragSession.active = false;
        dragSession.settleStrandId = strandId;
        dragSession.settlePath = path;
        dragSession.settleHidden = false;
        root._lastDrop = "block";
        if (!root.contains(root.mapFromItem(null, sx, sy))) {
            // The host calls settleTrashed() if it actually deletes the tail.
            clearSnap(); dragSession.settling = false;
            root.blockDragOutside(strandId, path, tail, sx, sy);
            return;
        }
        if (snapValid && snapTargetId) {
            // Attach, just like the webapp: merge the whole strand, or move
            // the tail atomically (no split-then-merge round trip). Settle on
            // the preview spot, keeping the gap open, until the merge lands.
            dragSession.settleDx = dragSession.snapX - ox;
            dragSession.settleDy = dragSession.snapY - oy;
            dragSession.settling = true;
            armSettle();
            if (isWhole) root.strandsMerged(strandId, snapTargetId, snapPath, dragSession.snapTargetY);
            else root.tailMerged(strandId, path, snapTargetId, snapPath, dragSession.snapTargetY);
            return;
        }
        // Optimistic settle: keep rendering the dragged tail at the drop
        // point until the backend state confirms the move/split. Cleared in
        // onStrandsChanged or by settleTimer below.
        clearSnap();
        dragSession.settleDx = dx;
        dragSession.settleDy = dy;
        dragSession.settling = true;
        armSettle();
        const strand = strandById(strandId);
        if (isWhole && strand) root.strandMoved(strandId, Math.round(strand.x + dx), Math.round(strand.y + dy));
        else root.instructionSplit(strandId, path, Math.round(ox + dx - root.originOffsetX), Math.round(oy + dy - root.originOffsetY));
    }
    // Hides the block or value just dropped outside the canvas until its removal lands.
    function settleTrashed() {
        if (root._lastDrop === "value") {
            if (valueDrag.active || !valueDrag.settleSource) return;
            settleValue(valueDrag.settleSource, 0, 0, true);
            return;
        }
        if (dragSession.active) return;
        dragSession.settleHidden = true;
        dragSession.settling = true;
        armSettle();
    }
    // Keeps a palette drop's attach gap open until the new block lands.
    function settlePaletteDrop() {
        if (dragSession.active || !dragSession.snapValid) return;
        dragSession.settleStrandId = ""; dragSession.settlePath = [];
        dragSession.settling = true;
        armSettle();
    }
    // Scene point where a palette drop will land (value slot or attach gap), or null.
    function paletteDropScenePoint() {
        if (paletteHighlightItem) return paletteHighlightItem.mapToItem(null, 0, 0);
        if (dragSession.snapValid) return workspace.mapToItem(null, dragSession.snapX, dragSession.snapY);
        return null;
    }
    function settleValue(source, x, y, hidden) {
        valueDrag.settleX = x; valueDrag.settleY = y;
        valueDrag.settleHidden = hidden;
        valueDrag.settling = true;
        BlockRegistry.settlingValueKey = BlockRegistry.locationKey(source);
        armSettle();
    }
    function endSettle() {
        if (valueDrag.settling) {
            valueDrag.settling = false; valueDrag.settleHidden = false;
            BlockRegistry.settlingValueKey = "";
        }
        if (!dragSession.settling) return;
        dragSession.settling = false; dragSession.settleHidden = false;
        clearSnap();
    }
    function cancelDrag() {
        dragSession.active = false; dragSession.settling = false; dragSession.settleHidden = false; clearSnap();
        dragSession.sourceStrandId = ""; dragSession.sourceBasePath = []; dragSession.sourceShrink = 0;
    }
    // ---- value (operator) drag: hit-testing + session ----
    function valueLocationsEqual(a, b) {
        if (!a || !b) return false;
        if (a.kind !== b.kind) return false;
        if (a.kind === "Field") {
            if (a.strand_id !== b.strand_id || a.field_id !== b.field_id) return false;
            const ai = a.index || [], bi = b.index || [];
            if (ai.length !== bi.length) return false;
            for (let i = 0; i < ai.length; ++i) {
                if (ai[i].index !== bi[i].index) return false;
                const sa = (ai[i].slot === undefined || ai[i].slot === null) ? -1 : ai[i].slot;
                const sb = (bi[i].slot === undefined || bi[i].slot === null) ? -1 : bi[i].slot;
                if (sa !== sb) return false;
            }
        } else {
            if (a.floating_id !== b.floating_id) return false;
        }
        const ap = a.path || [], bp = b.path || [];
        if (ap.length !== bp.length) return false;
        for (let i = 0; i < ap.length; ++i) if (ap[i] !== bp[i]) return false;
        return true;
    }
    function isFloatingRootLocation(loc) { return !!loc && loc.kind === "Floating" && !(loc.path && loc.path.length); }
    function isValueDescendantOrSelf(target, source) {
        if (!target || !source || target.kind !== source.kind) return false;
        if (target.kind === "Field") {
            if (target.strand_id !== source.strand_id || target.field_id !== source.field_id) return false;
            const ai = target.index || [], bi = source.index || [];
            if (ai.length !== bi.length) return false;
            for (let i = 0; i < ai.length; ++i) {
                if (ai[i].index !== bi[i].index) return false;
                const sa = (ai[i].slot === undefined || ai[i].slot === null) ? -1 : ai[i].slot;
                const sb = (bi[i].slot === undefined || bi[i].slot === null) ? -1 : bi[i].slot;
                if (sa !== sb) return false;
            }
        } else if (target.floating_id !== source.floating_id) return false;
        const tp = target.path || [], sp = source.path || [];
        if (tp.length < sp.length) return false;
        for (let i = 0; i < sp.length; ++i) if (tp[i] !== sp[i]) return false;
        return true;
    }
    // ---- value (operator) drop targeting: proximity, not point hits ----
    // Slots are small and the dragged ghost covers the pointer, so a pure
    // childAt(x, y) point query misses whenever the cursor sits just off a
    // slot edge. Instead we measure the gap between the dragged ghost rect
    // and every slot rect in workspace coordinates and take the nearest one
    // inside valueDropRadius (same idea as the block snapThreshold above).
    // Tie-breaks reproduce the old exact-hover behavior: among equidistant
    // slots the one actually containing the pointer wins, then the smallest
    // (deepest nested), then the one whose center is nearest the ghost.
    readonly property real valueDropRadius: 12
    function workspaceRectOf(item) {
        try {
            const tl = item.mapToItem(workspace, 0, 0);
            const br = item.mapToItem(workspace, item.width, item.height);
            return { x: Math.min(tl.x, br.x), y: Math.min(tl.y, br.y), w: Math.abs(br.x - tl.x), h: Math.abs(br.y - tl.y) };
        } catch (e) { return null; }
    }
    function rectGap(ax, ay, aw, ah, bx, by, bw, bh) {
        const dx = Math.max(bx - (ax + aw), ax - (bx + bw), 0);
        const dy = Math.max(by - (ay + ah), ay - (by + bh), 0);
        return Math.hypot(dx, dy);
    }
    function collectValueSlots(node, out) {
        if (!node || node.visible === false) return;
        const kids = node.children;
        if (kids) for (let i = 0; i < kids.length; ++i) collectValueSlots(kids[i], out);
        if (node.isValueChip && node.valueLocation && !isFloatingRootLocation(node.valueLocation)) {
            const r = workspaceRectOf(node);
            if (r) out.push({ item: node, loc: node.valueLocation, rect: r });
        }
    }
    // gx/gy/gw/gh: dragged ghost rect in workspace coordinates (gw/gh 0 =
    // unknown: pointer used as a zero-size ghost). px/py: pointer workspace
    // point, used for tie-breaks. Returns { item, loc, rect } or null.
    function resolveValueSlot(gx, gy, gw, gh, px, py) {
        const slots = [];
        collectValueSlots(workspace, slots);
        const gcx = gx + gw / 2, gcy = gy + gh / 2;
        let best = null, bestGap = 1e9, bestInside = false, bestArea = 1e9, bestCenter = 1e9;
        for (let i = 0; i < slots.length; ++i) {
            const s = slots[i], r = s.rect;
            const gap = rectGap(gx, gy, gw, gh, r.x, r.y, r.w, r.h);
            if (gap > valueDropRadius) continue;
            const inside = px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h;
            const area = Math.max(1, r.w) * Math.max(1, r.h);
            const cd = Math.hypot(gcx - (r.x + r.w / 2), gcy - (r.y + r.h / 2));
            let take = false;
            if (!best || gap < bestGap - 1e-6) take = true;
            else if (Math.abs(gap - bestGap) <= 1e-6) {
                if (inside && !bestInside) take = true;
                else if (inside === bestInside) {
                    if (area < bestArea - 1e-6) take = true;
                    else if (Math.abs(area - bestArea) <= 1e-6 && cd < bestCenter) take = true;
                }
            }
            if (take) { best = s; bestGap = gap; bestInside = inside; bestArea = area; bestCenter = cd; }
        }
        return best;
    }
    function findValueChip(item, x, y) {
        let child = null;
        try { child = item.childAt(x, y); } catch (e) { child = null; }
        if (!child) return item.isValueChip ? item : null;
        const cp = item.mapToItem(child, x, y);
        const deep = findValueChip(child, cp.x, cp.y);
        if (deep) return deep;
        return item.isValueChip ? item : null;
    }
    // Deepest value slot under a scene point, or null. Floating roots are
    // never slots (only their nested inputs are). `exclude` marks the drag
    // source: dropping back onto it is a no-op, dropping into its own
    // descendant is invalid - both still return the location so the caller
    // can tell "cancel" apart from "open canvas".
    function findValueTarget(sx, sy, exclude) {
        if (!root.contains(root.mapFromItem(null, sx, sy))) return null;
        const wp = workspace.mapFromItem(null, sx, sy);
        const found = resolveValueSlot(wp.x, wp.y, 0, 0, wp.x, wp.y);
        if (!found) return null;
        return JSON.parse(JSON.stringify(found.loc));
    }
    function clearValueTarget() { valueDrag.target = null; valueDrag.targetValid = false; setValueHighlight(null); }
    function setValueHighlight(item) {
        if (valueDrag.highlightItem && valueDrag.highlightItem !== item && valueDrag.highlightItem.dropHighlighted !== undefined) valueDrag.highlightItem.dropHighlighted = false;
        valueDrag.highlightItem = item;
        if (item && item.dropHighlighted !== undefined) item.dropHighlighted = true;
    }
    function setPaletteValueTarget(loc) { paletteValueTarget = loc ? JSON.parse(JSON.stringify(loc)) : null; }
    // Fields use the definition heading their strand, including nested expressions.
    function branchColorForStrand(strandId) {
        const strand = strandById(strandId);
        const header = strand && (strand.instructions || [])[0];
        const definition = header && header.type === "BlockHeader"
            ? blockDefinitions.find(d => d.id === header.block_id) : null;
        return definition && definition.color ? definition.color : Theme.accent;
    }
    readonly property color dragBranchColor: branchColorForStrand(dragSession.strandId)
    function parameterOrigin(location) {
        if (!location) return "";
        if (location.kind === "Floating") {
            const floating = (root.floatingValues || []).find(v => v.id === location.floating_id);
            return floating ? floating.origin_block_id || "" : "";
        }
        const strand = strandById(location.strand_id);
        const header = strand && (strand.instructions || [])[0];
        return header && header.type === "BlockHeader" ? header.block_id : "";
    }
    function beginValueDrag(location, value, sx, sy, ox, oy, w, h) {
        if (root.locked) return;
        cancelDrag(); endSettle();
        const p = workspace.mapFromItem(null, sx, sy);
        valueDrag.sourceLocation = JSON.parse(JSON.stringify(location));
        valueDrag.sourceValue = JSON.parse(JSON.stringify(value));
        valueDrag.sourceOrigin = parameterOrigin(location);
        valueDrag.startX = p.x; valueDrag.startY = p.y;
        valueDrag.originX = p.x - ox; valueDrag.originY = p.y - oy;
        valueDrag.dx = 0; valueDrag.dy = 0;
        valueDrag.sceneX = sx; valueDrag.sceneY = sy;
        valueDrag.draggedWidth = w || 120; valueDrag.draggedHeight = h || 32;
        valueDrag.active = true;
        clearValueTarget();
        clearPaletteHighlight();
        updateValueTarget(sx, sy);
    }
    function updateValueTarget(sx, sy) {
        if (!valueDrag.active) return;
        valueDrag.sceneX = sx; valueDrag.sceneY = sy;
        const p = workspace.mapFromItem(null, sx, sy);
        valueDrag.dx = p.x - valueDrag.startX; valueDrag.dy = p.y - valueDrag.startY;
        if (!root.contains(root.mapFromItem(null, sx, sy))) { clearValueTarget(); return; }
        // Ghost rect in workspace coordinates (draggedWidth/Height are item
        // pixels, so normalize by zoom); the drop target is the nearest slot
        // to this rect, not the item exactly under the cursor.
        const z = Math.max(0.01, root.zoom);
        const gx = valueDrag.originX + valueDrag.dx, gy = valueDrag.originY + valueDrag.dy;
        const gw = (valueDrag.draggedWidth || 0) / z, gh = (valueDrag.draggedHeight || 0) / z;
        const found = resolveValueSlot(gx, gy, gw, gh, p.x, p.y);
        if (!found) { clearValueTarget(); return; }
        const t = JSON.parse(JSON.stringify(found.loc));
        // Dropping back onto the exact source slot, or into its own
        // descendant, is never a move - keep the highlight for the
        // exact-source no-op, clear it for a true self-nesting.
        if (valueLocationsEqual(t, valueDrag.sourceLocation)) { valueDrag.target = t; valueDrag.targetValid = true; setValueHighlight(found.item); return; }
        if (isValueDescendantOrSelf(t, valueDrag.sourceLocation)) { clearValueTarget(); return; }
        valueDrag.target = t; valueDrag.targetValid = true; setValueHighlight(found.item);
    }
    function moveValueDrag(sx, sy) { updateValueTarget(sx, sy); }
    function endValueDrag(sx, sy) {
        if (!valueDrag.active) return;
        updateValueTarget(sx, sy);
        const source = valueDrag.sourceLocation ? JSON.parse(JSON.stringify(valueDrag.sourceLocation)) : null;
        const value = valueDrag.sourceValue ? JSON.parse(JSON.stringify(valueDrag.sourceValue)) : null;
        const target = valueDrag.targetValid && valueDrag.target ? JSON.parse(JSON.stringify(valueDrag.target)) : null;
        const targetItem = valueDrag.highlightItem;
        const gx = valueDrag.originX + valueDrag.dx, gy = valueDrag.originY + valueDrag.dy;
        valueDrag.active = false;
        clearValueTarget();
        root._lastDrop = "value";
        valueDrag.settleSource = source;
        if (!source || !value) return;
        if (!root.contains(root.mapFromItem(null, sx, sy))) { root.valueDragOutside(source, value, sx, sy); return; }
        if (target) {
            if (valueLocationsEqual(target, source)) return; // no-op put-back
            const p = targetItem ? workspace.mapFromItem(targetItem, 0, 0) : Qt.point(gx, gy);
            settleValue(source, p.x, p.y, false);
            root.valueMoveRequested(source, target, 0, 0, "");
            return;
        }
        // Open canvas: lands where the ghost is. A whole floating block just
        // moves; anything pulled out of a field becomes a new floating block.
        const fx = Math.round(gx - root.originOffsetX), fy = Math.round(gy - root.originOffsetY);
        settleValue(source, gx, gy, false);
        if (source.kind === "Floating" && !(source.path && source.path.length)) root.floatingValueMoved(source.floating_id, fx, fy);
        else root.valueMoveRequested(source, null, fx, fy, valueDrag.sourceOrigin);
    }
    function cancelValueDrag() { valueDrag.active = false; clearValueTarget(); }
    // A settle ends once the document differs from the drop moment; other
    // state pushes (run log, debugger) reassign it unchanged and must not.
    property string _settleDoc: ""
    function docKey() { return JSON.stringify([root.strands, root.floatingValues]); }
    function armSettle() { _settleDoc = docKey(); settleTimer.restart(); }
    function settleOnDocChange() {
        if ((dragSession.settling || valueDrag.settling) && docKey() === _settleDoc) return;
        endSettle();
    }
    onFloatingValuesChanged: settleOnDocChange()
    Timer { id: settleTimer; interval: 1500; onTriggered: root.endSettle() }
    // The settling key lives in the shared registry; don't leave it naming a
    // location on whatever canvas comes next.
    Component.onDestruction: endSettle()
    onStrandsChanged: {
        if (_insertionDocument !== JSON.stringify(root.strands)) insertionPoint = null;
        // The document already includes the preview gap. Do not animate it again.
        dragSession.applyingDocument = true;
        try {
            syncStrands();
            settleOnDocChange();
            if (!dragSession.active && !dragSession.settling) {
                dragSession.sourceStrandId = ""; dragSession.sourceBasePath = []; dragSession.sourceShrink = 0;
            }
        } finally {
            dragSession.applyingDocument = false;
        }
    }
    Component.onCompleted: { syncStrands(); syncListEditors(); syncDictEditors(); tryInitialCenter(); }
    // Prefer free space in the view, then the closest free space around it.
    function findEmptyPosition(width, height) {
        applyWorldResize();
        const gap = 16 / root.zoom;
        const left = flick.contentX / root.zoom - root.originOffsetX + gap;
        const top = flick.contentY / root.zoom - root.originOffsetY + gap;
        const right = left + flick.width / root.zoom - 2 * gap;
        const bottom = top + flick.height / root.zoom - 2 * gap;
        const preferredX = Math.round((left + right - width) / 2);
        const preferredY = Math.round((top + bottom - height) / 2);
        const occupied = [];
        for (const item of workspace.children) {
            if (!item.sid && !(item.modelData && item.modelData.id)) continue;
            occupied.push({left: item.x - root.originOffsetX - gap,
                top: item.y - root.originOffsetY - gap,
                right: item.x - root.originOffsetX + item.width + gap,
                bottom: item.y - root.originOffsetY + item.height + gap});
        }
        const xs = [preferredX, Math.ceil(left), Math.floor(right - width)];
        const ys = [preferredY, Math.ceil(top), Math.floor(bottom - height)];
        for (const rect of occupied) {
            xs.push(Math.floor(rect.left - width), Math.ceil(rect.right));
            ys.push(Math.floor(rect.top - height), Math.ceil(rect.bottom));
        }
        let best = null, bestScore = Infinity, bestInView = false;
        for (const x of xs) for (const y of ys) {
            const inView = x >= left && y >= top && x + width <= right && y + height <= bottom;
            if (bestInView && !inView) continue;
            const score = (x - preferredX) ** 2 + (y - preferredY) ** 2;
            if (inView === bestInView && score >= bestScore) continue;
            if (occupied.some(r => x < r.right && x + width > r.left && y < r.bottom && y + height > r.top)) continue;
            best = {x: x, y: y}; bestScore = score; bestInView = inView;
        }
        return best || {x: preferredX, y: preferredY};
    }
    // World coordinates for a scene point, or null when it is outside the canvas.
    function workspacePoint(sx, sy) {
        if (!root.contains(root.mapFromItem(null, sx, sy))) return null;
        const p = workspace.mapFromItem(null, sx, sy);
        return { x: p.x - root.originOffsetX, y: p.y - root.originOffsetY };
    }

    property real _touchAnchorX: 0
    property real _touchAnchorY: 0
    property real _touchStartZoom: 1
    property bool _touchNavigating: false
    function beginTouchNavigation(point) {
        if (_touchNavigating) return;
        _touchNavigating = true;
        root.cancelDrag(); root.cancelValueDrag(); root.touchNavigationStarted();
        flick.cancelFlick(); zoomGlide.stop(); canvasScroll.stop();
        const p = root.mapFromItem(null, point.x, point.y);
        _touchAnchorX = (flick.contentX + p.x) / root.zoom;
        _touchAnchorY = (flick.contentY + p.y) / root.zoom;
        _touchStartZoom = root.zoom;
    }
    function updateTouchNavigation(point, scale) {
        if (!_touchNavigating || !touchNavigationActive) return;
        const p = root.mapFromItem(null, point.x, point.y);
        root._zoomTo(_touchAnchorX, _touchAnchorY, p.x, p.y,
            Math.max(0.5, Math.min(1.8, _touchStartZoom * scale)), false);
    }
    // Both handlers observe the contacts before any exclusive grab. Translation
    // also activates for parallel fingers, when a pinch has no scale change.
    DragHandler {
        id: twoFingerPan
        target: null
        acceptedDevices: PointerDevice.TouchScreen
        minimumPointCount: 2; maximumPointCount: 2
        dragThreshold: 0
        grabPermissions: PointerHandler.CanTakeOverFromAnything | PointerHandler.ApprovesTakeOverByAnything
        onActiveChanged: {
            if (active) root.beginTouchNavigation(centroid.scenePosition);
            else if (!screenPinch.active) root._touchNavigating = false;
        }
        onCentroidChanged: if (active) root.updateTouchNavigation(centroid.scenePosition, screenPinch.active ? screenPinch.activeScale : root.zoom / root._touchStartZoom)
    }
    PinchHandler {
        id: screenPinch
        target: null
        acceptedDevices: PointerDevice.TouchScreen
        minimumPointCount: 2; maximumPointCount: 2
        dragThreshold: 0
        grabPermissions: PointerHandler.CanTakeOverFromAnything | PointerHandler.ApprovesTakeOverByAnything
        onActiveChanged: {
            if (active) root.beginTouchNavigation(centroid.scenePosition);
            else if (!twoFingerPan.active) root._touchNavigating = false;
        }
        onActiveScaleChanged: if (active) root.updateTouchNavigation(centroid.scenePosition, activeScale)
        onCentroidChanged: if (active) root.updateTouchNavigation(centroid.scenePosition, activeScale)
    }
    Flickable {
        id:flick; anchors.fill:parent; contentWidth:root.worldW*root.zoom; contentHeight:root.worldH*root.zoom; boundsBehavior:Flickable.StopAtBounds
        ScrollBar.horizontal:ScrollBar{}
        ScrollBar.vertical:ScrollBar{}
        onWidthChanged: root.tryInitialCenter()
        onHeightChanged: root.tryInitialCenter()
        // While a wheel gesture is active the Flickable must not run its own
        // wheel logic: it would process every event a second time (it
        // un-accepts wheel events) with a dominant-axis threshold and
        // momentum flicks, which read as straight-line priming before
        // diagonal kicked in - and slow diagonals died out entirely. The
        // WheelHandler below is then the single owner and pans 1:1.
        interactive: !wheelPan.active && !root.touchNavigationActive && !emptyPan.active
        SmoothScroll { id: canvasScroll; flickable: flick }
        // A drag or flick takes over from a zoom glide in progress.
        onMovementStarted: zoomGlide.stop()
        // Dragging the canvas with a mouse stops where it's let go: the
        // release fling read as the view sliding away on its own. Touch
        // screens keep their momentum.
        property bool touchDrag: false
        onFlickStarted: if (!touchDrag) cancelFlick()
        PointHandler {
            target: null
            acceptedDevices: PointerDevice.TouchScreen
            onActiveChanged: if (active) flick.touchDrag = true
        }
        PointHandler {
            target: null
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad | PointerDevice.Stylus
            onActiveChanged: if (active) flick.touchDrag = false
        }
        // A touchpad pinch zooms around the point between the fingers, like
        // Ctrl+scroll does around the pointer.
        PinchHandler {
            id: pinchZoom
            target: null
            acceptedDevices: PointerDevice.TouchPad
            onScaleChanged: delta => {
                if (!isFinite(delta) || delta <= 0 || delta === 1) return;
                const sp = pinchZoom.centroid.scenePosition;
                const v = flick.mapFromItem(null, sp.x, sp.y);
                root.zoomBy(v.x, v.y, delta, true, root._touchpadZoomMs);
            }
        }
        // Two-finger touchpads emit one wheel event carrying both axes; pan
        // both from that same event so diagonal stays diagonal at any speed.
        WheelHandler {
            id: wheelPan
            target: null
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            orientation: Qt.Horizontal | Qt.Vertical
            onWheel: event => {
                if (event.modifiers & Qt.ControlModifier) {
                    // The handler sits on the Flickable's content item, so the
                    // event's own position is in (scrolled, scaled) content
                    // coordinates; zooming around that dragged the view right
                    // and down. Map the cursor into the viewport instead.
                    const sp = wheelPan.point.scenePosition;
                    const v = flick.mapFromItem(null, sp.x, sp.y);
                    // 12% per wheel notch (angleDelta 120), and in proportion
                    // for hi-res wheels. Touchpads send about 12 units per
                    // pixel, so 12% takes 20px of finger travel, with a
                    // shorter glide per step.
                    const delta = event.angleDelta.y !== 0 ? event.angleDelta.y : event.pixelDelta.y * 12;
                    const touchpad = canvasScroll.isTouchpad(event);
                    root.zoomBy(v.x, v.y, Math.pow(1.12, delta / (touchpad ? 240 : 120)), true, touchpad ? root._touchpadZoomMs : undefined);
                    event.accepted = true;
                    return;
                }
                zoomGlide.stop();
                // A mouse wheel gets Qt's own Flickable step per notch (angleDelta 120,
                // less on hi-res wheels) with no acceleration; a touchpad gets the
                // accelerated pan. pixelDelta is truncated to ints on touchpads;
                // angleDelta carries the real resolution (about 12 units per px).
                const touchpad = canvasScroll.isTouchpad(event);
                const pan = (ad, pd) => {
                    if (!touchpad) return ad / 120 * Qt.styleHints.wheelScrollLines * 24;
                    const d = ad !== 0 ? ad / 12 : pd;
                    return d * Math.min(24, 6 + 1.5 * Math.abs(d));
                };
                // A wheel notch glides to its target; a touchpad pan trails its
                // fingers by a few frames.
                const px = pan(event.angleDelta.x, event.pixelDelta.x), py = pan(event.angleDelta.y, event.pixelDelta.y);
                if (touchpad) canvasScroll.follow(px, py);
                else canvasScroll.scrollBy(px, py, true);
                event.accepted = true;
            }
        }
        Item {
            id:workspace; width:root.worldW;height:root.worldH; scale:root.zoom; transformOrigin:Item.TopLeft
            // Keep new delegates in place before Flickable applies the new scroll bounds.
            transform: Translate {
                x: root._resizeViewX === null ? 0 : flick.contentX - root._resizeViewX
                y: root._resizeViewY === null ? 0 : flick.contentY - root._resizeViewY
            }
            // The dot grid lives in world space and moves with the content,
            // so scrolling and zooming never repaint it. Each 22px tile holds a
            // quarter dot in every corner, drawn at twice the size and scaled
            // down so zooming in stays sharp; the margin covers a zoomed-out view.
            Image {
                id:grid; z:-200
                readonly property int margin: 22 * 100
                // Keep the tile phase tied to world coordinates as the origin moves.
                x:-margin + root.originOffsetX % 22; y:-margin + root.originOffsetY % 22
                width:2*(root.worldW+2*margin); height:2*(root.worldH+2*margin)
                scale:0.5; transformOrigin:Item.TopLeft
                fillMode:Image.Tile; smooth:true
                // Tile from the top left so resizing cannot shift the pattern.
                horizontalAlignment: Image.AlignLeft
                verticalAlignment: Image.AlignTop
                sourceSize: Qt.size(44, 44)
                source: "data:image/svg+xml;utf8," + encodeURIComponent("<svg xmlns='http://www.w3.org/2000/svg' width='44' height='44'><g fill='" + Theme.grid + "'>"
                    + "<circle cx='0' cy='0' r='2.3'/><circle cx='44' cy='0' r='2.3'/><circle cx='0' cy='44' r='2.3'/><circle cx='44' cy='44' r='2.3'/></g></svg>")
            }
            MouseArea {
                anchors.fill:parent; z:-100; acceptedButtons:Qt.LeftButton|Qt.RightButton
                containmentMask: QtObject {
                    function contains(point: point): bool {
                        for (const item of workspace.children) {
                            if (!item.sid && !(item.modelData && item.modelData.id)) continue;
                            if (item.contains(item.mapFromItem(workspace, point.x, point.y))) return false;
                        }
                        return true;
                    }
                }
                onPressAndHold: mouse => { canvasMenu.canvasX = Math.round(mouse.x - root.originOffsetX); canvasMenu.canvasY = Math.round(mouse.y - root.originOffsetY); canvasMenu.popup(mouse.x, mouse.y); }
                onClicked: mouse => { root.insertionPoint = null; if (mouse.button === Qt.RightButton) { canvasMenu.canvasX = Math.round(mouse.x - root.originOffsetX); canvasMenu.canvasY = Math.round(mouse.y - root.originOffsetY); canvasMenu.popup(mouse.x, mouse.y); } else flick.forceActiveFocus(); }
                DragHandler {
                    id: emptyPan
                    target: null
                    acceptedDevices: PointerDevice.TouchScreen
                    enabled: !root.touchNavigationActive && !root.dragging
                    grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.ApprovesTakeOverByAnything
                    property real viewX: 0
                    property real viewY: 0
                    property point start
                    onActiveChanged: if (active) {
                        flick.cancelFlick(); zoomGlide.stop(); canvasScroll.stop();
                        viewX = flick.contentX; viewY = flick.contentY; start = centroid.scenePosition;
                    }
                    onCentroidChanged: if (active) {
                        flick.contentX = Math.max(0, Math.min(flick.contentWidth-flick.width, viewX-(centroid.scenePosition.x-start.x)));
                        flick.contentY = Math.max(0, Math.min(flick.contentHeight-flick.height, viewY-(centroid.scenePosition.y-start.y)));
                    }
                }
                BwMenu { id:canvasMenu;property int canvasX:0;property int canvasY:0
                    BwMenuItem{visible:root.offerClear;iconName:root.clearIcon;danger:true;text:root.clearText;onTriggered:root.clearRequested()}
                    BwMenuItem{iconName:"clipboard-paste";enabled:root.canPaste;text:qsTr("Paste");onTriggered:root.canvasMenuAction("paste",canvasMenu.canvasX,canvasMenu.canvasY)}
                    BwMenuItem{iconName:"message-square";text:qsTr("Add Comment");onTriggered:root.canvasNoteRequested(canvasMenu.canvasX,canvasMenu.canvasY)}
                }
                Instantiator {
                    model: BlockRegistry.canvasMenu
                    delegate: BwMenuItem {
                        required property var modelData
                        iconName: modelData.icon || ""; text: modelData.text; danger: !!modelData.danger
                        onTriggered: root.canvasMenuAction(modelData.id, canvasMenu.canvasX, canvasMenu.canvasY)
                    }
                    onObjectAdded: (index, object) => canvasMenu.insertItem(index, object)
                    onObjectRemoved: (index, object) => canvasMenu.removeItem(object)
                }
            }
            Repeater {
                model: strandModel
                delegate:Column {
                    id:strand
                    required property string sid
                    required property real sx
                    required property real sy
                    required property string payload
                    required property int index
                    readonly property var blockList: JSON.parse(payload || "[]")
                    x:sx + root.originOffsetX; y:sy + root.originOffsetY; spacing:-8
                    z: (dragSession.active && dragSession.strandId === sid) || (dragSession.settling && dragSession.settleStrandId === sid) ? 100 : 0
                    // Report the real card width so the world extends past
                    // actual strand content, not a fixed estimate.
                    // (InstructionList below is a non-visual ListModel, so the
                    // Column's implicitWidth is still the widest block.)
                    onImplicitWidthChanged: root.noteStrandWidth(sid, implicitWidth)
                    onImplicitHeightChanged: root.noteStrandHeight(sid, implicitHeight)
                    Component.onCompleted: {
                        root.noteStrandWidth(sid, implicitWidth);
                        root.noteStrandHeight(sid, implicitHeight);
                    }
                    Component.onDestruction: {
                        root.clearStrandWidth(sid);
                        root.clearStrandHeight(sid);
                    }
                    InstructionList { id:strandBlocks; instructions:strand.blockList }
                    Repeater {
                        model:strandBlocks
                        delegate:InstructionBlock {
                            id:block; required property string payload; required property int index
                            instruction:JSON.parse(payload); strandId:strand.sid; path:[{index:index}]; tailCount:strandBlocks.count-index; recordingTarget:index===0&&root.recordingTargetId===strand.sid; dragState:dragSession
                            variables:root.variables; lists:root.lists; blockDefinitions:root.blockDefinitions; keyCapture:root.keyCapture; locked:root.locked
                            branchColor: root.branchColorForStrand(strand.sid)
                            onRemoveRequested:(sid,p)=>root.instructionRemoved(sid,p)
                            onDuplicateRequested:(sid,p,i)=>root.instructionDuplicated(sid,p,i)
                            onCommentRequested:i=>root.commentForInstructionRequested(i)
                            onRecordingTargetRequested:sid=>root.recordingTargetRequested(sid)
                            onKeyCaptureRequested:(sid,p)=>root.keyCaptureRequested(sid,p)
                            onAppPickerRequested:(sid,p,i)=>root.appPickerRequested(sid,p,i)
                            onPieceActionRequested:(a,sid,p,i)=>root.pieceActionRequested(a,sid,p,i)
                            onDetailsRequested:type=>root.detailsRequested(type)
                            onMenuActionRequested:(a,sid,p,i)=>root.blockMenuAction(a,sid,p,i)
                            onPaletteDragStarted:(sp,sx,sy,ox,oy)=>root.paletteDragStarted(sp,sx,sy,ox,oy)
                            onPaletteDragMoved:(sx,sy)=>root.paletteDragMoved(sx,sy)
                            onPaletteDragEnded:(sx,sy)=>root.paletteDragEnded(sx,sy)
                            onPaletteDragCanceled:root.paletteDragCanceled()
                            onInstructionEdited:(sid,p,i)=>root.instructionEdited(sid,p,i)
                            onRunBranchRequested:(sid,p,n)=>root.runBranchRequested(sid,p,n)
                            onValueEdited:(l,t)=>root.valueEdited(l,t)
                            onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.beginValueDrag(loc,val,sx,sy,ox,oy,w,h)
                            onValueDragMoved:(sx,sy)=>root.moveValueDrag(sx,sy)
                            onValueDragEnded:(sx,sy)=>root.endValueDrag(sx,sy)
                            onValueDragCanceled:root.cancelValueDrag()
                            onDragBegan:(sid,p,tail,sx,sy,ox,oy,bw,bh)=>root.beginDrag(sid,p,tail,sx,sy,ox,oy,bw,bh)
                            onDragMoved:(sx,sy)=>root.moveDrag(sx,sy)
                            onDragEnded:(sx,sy)=>root.endDrag(sx,sy)
                            onDragCanceled:root.cancelDrag()
                            onInsertionRequested:(sid,p)=>root.selectInsertion(sid,p)
                        }
                    }
                }
            }
            Repeater {
                model:root.comments||[]
                delegate:Rectangle {
                    id:commentCard; required property var modelData
                    objectName: "commentCard_" + modelData.id
                    readonly property var attachedBlock: modelData.attached_to ? root.findBlockItem(workspace, modelData.attached_to) : null
                    readonly property point blockAnchor: root.commentAnchor(attachedBlock)
                    Shape {
                        parent: workspace
                        objectName: "commentConnector_" + commentCard.modelData.id
                        z: -2
                        visible: !!commentCard.attachedBlock
                        ShapePath {
                            strokeColor: Theme.textDim
                            strokeWidth: 1.5
                            fillColor: "transparent"
                            startX: commentCard.blockAnchor.x
                            startY: commentCard.blockAnchor.y
                            PathLine {
                                // End inside the card so its rounded surface masks the line.
                                x: commentCard.x + commentCard.width / 2
                                y: commentCard.y + commentCard.height / 2
                            }
                        }
                    }
                    readonly property real headerH: 38
                    readonly property real bodyMinH: 74
                    readonly property real bodyMaxH: 240
                    x:modelData.x + root.originOffsetX;y:modelData.y + root.originOffsetY;z:4;width:220
                    height:modelData.collapsed ? headerH : headerH + 1 + Math.min(bodyMaxH, Math.max(bodyMinH, bodyArea.implicitHeight + 14))
                    radius:Theme.radius;color:Theme.panelRaised;border.color:Theme.border;clip:true
                    Row {
                        id:headerRow;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.leftMargin:8;anchors.rightMargin:4;height:commentCard.headerH;spacing:7
                        LucideIcon { name:"message-square";color:Theme.textDim;width:14;height:14;anchors.verticalCenter:parent.verticalCenter }
                        Text { id:commentLabel;text:qsTr("COMMENT");color:Theme.textDim;font.pixelSize:10;font.weight:Font.Bold;anchors.verticalCenter:parent.verticalCenter }
                        Item { width:Math.max(0,parent.width-14-commentLabel.width-collapseBtn.width-parent.spacing*3);height:1 }
                        BwButton { id:collapseBtn;iconName:commentCard.modelData.collapsed?"chevron-down":"chevron-up";text:"";flat:true;implicitWidth:24;implicitHeight:24;anchors.verticalCenter:parent.verticalCenter;onClicked:root.commentCollapseChanged(commentCard.modelData.id,!commentCard.modelData.collapsed) }
                    }
                    Rectangle { id:commentDivider;visible:!commentCard.modelData.collapsed;anchors.left:parent.left;anchors.right:parent.right;anchors.top:headerRow.bottom;height:1;color:Theme.borderSoft }
                    ScrollView {
                        id: bodyScroll
                        objectName: "commentScroll_" + commentCard.modelData.id
                        visible: !commentCard.modelData.collapsed
                        anchors.left: parent.left; anchors.right: parent.right
                        anchors.top: commentDivider.bottom; anchors.bottom: parent.bottom
                        contentWidth: availableWidth
                        clip: true
                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                        TextArea {
                            id: bodyArea
                            objectName: "commentText_" + commentCard.modelData.id
                            ContextMenu.menu: root.textMenu ? root.textMenu.createObject(bodyArea, { editor: bodyArea }) : null
                            text: commentCard.modelData.text
                            color: Theme.text
                            wrapMode: TextEdit.Wrap
                            padding: 7
                            background: null
                            onActiveFocusChanged: if (!activeFocus && text !== commentCard.modelData.text)
                                root.commentEdited(commentCard.modelData.id, text)
                        }
                    }
                    DragHandler {
                        parent: headerRow
                        enabled: !root.locked
                        target: commentCard
                        onActiveChanged: if (!active)
                            root.commentMoved(commentCard.modelData.id, Math.round(commentCard.x - root.originOffsetX), Math.round(commentCard.y - root.originOffsetY))
                    }
                    TapHandler { acceptedButtons:Qt.RightButton;gesturePolicy:TapHandler.ReleaseWithinBounds;onTapped:commentMenu.popup() }
                    BwMenu { id:commentMenu;BwMenuItem{iconName:"trash";danger:true;text:qsTr("Delete comment");onTriggered:root.commentRemoved(commentCard.modelData.id)} }
                }
            }
            Repeater {
                model:root.floatingValues||[]
                delegate:Item {
                    id:floatingItem;required property var modelData;x:modelData.x + root.originOffsetX;y:modelData.y + root.originOffsetY;z:5;width:floatingChip.implicitWidth;height:floatingChip.implicitHeight
                    ValueChip { id:floatingChip;valueData:floatingItem.modelData.value;originBlockId:floatingItem.modelData.origin_block_id||"";location:({kind:"Floating",floating_id:floatingItem.modelData.id,path:[]});boxed:true;blockDefinitions:root.blockDefinitions;onEditRequested:(l,t)=>root.valueEdited(l,t);onDetailsRequested:kind=>root.detailsRequested(kind);onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.beginValueDrag(loc,val,sx,sy,ox,oy,w,h);onValueDragMoved:(sx,sy)=>root.moveValueDrag(sx,sy);onValueDragEnded:(sx,sy)=>root.endValueDrag(sx,sy);onValueDragCanceled:root.cancelValueDrag() }
                    TapHandler { acceptedButtons:Qt.RightButton;gesturePolicy:TapHandler.ReleaseWithinBounds;onTapped:floatingMenu.popup() }
                    BwMenu { id:floatingMenu;BwMenuItem{iconName:"info";text:qsTr("Details");onTriggered:root.detailsRequested(floatingItem.modelData.value.block_id||floatingItem.modelData.value.op||floatingItem.modelData.value.kind)}BwMenuItem{iconName:"trash";danger:true;text:qsTr("Delete value");onTriggered:root.floatingValueRemoved(floatingItem.modelData.id)} }
                }
            }
            Rectangle {
                readonly property var selectedBlock: root.insertionPoint ? root.findBlockItem(workspace, root.insertionPoint.blockId) : null
                readonly property point at: selectedBlock ? selectedBlock.mapToItem(workspace, 0, selectedBlock.height - 8) : Qt.point(0, 0)
                visible: !!selectedBlock && !root.dragging
                x: at.x; y: at.y; width: selectedBlock ? selectedBlock.width : 0; height: 4 / root.zoom
                color: Theme.accent; radius: height / 2; z: 50
            }
            // Attach preview: the dragged first block's own silhouette (same
            // tabs/notches/C-shape as a real block, like the webapp's
            // translucent clone) exactly where the drop will land. Siblings
            // after it shift down and wrap mouths grow around it (see
            // InstructionBlock snapShift/effMouths), so the gap it sits in is
            // real layout room, not an overlay on top.
            Item {
                id: snapPreview
                visible: dragSession.snapValid && !dragSession.settling
                x: dragSession.snapX; y: dragSession.snapY
                width: Math.max(120, dragSession.snapWidth); height: Math.max(20, dragSession.snapHeight)
                z: 90; enabled: false; opacity: 0.55
                BlockSurface {
                    anchors.fill: parent
                    shape: dragSession.draggedShape
                    fill: Theme.accent
                    outline: Theme.accent
                    headHeight: 50; midHeight: 34; footHeight: 26; spine: 20
                    mouthHeights: dragSession.draggedMouths
                    flatEnds: dragSession.draggedFlat
                }
            }
            // Ghost following the pointer while an existing value is dragged.
            Item {
                id: valueGhost
                visible: (valueDrag.active && !root.externalDragPreview) || (valueDrag.settling && !valueDrag.settleHidden)
                x: valueDrag.active ? valueDrag.originX + valueDrag.dx : valueDrag.settleX
                y: valueDrag.active ? valueDrag.originY + valueDrag.dy : valueDrag.settleY
                width: Math.max(40, valueDrag.draggedWidth); height: Math.max(27, valueDrag.draggedHeight)
                z: 95; enabled: false; opacity: 0.9
                ValueChip {
                    anchors.fill: parent
                    valueData: valueDrag.sourceValue; boxed: true; editable: false
                    originBlockId: valueDrag.sourceOrigin; dragPreview: true
                    blockDefinitions: root.blockDefinitions
                }
            }
        }
    }
    Item {
        id: collectionOverlay
        anchors.fill: parent
        z: 12
        Repeater {
            model: listEditorModel
            delegate:Rectangle {
                id:listCard
                required property string editorName
                required property string payload
                required property real editorX
                required property real editorY
                readonly property var modelData: JSON.parse(payload)
                objectName: "listEditor_" + modelData.name
                readonly property real maxX: Math.max(0, collectionOverlay.width - width)
                readonly property real maxY: Math.max(0, collectionOverlay.height - height)
                visible:modelData.editor_visible;x:Math.max(0, Math.min(editorX, maxX));y:Math.max(0, Math.min(editorY, maxY));z:12
                function keepInView() {
                    if (x < 0 || x > maxX) x = Math.max(0, Math.min(x, maxX));
                    if (y < 0 || y > maxY) y = Math.max(0, Math.min(y, maxY));
                }
                onMaxXChanged: keepInView()
                onMaxYChanged: keepInView()
                width:300;height:Math.min(400,90+Math.max(64,(modelData.items||[]).length*40+14));radius:12;color:Theme.panel;border.color:Theme.borderSoft;clip:true
                function itemFromText(value){const trimmed=value.trim();const number=Number(value);return trimmed.length&&Number.isFinite(number)?{kind:"Number",value:number}:{kind:"Text",value:value};}
                function editItem(index,value){const next=JSON.parse(JSON.stringify(modelData.items||[]));next[index]=itemFromText(value);root.listItemsEdited(modelData.name,next);}
                function removeItem(index){const next=JSON.parse(JSON.stringify(modelData.items||[]));next.splice(index,1);root.listItemsEdited(modelData.name,next);}
                function addItem(index, value) {
                    const next = JSON.parse(JSON.stringify(modelData.items || []));
                    for (let i = 0; i < listRepeater.count; ++i) {
                        const row = listRepeater.itemAt(i);
                        if (row && row.field.text !== String(next[i].value))
                            next[i] = itemFromText(row.field.text);
                        if (row) row.field.submitting = true;
                    }
                    if (index !== undefined) next[index] = itemFromText(value);
                    root.pendingListFocus = {name: modelData.name, index: next.length};
                    next.push({kind: "Text", value: ""});
                    root.listItemsEdited(modelData.name, next);
                }
                Rectangle { id:listHead;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;height:44;color:"transparent"
                    Rectangle { anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom;height:1;color:Theme.borderSoft }
                    // Accept the press before the canvas can start panning.
                    MouseArea {
                        id: listHeaderDrag
                        objectName: "listHeaderDrag"
                        anchors.fill: parent
                        enabled: !root.locked
                        acceptedButtons: Qt.LeftButton
                        preventStealing: true
                        drag.target: listCard
                        drag.minimumX: 0
                        drag.maximumX: listCard.maxX
                        drag.minimumY: 0
                        drag.maximumY: listCard.maxY
                        cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                        onReleased: if (drag.active)
                            root.listEditorStateChanged(listCard.modelData.name, true, Math.round(listCard.x), Math.round(listCard.y))
                    }
                    Row { anchors.fill:parent;anchors.margins:10;spacing:8
                        LucideIcon{name:"layers";color:Theme.textDim;width:16;height:16;anchors.verticalCenter:parent.verticalCenter}
                        Text{text:listCard.modelData.name;color:Theme.text;font.pixelSize:13;font.weight:Font.DemiBold;width:parent.width-16-28-16;elide:Text.ElideRight;anchors.verticalCenter:parent.verticalCenter}
                        BwButton{iconName:"x";text:"";flat:true;implicitWidth:28;implicitHeight:28;anchors.verticalCenter:parent.verticalCenter;onClicked:root.listEditorStateChanged(listCard.modelData.name,false,Math.round(listCard.x),Math.round(listCard.y))}
                    }

                }
                Flickable { id:listScroll;anchors.left:parent.left;anchors.right:parent.right;anchors.top:listHead.bottom;anchors.bottom:listFoot.top;contentHeight:listRows.implicitHeight;clip:true
                    Column { id:listRows;width:parent.width;spacing:6;padding:10
                        Text{visible:!(listCard.modelData.items||[]).length;text:qsTr("This list is empty.");color:Theme.textDim;font.pixelSize:12;width:parent.width-20;height:44;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
                        Repeater{id:listRepeater;model:listCard.modelData.items||[];delegate:Row{
                            property alias field: itemField
                            required property var modelData;required property int index;spacing:6;width:listRows.width-20;height:34
                            Text{text:String(index+1);color:Theme.textDim;font.pixelSize:11;width:18;horizontalAlignment:Text.AlignRight;anchors.verticalCenter:parent.verticalCenter}
                            BwTextField {
                                id: itemField
                                objectName: "listItem_" + index
                                property bool submitting: false
                                text:String(modelData.value);color:modelData.kind==="Number"?Theme.accent:Theme.text;width:parent.width-18-28-12;implicitHeight:34;font.pixelSize:13
                                function appendItem(event) {
                                    event.accepted = true;
                                    submitting = true;
                                    listCard.addItem(index, text);
                                }
                                Keys.onReturnPressed: event => appendItem(event)
                                Keys.onEnterPressed: event => appendItem(event)
                                onEditingFinished: if (!submitting && text !== String(modelData.value)) listCard.editItem(index, text)
                                Component.onCompleted: Qt.callLater(function() {
                                    const pending = root.pendingListFocus;
                                    if (!pending || pending.name !== listCard.modelData.name || pending.index !== index) return;
                                    root.pendingListFocus = null;
                                    itemField.forceActiveFocus();
                                    itemField.selectAll();
                                    listRows.forceLayout();
                                    const position = itemField.mapToItem(listRows, 0, 0);
                                    listScroll.contentY = Math.max(0, Math.min(position.y, listScroll.contentHeight - listScroll.height));
                                })
                            }
                            BwButton{iconName:"trash-2";text:"";flat:true;danger:hovered;implicitWidth:28;implicitHeight:28;anchors.verticalCenter:parent.verticalCenter;onClicked:listCard.removeItem(index)}
                        }}
                    }
                }
                Rectangle {
                    id:listFoot
                    anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom
                    height:46;color:"transparent"
                    Rectangle { anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;height:1;color:Theme.borderSoft }
                    BwButton {
                        objectName:"addListItem";iconName:"plus";text:qsTr("Add item");implicitHeight:30
                        anchors.left:parent.left;anchors.leftMargin:10;anchors.verticalCenter:parent.verticalCenter
                        onClicked:listCard.addItem()
                    }
                    Text {
                        text:qsTr("%n item(s)","",(listCard.modelData.items||[]).length)
                        color:Theme.textDim;font.pixelSize:11
                        anchors.right:parent.right;anchors.rightMargin:12;anchors.verticalCenter:parent.verticalCenter
                    }
                }
            }
        }
        Repeater {
            model:dictEditorModel
            delegate:Rectangle {
                id:dictCard
                required property string editorName
                required property string payload
                required property real editorX
                required property real editorY
                readonly property var modelData: JSON.parse(payload)
                objectName: "dictEditor_" + modelData.name
                readonly property real maxX: Math.max(0, collectionOverlay.width - width)
                readonly property real maxY: Math.max(0, collectionOverlay.height - height)
                function keepInView() {
                    if (x < 0 || x > maxX) x = Math.max(0, Math.min(x, maxX));
                    if (y < 0 || y > maxY) y = Math.max(0, Math.min(y, maxY));
                }
                onMaxXChanged: keepInView()
                onMaxYChanged: keepInView()
                readonly property var entries: modelData.entries||[]
                visible:modelData.editor_visible;x:Math.max(0, Math.min(editorX, maxX));y:Math.max(0, Math.min(editorY, maxY));z:12
                width:360;height:Math.min(420,90+Math.max(64,entries.length*40+14));radius:12;color:Theme.panel;border.color:Theme.borderSoft;clip:true
                // A monitor edit is a literal: finite numbers stay numeric.
                function itemFromText(value){const trimmed=value.trim();const number=Number(value);return trimmed.length&&Number.isFinite(number)?{kind:"Number",value:number}:{kind:"Text",value:value};}
                function save(next){root.dictEntriesEdited(modelData.name,next);}
                function copy(){return JSON.parse(JSON.stringify(entries));}
                function editKey(index,key){const next=copy();next[index].key=key;save(next);}
                function editValue(index,value){const next=copy();next[index].value=itemFromText(value);save(next);}
                function removeEntry(index){const next=copy();next.splice(index,1);save(next);}
                function addEntry() {
                    const next = copy();
                    for (let i = 0; i < dictRepeater.count; ++i) {
                        const row = dictRepeater.itemAt(i);
                        if (!row) continue;
                        next[i].key = row.keyField.text;
                        if (row.valueField.text !== String(next[i].value.value))
                            next[i].value = itemFromText(row.valueField.text);
                        row.submitting = true;
                    }
                    root.pendingDictFocus = {name: modelData.name, index: next.length};
                    next.push({key: "", value: {kind: "Text", value: ""}});
                    save(next);
                }
                Rectangle { id:dictHead;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;height:44;color:"transparent"
                    Rectangle { anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom;height:1;color:Theme.borderSoft }
                    // Accept the press before the canvas can start panning.
                    MouseArea {
                        id: dictHeaderDrag
                        objectName: "dictHeaderDrag"
                        anchors.fill: parent
                        enabled: !root.locked
                        acceptedButtons: Qt.LeftButton
                        preventStealing: true
                        drag.target: dictCard
                        drag.minimumX: 0
                        drag.maximumX: dictCard.maxX
                        drag.minimumY: 0
                        drag.maximumY: dictCard.maxY
                        cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                        onReleased: if (drag.active)
                            root.dictEditorStateChanged(dictCard.modelData.name, true, Math.round(dictCard.x), Math.round(dictCard.y))
                    }
                    Row { anchors.fill:parent;anchors.margins:10;spacing:8
                        LucideIcon{name:"book-key";color:Theme.textDim;width:16;height:16;anchors.verticalCenter:parent.verticalCenter}
                        Text{text:dictCard.modelData.name;color:Theme.text;font.pixelSize:13;font.weight:Font.DemiBold;width:parent.width-16-28-16;elide:Text.ElideRight;anchors.verticalCenter:parent.verticalCenter}
                        BwButton{iconName:"x";text:"";flat:true;implicitWidth:28;implicitHeight:28;anchors.verticalCenter:parent.verticalCenter;onClicked:root.dictEditorStateChanged(dictCard.modelData.name,false,Math.round(dictCard.x),Math.round(dictCard.y))}
                    }

                }
                Flickable { id:dictScroll;anchors.left:parent.left;anchors.right:parent.right;anchors.top:dictHead.bottom;anchors.bottom:dictFoot.top;contentHeight:dictRows.implicitHeight;clip:true
                    Column { id:dictRows;width:parent.width;spacing:6;padding:10
                        Text{visible:!dictCard.entries.length;text:qsTr("This dict is empty.");color:Theme.textDim;font.pixelSize:12;width:parent.width-20;height:44;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
                        Repeater{id:dictRepeater;model:dictCard.entries;delegate:Row{
                            id: entryRow
                            property alias keyField: dictKey
                            property alias valueField: dictValue
                            property bool submitting: false
                            function appendEntry(event) { event.accepted = true; dictCard.addEntry(); }
                            required property var modelData;required property int index;spacing:6;width:dictRows.width-20;height:34
                            BwTextField{
                                id: dictKey
                                objectName: "dictKey_" + index
                                Keys.onReturnPressed: event => entryRow.appendEntry(event)
                                Keys.onEnterPressed: event => entryRow.appendEntry(event)
                                Component.onCompleted: Qt.callLater(function() {
                                    const pending = root.pendingDictFocus;
                                    if (!pending || pending.name !== dictCard.modelData.name || pending.index !== index) return;
                                    root.pendingDictFocus = null;
                                    dictKey.forceActiveFocus();
                                    dictKey.selectAll();
                                    dictRows.forceLayout();
                                    const position = dictKey.mapToItem(dictRows, 0, 0);
                                    dictScroll.contentY = Math.max(0, Math.min(position.y, dictScroll.contentHeight - dictScroll.height));
                                })
                                text:modelData.key;placeholderText:qsTr("key");width:128;implicitHeight:34;font.pixelSize:13;onEditingFinished:if(!entryRow.submitting && text!==modelData.key)dictCard.editKey(index,text)}
                            LucideIcon{name:"arrow-right";color:Theme.textDim;width:13;height:13;anchors.verticalCenter:parent.verticalCenter}
                            BwTextField{
                                id: dictValue
                                objectName: "dictValue_" + index
                                Keys.onReturnPressed: event => entryRow.appendEntry(event)
                                Keys.onEnterPressed: event => entryRow.appendEntry(event)
                                text:String(modelData.value.value);placeholderText:qsTr("value");color:modelData.value.kind==="Number"?Theme.accent:Theme.text;width:parent.width-128-13-28-18;implicitHeight:34;font.pixelSize:13;onEditingFinished:if(!entryRow.submitting && text!==String(modelData.value.value))dictCard.editValue(index,text)}
                            BwButton{iconName:"trash-2";text:"";flat:true;danger:hovered;implicitWidth:28;implicitHeight:28;anchors.verticalCenter:parent.verticalCenter;onClicked:dictCard.removeEntry(index)}
                        }}
                    }
                }
                Rectangle {
                    id:dictFoot
                    anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom
                    height:46;color:"transparent"
                    Rectangle { anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;height:1;color:Theme.borderSoft }
                    BwButton {
                        objectName:"addDictEntry";iconName:"plus";text:qsTr("Add entry");implicitHeight:30
                        anchors.left:parent.left;anchors.leftMargin:10;anchors.verticalCenter:parent.verticalCenter
                        onClicked:dictCard.addEntry()
                    }
                    Text {
                        text:qsTr("%n entry(s)","",dictCard.entries.length)
                        color:Theme.textDim;font.pixelSize:11
                        anchors.right:parent.right;anchors.rightMargin:12;anchors.verticalCenter:parent.verticalCenter
                    }
                }
            }
        }
    }
    // Keeps the grab cursor while a block or value is dragged away from its source.
    MouseArea { anchors.fill: parent; visible: root.dragging; acceptedButtons: Qt.NoButton; cursorShape: Qt.ClosedHandCursor }
    Column {
        anchors.right:parent.right;anchors.bottom:parent.bottom;anchors.margins:18;spacing:8
        BwButton { iconName:"zoom-in";text:"";implicitWidth:40;implicitHeight:40;radius:20;onClicked:root.stepZoom(.1) }
        BwButton { iconName:"zoom-out";text:"";implicitWidth:40;implicitHeight:40;radius:20;onClicked:root.stepZoom(-.1) }
        BwButton { iconName:"rotate-ccw";text:"";implicitWidth:40;implicitHeight:40;radius:20;onClicked:root.resetView() }
    }
}
