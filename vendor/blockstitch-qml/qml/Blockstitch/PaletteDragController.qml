import QtQuick

// Runs palette drags for one canvas: the ghost under the pointer, gap and
// slot targeting, trash drops, and the landing hold. A drop becomes one
// semantic edit handed to the host's `submit`; the host maps it onto its
// own commands. Fill the editor with it, above the palette and the canvas.
//
// Edits (`kind` and its fields):
//   insert_instruction  strandId, path, instruction
//   add_strand          x, y, instruction
//   put_value           location, value
//   create_floating_value  x, y, value, originBlockId
//   remove_instructions strandId, path, count
//   remove_strand       strandId
//   take_value          location
//   remove_floating_value  floatingId
Item {
    id: root
    z: 500

    property var canvas: null
    // Context for drawing the ghost.
    property var variables: []
    property var lists: []
    property var blockDefinitions: []
    // Drops are refused while locked, and a drag in progress is cancelled.
    property bool locked: false
    // A change cancels the drag and drops any held ghost (another macro or actor).
    property string documentId: ""
    // The part of the document a drop changes. Once an accepted drop's edit
    // shows up here the held ghost goes; unrelated state pushes leave it.
    property var document: null
    property bool holdLanding: true
    property int holdTimeout: 1500
    // function(sceneX, sceneY) -> bool: whether a point is over the trash.
    property var trashContains: null
    // function(spec) -> a fresh instruction with its own id. The default
    // copies spec.instruction; branch and custom calls come ready-made.
    property var instructionFor: null
    // function(spec) -> the value a value entry drops. Defaults to spec.value.
    property var valueFor: null
    // function(edit, reply): send the edit, then reply(true) or reply(false, error).
    property var submit: null

    // { spec, entry, offsetX, offsetY } while a palette entry is dragged.
    property var drag: null
    readonly property bool active: drag !== null
    readonly property bool holding: ghostHeld
    readonly property bool overTrash: canvasGhost.overTrash

    function newId() { return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, c => { const r = Math.random() * 16 | 0; return (c === "x" ? r : (r & 3 | 8)).toString(16); }); }
    function copy(v) { return v === undefined ? v : JSON.parse(JSON.stringify(v)); }
    // A palette spec as what it drops: an instruction or a value.
    function entryFor(spec) {
        if (!spec) return null;
        if (spec.kind === "value") {
            const value = root.valueFor ? root.valueFor(spec) : copy(spec.value);
            return value ? { kind: "value", value: value, originBlockId: spec.originBlockId || null } : null;
        }
        let instruction = root.instructionFor ? root.instructionFor(spec) : null;
        if (!instruction && spec.instruction) { instruction = copy(spec.instruction); instruction.id = newId(); }
        return instruction ? { kind: "instruction", instruction: instruction } : null;
    }
    function inTrash(sx, sy) { return !!root.trashContains && root.trashContains(sx, sy); }
    function send(edit, onReply) {
        if (!root.submit) { console.warn("PaletteDragController: no submit for", edit.kind); return; }
        root.submit(edit, (ok, error) => { if (onReply) onReply(ok !== false, error); });
    }

    Connections { target: root.canvas; function onTouchNavigationStarted() { root.cancel(); } }

    // ---- palette drags ----
    function begin(spec, sx, sy, ox, oy) {
        releaseGhost();
        if (root.locked || !root.canvas) return;
        const entry = entryFor(spec);
        if (!entry) return;
        drag = { spec: spec, entry: entry, offsetX: ox, offsetY: oy };
        ghost.instruction = spec.instruction || entry.instruction || null;
        ghost.spec = spec;
        move(sx, sy);
        ghost.visible = true;
    }
    function move(sx, sy) {
        if (!drag) return;
        const p = root.mapFromItem(null, sx, sy);
        ghost.x = p.x - drag.offsetX * ghost.scale; ghost.y = p.y - drag.offsetY * ghost.scale;
        if (drag.entry.kind === "value") {
            canvas.clearSnap();
            // The ghost's footprint picks the nearest slot; before the ghost
            // has a size the canvas falls back to the pointer.
            const g1 = ghost.mapToItem(null, 0, 0);
            const g2 = ghost.mapToItem(null, ghost.width, ghost.height);
            canvas.updatePaletteValueTargetRect(g1.x, g1.y, g2.x, g2.y, sx, sy);
            return;
        }
        canvas.updatePaletteValueTarget(null, null);
        const at = canvas.workspacePoint(sx, sy);
        if (at) canvas.updatePaletteSnap(at.x - drag.offsetX, at.y - drag.offsetY, drag.entry.instruction, ghost.width, ghost.height);
        else canvas.clearSnap();
    }
    function end(sx, sy) {
        const d = drag;
        if (!d) return;
        if (root.locked) { cancel(); return; }
        const snapValid = canvas.paletteSnapValid, snapTargetId = canvas.paletteSnapTargetId;
        const snapPath = copy(canvas.paletteSnapPath || []);
        const targetY = canvas.paletteSnapTargetY;
        const valueTarget = canvas.paletteValueTarget ? copy(canvas.paletteValueTarget) : null;
        const landing = canvas.paletteDropScenePoint();
        drag = null;
        canvas.updatePaletteValueTarget(null, null);
        const entry = d.entry;
        if (entry.kind === "value" && valueTarget) {
            canvas.clearSnap();
            dropped({ kind: "put_value", location: valueTarget, value: entry.value }, landing);
            return;
        }
        const at = canvas.workspacePoint(sx, sy);
        if (!at) { cancel(); return; }
        const x = Math.round(at.x - d.offsetX), y = Math.round(at.y - d.offsetY);
        if (entry.kind === "value") {
            canvas.clearSnap();
            dropped({ kind: "create_floating_value", x: x, y: y, value: entry.value, originBlockId: entry.originBlockId }, null);
            return;
        }
        if (snapValid && snapTargetId) {
            canvas.settlePaletteDrop();
            dropped({ kind: "insert_instruction", strandId: snapTargetId, path: snapPath, instruction: entry.instruction, targetY: targetY }, landing);
            return;
        }
        canvas.clearSnap();
        dropped({ kind: "add_strand", x: x, y: y, instruction: entry.instruction }, null);
    }
    function cancel() {
        drag = null;
        releaseGhost();
        if (canvas) { canvas.clearSnap(); canvas.updatePaletteValueTarget(null, null); }
    }
    // A double-click or a keyboard add: the same edit a drop on open canvas makes.
    function addAt(spec, x, y) {
        if (root.locked) return false;
        const entry = entryFor(spec);
        if (!entry) return false;
        if (entry.kind === "value") send({ kind: "create_floating_value", x: Math.round(x), y: Math.round(y), value: entry.value, originBlockId: entry.originBlockId });
        else send({ kind: "add_strand", x: Math.round(x), y: Math.round(y), instruction: entry.instruction });
        return true;
    }
    // Workspace point near the top-left of what the canvas shows.
    function viewPoint() {
        const scene = canvas.mapToItem(null, 80, 80);
        return canvas.workspacePoint(scene.x, scene.y) || { x: 0, y: 0 };
    }
    // A touch add uses the selected gap, or a free position in the visible view.
    function addAtView(spec) {
        if (root.locked || !canvas) return false;
        const entry = entryFor(spec);
        if (!entry) return false;
        const point = canvas.insertionPoint;
        const strand = point && canvas.strandById(point.strandId);
        if (entry.kind === "instruction" && strand && !canvas.isHeaderType(entry.instruction.type)) {
            const list = canvas.listFor(strand, point.path.slice(0, -1));
            const at = point.path[point.path.length - 1].index;
            // A terminal block has no following connector. Never create unreachable code.
            if (list && at <= list.length && !(at > 0 && canvas.isCapBlock(list[at - 1]))) {
                send({ kind: "insert_instruction", strandId: point.strandId, path: copy(point.path), instruction: entry.instruction });
                return true;
            }
        }
        const at = canvas.findEmptyPosition(entry.kind === "value" ? 160 : 260, entry.kind === "value" ? 48 : canvas.blockHeight(entry.instruction));
        return addAt(spec, at.x, at.y);
    }

    // ---- canvas drags dropped on the trash ----
    // A tail is one removal; a whole strand goes with its first block.
    function trashBlocks(strandId, path, tailCount, sx, sy) {
        if (root.locked || !inTrash(sx, sy)) return false;
        canvas.settleTrashed();
        if (path.length === 1 && path[0].index === 0) send({ kind: "remove_strand", strandId: strandId });
        else send({ kind: "remove_instructions", strandId: strandId, path: copy(path), count: Math.max(1, tailCount) });
        return true;
    }
    // A value resets its slot; a whole floating block is deleted.
    function trashValue(location, value, sx, sy) {
        if (root.locked || !inTrash(sx, sy)) return false;
        canvas.settleTrashed();
        if (location.kind === "Floating" && !(location.path && location.path.length)) send({ kind: "remove_floating_value", floatingId: location.floating_id });
        else send({ kind: "take_value", location: copy(location) });
        return true;
    }

    // ---- landing ----
    // The ghost stays where the block will land until the host confirms the
    // edit and the document shows it, so it never blinks out. A rejected
    // edit lets go at once.
    property bool ghostHeld: false
    property int _pending: 0
    property bool _accepted: false
    property string _heldDoc: ""
    function documentText() { return root.document ? JSON.stringify(root.document) : ""; }
    function dropped(edit, landing) {
        const id = ++_pending;
        if (root.holdLanding) holdGhost(landing);
        else releaseGhost();
        send(edit, (ok, error) => {
            if (id !== _pending) return;
            if (!ok) { releaseGhost(); if (canvas) canvas.endSettle(); return; }
            _accepted = true;
            if (documentText() !== _heldDoc) releaseGhost();
        });
    }
    function holdGhost(scenePoint) {
        if (scenePoint) { const p = root.mapFromItem(null, scenePoint.x, scenePoint.y); ghost.x = p.x; ghost.y = p.y; }
        ghostHeld = true; _accepted = false;
        _heldDoc = documentText();
        holdTimer.restart();
    }
    function releaseGhost() {
        holdTimer.stop();
        ghostHeld = false; _accepted = false;
        if (!drag) { ghost.visible = false; ghost.spec = null; ghost.instruction = null; }
    }
    Timer { id: holdTimer; interval: root.holdTimeout; onTriggered: root.releaseGhost() }
    onDocumentChanged: if (ghostHeld && _accepted && documentText() !== _heldDoc) releaseGhost()
    onDocumentIdChanged: { _pending++; cancel(); }
    onLockedChanged: if (locked && drag) cancel()

    Connections {
        target: root.canvas
        // Parameter ovals dragged out of a block header run as palette drags.
        function onPaletteDragStarted(spec, sx, sy, ox, oy) { root.begin(spec, sx, sy, ox, oy); }
        function onPaletteDragMoved(sx, sy) { root.move(sx, sy); }
        function onPaletteDragEnded(sx, sy) { root.end(sx, sy); }
        function onPaletteDragCanceled() { root.cancel(); }
        function onBlockDragOutside(strandId, path, tailCount, sx, sy) { root.trashBlocks(strandId, path, tailCount, sx, sy); }
        function onValueDragOutside(location, value, sx, sy) { root.trashValue(location, value, sx, sy); }
    }

    MouseArea {
        objectName: "dragCursor"
        anchors.fill: parent; z: 1
        visible: root.active || (!!root.canvas && root.canvas.dragging)
        acceptedButtons: Qt.NoButton
        cursorShape: Qt.ClosedHandCursor
    }
    Binding {
        target: root.canvas
        property: "externalDragPreview"
        value: true
    }
    // Draw the whole drag above the sidebar, even before the pointer crosses it.
    Item {
        id: canvasGhost
        objectName: "canvasDragGhost"
        enabled: false
        readonly property bool overTrash: !!root.canvas && root.canvas.dragging && root.inTrash(root.canvas.dragSceneX, root.canvas.dragSceneY)
        visible: !!root.canvas && root.canvas.dragging
        property point pos: visible ? root.mapFromItem(null, root.canvas.dragGhostX, root.canvas.dragGhostY) : Qt.point(0, 0)
        x: pos.x; y: pos.y; opacity: 0.92
        Column {
            visible: !root.canvas || !root.canvas.dragIsValue
            spacing: -8; scale: root.canvas ? root.canvas.zoom : 1; transformOrigin: Item.TopLeft
            Repeater {
                model: canvasGhost.visible && !root.canvas.dragIsValue ? root.canvas.dragInstructions : []
                delegate: InstructionBlock {
                    required property var modelData
                    instruction: modelData; paletteMode: true; locked: true
                    branchColor: root.canvas ? root.canvas.dragBranchColor : Theme.accent
                    variables: root.variables; lists: root.lists; blockDefinitions: root.blockDefinitions
                }
            }
        }
        Loader {
            active: canvasGhost.visible && root.canvas.dragIsValue
            scale: root.canvas ? root.canvas.zoom : 1; transformOrigin: Item.TopLeft
            sourceComponent: ValueChip {
                valueData: root.canvas.dragValue; boxed: true; editable: false
                originBlockId: root.canvas.dragValueOrigin; dragPreview: true
                blockDefinitions: root.blockDefinitions
            }
        }
    }
    // Follows the pointer while a palette entry is dragged.
    Item {
        id: ghost
        objectName: "paletteGhost"
        enabled: false
        property var spec: null
        property var instruction: null
        // Match the canvas at pickup, keeping the grabbed point under the pointer.
        scale: root.canvas ? root.canvas.zoom : 1
        transformOrigin: Item.TopLeft
        visible: false; opacity: 0.92
        width: ghostBlock.active ? ghostBlock.item.width : (ghostValue.active ? ghostValue.item.width : 0)
        height: ghostBlock.active ? ghostBlock.item.height : (ghostValue.active ? ghostValue.item.height : 0)
        Loader {
            id: ghostBlock
            active: ghost.visible && !!ghost.spec && ghost.spec.kind !== "value"
            sourceComponent: InstructionBlock {
                instruction: ghost.instruction; paletteMode: true; locked: true
                blockColor: ghost.spec.color || Theme.block
                branchColor: ghost.spec.color || Theme.accent
                variables: root.variables; lists: root.lists; blockDefinitions: root.blockDefinitions
            }
        }
        Loader {
            id: ghostValue
            active: ghost.visible && !!ghost.spec && ghost.spec.kind === "value"
            sourceComponent: ValueChip {
                valueData: ghost.spec.value; boxed: true; editable: false; dragPreview: true
                originBlockId: ghost.spec.originBlockId || ""
                referenceColor: ghost.spec.color || Theme.accent
                forceBoolean: !!ghost.spec.forceBoolean; callDisplayLabel: ghost.spec.label || qsTr("custom block")
                blockDefinitions: root.blockDefinitions
            }
        }
    }
}
