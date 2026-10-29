import QtQuick
import QtQuick.Controls
import QtQuick.Shapes

Item {
    id: root
    property var instruction
    property string strandId: ""
    property var path: []
    property var variables: []
    property var lists: []
    property var blockDefinitions: []
    property var keyCapture: null
    property bool paletteMode: false
    property bool locked: false
    // The first block of the strand new recordings are appended to.
    property bool recordingTarget: false
    property color blockColor: Theme.block
    property color branchColor: Theme.accent
    // shared drag session owned by BlockCanvas (null for palette/preview blocks)
    property var dragState: null
    // number of instructions from this one to the end of its list (inclusive)
    property int tailCount: 1
    signal tapped()
    signal insertionRequested(string strandId, var path)
    signal dragBegan(string strandId, var path, int tailCount, real sceneX, real sceneY, real offsetX, real offsetY, real blockW, real blockH)
    signal dragMoved(real sceneX, real sceneY)
    signal dragEnded(real sceneX, real sceneY)
    signal dragCanceled()
    signal activated()
    signal removeRequested(string strandId, var path)
    signal duplicateRequested(string strandId, var path, var instruction)
    signal commentRequested(var instruction)
    signal recordingTargetRequested(string strandId)
    signal instructionEdited(string strandId, var path, var instruction)
    signal valueEdited(var location, string text)
    signal valueDragBegan(var location, var value, real sceneX, real sceneY, real offsetX, real offsetY, real w, real h)
    signal valueDragMoved(real sceneX, real sceneY)
    signal valueDragEnded(real sceneX, real sceneY)
    signal valueDragCanceled()
    signal runBranchRequested(string strandId, var path, string name)
    signal keyCaptureRequested(string strandId, var path)
    signal appPickerRequested(string strandId, var path, var instruction)
    // A registered row's "button" piece was clicked; `action` names it.
    signal pieceActionRequested(string action, string strandId, var path, var instruction)
    signal detailsRequested(string type)
    // A host menu entry from BlockRegistry.blockMenu was chosen.
    signal menuActionRequested(string action, string strandId, var path, var instruction)
    // Palette entries add their own menu items ([{id, text, icon, danger}]).
    property var extraMenu: []
    signal extraMenuTriggered(string id)
    // A parameter oval dragged out of a custom block's header: the same drag
    // a palette value makes, with spec {kind:"value", value:{kind:"Param"}, originBlockId}.
    signal paletteDragStarted(var spec, real sceneX, real sceneY, real offsetX, real offsetY)
    signal paletteDragMoved(real sceneX, real sceneY)
    signal paletteDragEnded(real sceneX, real sceneY)
    signal paletteDragCanceled()

    readonly property string type: instruction ? instruction.type : ""
    readonly property bool isWrap: BlockRegistry.isWrap(type)
    readonly property bool isHeader: BlockRegistry.isHeader(type)
    readonly property bool isRecordingTarget: strandId !== "" && BlockRegistry.recordingTargetId === strandId
    readonly property bool hasComment: !!instruction && BlockRegistry.commentedIds.indexOf(instruction.id) >= 0
    readonly property string markText: !paletteMode && instruction ? (BlockRegistry.blockMarks[instruction.id] || "") : ""
    readonly property bool flashing: !paletteMode && !!instruction && BlockRegistry.flashId === instruction.id
    readonly property bool running: !paletteMode && !!instruction && BlockRegistry.runningIds.indexOf(instruction.id) >= 0
    readonly property bool hasBreakpoint: !paletteMode && !!instruction && BlockRegistry.breakpointIds.indexOf(instruction.id) >= 0
    readonly property bool pausedHere: !paletteMode && !!instruction && BlockRegistry.pausedIds.indexOf(instruction.id) >= 0
    readonly property bool isCap: BlockRegistry.isCap(instruction, blockDefinitions)
    // The host's declarative row for this type, if it registered one.
    readonly property var rowSpec: (type === "CallBlock" || type === "BranchCallBlock")
        ? BlockRegistry.customCallRow(defFor(instruction.block_id)) || BlockRegistry.row(type) : BlockRegistry.row(type)
    // A new pieces array rebuilds every control on the row, so it only
    // changes when a different set of pieces shows.
    property var headPieces: []
    readonly property var wantedPieces: {
        BlockRegistry.revision;
        return BlockRegistry.rowPieces(rowSpec, instruction, paletteMode);
    }
    onWantedPiecesChanged: {
        const a = headPieces, b = wantedPieces;
        if (a.length === b.length && a.every((p, i) => p === b[i])) return;
        headPieces = b;
    }
    // Group only the displayed pieces; hidden conditional fields take no space.
    readonly property var headGroups: {
        const groups = [];
        for (const piece of headPieces) {
            if (piece.kind !== "removeComponent") { groups.push(piece); continue; }
            const pieces = [piece], members = piece.component.pieces || [];
            while (groups.length && members.indexOf(groups[groups.length - 1]) >= 0)
                pieces.unshift(groups.pop());
            groups.push({ kind: "component", component: piece.component, pieces: pieces });
        }
        return groups;
    }
    readonly property int rowHeight: 58
    readonly property real tabDepth: 8
    readonly property real spine: 20
    readonly property real headHeight: 50
    readonly property real midHeight: 34
    readonly property real footHeight: 26
    readonly property real emptyMouth: 26
    readonly property bool hovered: grab.hovered && !grab.dragging
    // 0: unaffected, 1: moves with the dragged block (it or a later sibling), 2: contains the dragged block.
    // While `settling` (dropped, waiting for the daemon round trip) the tail
    // keeps its drop offset so it doesn't visibly snap back and teleport.
    readonly property int dragRole: {
        const d = dragState;
        if (!d) return 0;
        if (d.active && d.strandId === strandId) {
            const a = d.path, b = path;
            if (b.length > a.length) return 0;
            for (let i = 0; i < b.length - 1; ++i)
                if (b[i].index !== a[i].index || (b[i].slot || 0) !== (a[i].slot || 0)) return 0;
            if (b.length < a.length) return b[b.length - 1].index === a[b.length - 1].index ? 2 : 0;
            return b[b.length - 1].index >= a[a.length - 1].index ? 1 : 0;
        }
        if (d.settling && d.settleStrandId === strandId) {
            const a = d.settlePath || [], b = path;
            if (!a.length || b.length > a.length) return 0;
            for (let i = 0; i < b.length - 1; ++i)
                if (b[i].index !== a[i].index || (b[i].slot || 0) !== (a[i].slot || 0)) return 0;
            if (b.length < a.length) return b[b.length - 1].index === a[b.length - 1].index ? 2 : 0;
            return b[b.length - 1].index >= a[a.length - 1].index ? 1 : 0;
        }
        return 0;
    }
    readonly property bool isCustomBlock: ["CallBlock","BranchCallBlock","BlockHeader"].indexOf(type) >= 0
    readonly property color customColor: {
        if (instruction && isCustomBlock) {
            const d = defFor(instruction.block_id);
            if (d && d.color) return d.color;
        }
        return Theme.accent;
    }
    // Match the web canvas: the chosen color marks a custom block's icon and
    // hover outline, while the block body stays in the neutral canvas palette.
    readonly property color displayColor: type === "RunBranch" ? Theme.panelRaised : isCustomBlock ? Theme.block : blockColor
    implicitWidth: isWrap ? Math.max(218, headContent.implicitWidth + 42, spine + bodyWidth + 24) : Math.max(type === "RunBranch" ? 150 : isHeader ? 108 : 132, fields.implicitWidth + (isHeader ? 24 : 28), hasBranchHeader ? pvOuterWidth + 28 : 0)
    implicitHeight: isWrap ? headHeight + mouthTotal + Math.max(0, slotCount() - 1) * midHeight + footHeight + (isCap ? 0 : tabDepth) : hasBranchHeader ? pvTop + pvOuterHeight + 8 + tabDepth : isCap ? rowHeight - tabDepth : rowHeight
    z: dragRole > 0 ? 50 : hovered ? 2 : 1
    // The overlay draws active tails; trashed tails wait for their removal.
    opacity: dragRole === 1 && dragState && ((dragState.active && dragState.externalPreview) || (!dragState.active && dragState.settling && dragState.settleHidden)) ? 0 : 1
    readonly property real dragOffsetX: {
        if (!dragState || dragRole !== 1) return 0;
        if (dragState.active) return dragState.dx;
        if (dragState.settling) return dragState.settleDx;
        return 0;
    }
    readonly property real dragOffsetY: {
        if (!dragState || dragRole !== 1) return 0;
        if (dragState.active) return dragState.dy;
        if (dragState.settling) return dragState.settleDy;
        return 0;
    }
    // Attach preview gap: stationary rows at/after the snap insertion point
    // shift down by exactly the dragged height, opening real room. Dragged
    // rows (dragRole 1) are excluded - they already follow the pointer.
    function snapLayout() {
        return dragState && dragState.snapLayout !== undefined ? dragState.snapLayout : dragState;
    }
    readonly property real snapShift: {
        const d = snapLayout();
        if (!d || !d.snapValid) return 0;
        if (d.snapTargetId !== strandId) return 0;
        if (dragRole === 1) return 0;
        const sp = d.snapPath || [];
        if (!sp.length || sp.length !== path.length) return 0;
        for (let i = 0; i < path.length - 1; ++i) {
            if (path[i].index !== sp[i].index) return 0;
            const a = (path[i].slot === undefined || path[i].slot === null) ? null : path[i].slot;
            const b = (sp[i].slot === undefined || sp[i].slot === null) ? null : sp[i].slot;
            if (a !== b) return 0;
        }
        if (path[path.length - 1].index < sp[sp.length - 1].index) return 0;
        return d.snapShiftAmt;
    }
    // Ease layout gaps without delaying the block under the pointer.
    readonly property bool animateLayout: Motion.enabled && !!dragState && !dragState.settling && !dragState.applyingDocument && dragRole !== 1
    Behavior on implicitWidth {
        enabled: root.isWrap && root.animateLayout
        NumberAnimation { duration: Motion.dur(140); easing.type: Easing.OutCubic }
    }
    property real displayedSnapShift: snapShift
    Behavior on displayedSnapShift {
        enabled: root.animateLayout
        NumberAnimation { duration: Motion.dur(140); easing.type: Easing.OutCubic }
    }
    transform: Translate { x: root.dragOffsetX; y: root.dragOffsetY + root.displayedSnapShift }

    // Heights/widths of each mouth's nested content, reported by the mouth delegates.
    property var mouthHeights: []
    property var mouthWidths: []
    property var flatEnds: []
    function isSnapMouth(k) {
        const d = snapLayout();
        if (!d || !d.snapValid) return false;
        if (d.snapTargetId !== strandId) return false;
        const sp = d.snapPath || [];
        if (sp.length !== path.length + 1) return false;
        for (let i = 0; i < path.length; ++i) {
            if (path[i].index !== sp[i].index) return false;
            if (i < path.length - 1) {
                const a = (path[i].slot === undefined || path[i].slot === null) ? null : path[i].slot;
                const b = (sp[i].slot === undefined || sp[i].slot === null) ? null : sp[i].slot;
                if (a !== b) return false;
            }
        }
        const mouthSlot = (sp[path.length - 1].slot === undefined || sp[path.length - 1].slot === null) ? null : sp[path.length - 1].slot;
        return mouthSlot === k;
    }
    // The mouth the dragged tail is being pulled out of (if this block's
    // mouth is it): mirrors isSnapMouth against the drag's source container
    // so the bracket shell collapses around the remaining prefix live.
    function isSourceMouth(k) {
        const d = dragState;
        if (!d || (!d.active && !d.settling)) return false;
        if (d.sourceStrandId !== strandId) return false;
        const sp = d.sourceBasePath || [];
        // NOTE: unlike a snap path (container + insertion step), the source
        // base path addresses the container itself, so it is the same length
        // as the owning wrap block's path.
        if (!sp.length || sp.length !== path.length) return false;
        for (let i = 0; i < path.length; ++i) {
            if (path[i].index !== sp[i].index) return false;
            if (i < path.length - 1) {
                const a = (path[i].slot === undefined || path[i].slot === null) ? null : path[i].slot;
                const b = (sp[i].slot === undefined || sp[i].slot === null) ? null : sp[i].slot;
                if (a !== b) return false;
            }
        }
        const mouthSlot = (sp[path.length - 1].slot === undefined || sp[path.length - 1].slot === null) ? null : sp[path.length - 1].slot;
        return mouthSlot === k;
    }
    function mouthAdjustment(k) {
        const preview = snapLayout();
        const grow = (preview && preview.snapGrowAmt !== undefined) ? preview.snapGrowAmt : Math.max(0, ((dragState ? (dragState.snapHeight || 58) : 58) - 8));
        const shrink = (dragState && (dragState.active || dragState.settling) && typeof dragState.sourceShrink === "number") ? dragState.sourceShrink : 0;
        return (isSnapMouth(k) ? grow : 0) - (isSourceMouth(k) ? shrink : 0);
    }
    readonly property var effMouthHeights: {
        const n = slotCount();
        const out = [];
        for (let k = 0; k < n; ++k) {
            const slot = mouthSlots.itemAt(k);
            out.push(Math.max(emptyMouth, (mouthHeights[k] || emptyMouth) + (slot ? slot.adjustment : mouthAdjustment(k))));
        }
        return out;
    }
    readonly property real mouthTotal: { let sum = 0; const e = effMouthHeights; for (let k = 0; k < slotCount(); ++k) sum += e[k] || emptyMouth; return sum; }
    readonly property real bodyWidth: mouthWidths.reduce((m, w) => Math.max(m, w), 0)
    function setMouth(i, height, width, flat) {
        if (mouthHeights[i] === height && mouthWidths[i] === width && flatEnds[i] === flat) return;
        const h = mouthHeights.slice(), w = mouthWidths.slice(), f = flatEnds.slice();
        h[i] = height; w[i] = width; f[i] = flat;
        mouthHeights = h; mouthWidths = w; flatEnds = f;
    }
    function mouthTop(k) { let y = headHeight; const e = effMouthHeights; for (let i = 0; i < k; ++i) y += (e[i] || emptyMouth) + midHeight; return y; }
    function lastIsCap(slot) { const b = body(slot); return b.length > 0 && BlockRegistry.isCap(b[b.length - 1], blockDefinitions); }

    // Where a press grabs this block: its own bars, spine and connector tabs, but
    // not the hollow mouths (those belong to the blocks nested in them or the canvas).
    function inRect(x, y, rx, ry, rw, rh) { return x >= rx && x < rx + rw && y >= ry && y < ry + rh; }
    function hitTest(x, y) {
        const w = width, t = tabDepth;
        if (!isWrap) {
            const bottomTab = !isCap, body = bottomTab ? height - t : height;
            if (inRect(x, y, 0, 0, w, body)) return !(!isHeader && inRect(x, y, 13, 0, 30, t));
            return bottomTab && inRect(x, y, 13, body, 30, t);
        }
        if (inRect(x, y, 0, 0, w, headHeight)) return !inRect(x, y, 13, 0, 30, t);
        if (inRect(x, y, spine + 13, headHeight, 30, t)) return true;
        const n = slotCount();
        let top = headHeight;
        for (let k = 0; k < n; ++k) {
            const mh = mouthHeights[k] || emptyMouth, barTop = top + mh, foot = k === n - 1;
            if (inRect(x, y, 0, top, spine, mh)) return true;
            const barH = foot ? footHeight : midHeight;
            if (inRect(x, y, 0, barTop, w, barH)) return !(inRect(x, y, spine + 13, barTop, 30, t) && !flatEnds[k]);
            if (!(foot && isCap) && inRect(x, y, foot ? 13 : spine + 13, barTop + barH, 30, t)) return true;
            top = barTop + barH;
        }
        return false;
    }

    function iconFor(t) {
        if (isCustomBlock && instruction) {
            const d = defFor(instruction.block_id);
            if (d && d.icon) return d.icon;
        }
        return BlockRegistry.iconFor(t);
    }
    function fieldLocation(fieldId) { return {kind:"Field",strand_id:strandId,index:path,field_id:fieldId,path:[]}; }
    function cloneInstruction() { return JSON.parse(JSON.stringify(instruction)); }
    function setField(name,value) { const n=cloneInstruction(); n[name]=value; instructionEdited(strandId,path,n); }
    function childPath(slot,index) { const p=JSON.parse(JSON.stringify(path)); if(p.length) p[p.length-1].slot=slot; p.push({index:index}); return p; }
    function body(slot) { return BlockRegistry.body(instruction, slot); }
    function slotCount() { return type === "BranchCallBlock" ? BlockRegistry.slotCount(instruction) : Math.max(1, BlockRegistry.slotCount(instruction)); }
    // Palette prefabs have no strand, so their slots edit the prefab itself.
    // A value piece names its slot by `key`, or by `key` plus `index` when
    // the instruction keeps its slots in an array (a plugin block's args).
    function slotValue(piece) {
        const v = instruction ? instruction[piece.key] : undefined;
        return piece.index === undefined ? v : (v ? v[piece.index] : undefined);
    }
    function setLeaf(key, path, leaf, index) {
        const n = cloneInstruction();
        const holder = index === undefined ? n : n[key];
        const slot = index === undefined ? key : index;
        if (!path.length) holder[slot] = leaf;
        else { let v = holder[slot]; for (let i = 0; i < path.length - 1; ++i) v = v.args[path[i]]; v.args[path[path.length - 1]] = leaf; }
        instructionEdited(strandId, root.path, n);
    }
    function pieceOptions(piece) { return BlockRegistry.choices(piece.options, instruction); }
    // A dropdown or text piece with an `index` edits one entry of the array
    // held under `key`, the way a value piece does.
    function pieceRaw(piece) { const v = instruction ? instruction[piece.key] : undefined; return piece.index === undefined ? v : (v ? v[piece.index] : undefined); }
    function pieceValue(piece) { const v = pieceRaw(piece); return piece.decode ? piece.decode(v) : (typeof v === "string" ? v : (v === undefined || v === null ? "" : String(v))); }
    function setPieceField(piece, value) {
        if (piece.index === undefined) { setField(piece.key, value); return; }
        const n = cloneInstruction();
        if (typeof piece.index === "string") {
            // A named entry of an object, a component's own fields say.
            n[piece.key] = Object.assign({}, n[piece.key] || {}, { [piece.index]: value });
            instructionEdited(strandId, path, n);
            return;
        }
        const list = Array.isArray(n[piece.key]) ? n[piece.key] : [];
        while (list.length <= piece.index) list.push("");
        list[piece.index] = value;
        n[piece.key] = list;
        instructionEdited(strandId, path, n);
    }
    function addComponent(component) { instructionEdited(strandId, path, BlockRegistry.withComponent(component, instruction)); }
    function removeComponent(component) { instructionEdited(strandId, path, BlockRegistry.withoutComponent(component, instruction)); }
    function setPiece(piece, chosen) { setPieceField(piece, piece.encode ? piece.encode(chosen, instruction) : chosen); }
    function defFor(id) { for(let i=0;i<(blockDefinitions||[]).length;i++) if(blockDefinitions[i].id===id) return blockDefinitions[i]; return null; }
    function callLabel() { const d=instruction?defFor(instruction.block_id):null; if(!d) return qsTr("custom block"); return (d.pieces||[]).filter(p=>p.kind!=="Branch").map(p=>p.kind==="Label"?p.text:("("+p.name+")")).join(" "); }
    function callHeadPieces(){
        const d=instruction?defFor(instruction.block_id):null;if(!d)return [{kind:"Label",text:qsTr("(deleted block)"),argIndex:-1}];
        const first=(d.pieces||[]).findIndex(p=>p.kind==="Branch");let arg=0;const out=[];
        for(let i=0;i<d.pieces.length;i++){const p=d.pieces[i];if(p.kind==="Input"){out.push({kind:"Input",text:p.name,argIndex:arg,bool:p.value_type==="Bool"});arg++;}else if(p.kind==="Label"&&(first<0||i<first))out.push({kind:"Label",text:p.text,argIndex:-1,bool:false});}
        return out;
    }
    function listNames(){return (lists||[]).map(l=>l.name);}
    function branchSeparator(slot){const d=instruction?defFor(instruction.block_id):null;if(!d)return "";let seen=-1;for(let i=0;i<(d.pieces||[]).length;i++){const p=d.pieces[i];if(p.kind!=="Branch")continue;seen++;if(seen===slot){for(let j=i+1;j<d.pieces.length;j++){if(d.pieces[j].kind==="Branch")break;if(d.pieces[j].kind==="Label")return d.pieces[j].text;}}}return "";}
    function headerPieces(){const d=instruction?defFor(instruction.block_id):null;if(!d)return [{kind:"Label",text:qsTr("(deleted block)")}];return (d.pieces||[]).filter(p=>p.kind!=="Branch");}
    function headerGroups() {
        const groups = [];
        let current = null;
        const d = instruction ? defFor(instruction.block_id) : null;
        if (!d) return [{component: false, pieces: [{kind: "Label", text: qsTr("(deleted block)")}]}];
        for (const p of d.pieces || []) {
            if (p.kind === "Branch") { current = null; continue; }
            if (p.component) { current = {component: true, pieces: [p]}; groups.push(current); }
            else if (current) current.pieces.push(p);
            else groups.push({component: false, pieces: [p]});
        }
        return groups;
    }
    function headerBranches(){const d=instruction?defFor(instruction.block_id):null;return d?(d.pieces||[]).filter(p=>p.kind==="Branch"):[];}
    // A definition header with branches embeds a call-site preview: a C-block whose mouths
    // hold one draggable "run branch" piece each. Keyed by a string so edits elsewhere don't rebuild it.
    readonly property string branchKey: { blockDefinitions; return type === "BlockHeader" ? JSON.stringify(headerBranches().map((b, i) => ({ n: b.name, s: branchSeparator(i) }))) : "[]"; }
    readonly property var branchModel: JSON.parse(branchKey)
    readonly property bool hasBranchHeader: branchModel.length > 0
    readonly property real pvTop: 10
    readonly property real pvHead: 50
    readonly property real pvMouth: 50
    readonly property real pvMid: 34
    readonly property real pvFoot: 26
    readonly property string pvShape: { blockDefinitions; const d = type === "BlockHeader" && instruction ? defFor(instruction.block_id) : null; return d ? (d.shape || "Normal") : "Normal"; }
    // Operators wear a pill / hexagon shell and have no connectors of their own.
    readonly property bool pvReporter: pvShape === "ReturnsValue" || pvShape === "ReturnsBool"
    readonly property bool pvBool: pvShape === "ReturnsBool"
    readonly property bool pvNoTab: pvReporter || pvShape === "Ending"
    readonly property real pvPadX: pvReporter ? (pvBool ? pvOuterHeight / 2 : 22) : 0
    readonly property real pvPadY: pvReporter ? 12 : 0
    readonly property real pvHeight: pvHead + branchModel.length * pvMouth + Math.max(0, branchModel.length - 1) * pvMid + pvFoot + (pvNoTab ? 0 : tabDepth)
    readonly property real pvOuterHeight: pvHeight + 2 * pvPadY
    property real pvPieceMax: 0
    readonly property real pvWidth: Math.max(218, fields.implicitWidth + 42, spine + pvPieceMax + 24)
    readonly property real pvOuterWidth: pvWidth + 2 * pvPadX
    function recalcPieceMax() {
        let m = 0;
        for (let i = 0; i < pvPieces.count; ++i) { const it = pvPieces.itemAt(i); if (it) m = Math.max(m, it.pieceWidth); }
        pvPieceMax = m;
    }

    BlockSurface {
        id: surface
        anchors.fill: parent
        shape: root.isWrap ? "wrap" : root.isHeader ? "header" : root.isCap ? "cap" : "stack"
        fill: root.displayColor; hovered: root.hovered || root.dragRole === 1 || root.flashing || root.running || root.pausedHere
        hoverColor: root.pausedHere ? Theme.warning : root.running ? Theme.running : root.flashing && root.markText ? Theme.danger : root.type === "RunBranch" ? root.branchColor : root.isCustomBlock ? root.customColor : Theme.accent
        outlineWidth: root.running || root.pausedHere ? 2.6 : 1.3
        headHeight: root.headHeight; midHeight: root.midHeight; footHeight: root.footHeight; spine: root.spine
        mouthHeights: root.isWrap ? root.effMouthHeights : []
        footTab: !(root.isWrap && root.isCap)
        flatEnds: root.flatEnds
    }
    // Raise highlighted outlines without covering a nested block's hover outline.
    Shape {
        anchors.fill: parent
        z: 1
        visible: root.isWrap && surface.hovered
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: surface.hovered ? surface.hoverColor : surface.outline
            strokeWidth: surface.outlineWidth
            joinStyle: ShapePath.RoundJoin
            fillColor: "transparent"
            PathSvg { path: surface.outlinePath }
        }
    }
    QtObject { id: hitMask; function contains(point: point): bool { return root.hitTest(point.x, point.y); } }
    BlockDragArea {
        id: grab
        anchors.fill: parent
        containmentMask: hitMask
        dragEnabled: root.paletteMode || !root.locked
        onDragBegan: (sx, sy, ox, oy) => root.dragBegan(root.strandId, root.path, root.tailCount, sx, sy, ox, oy, root.implicitWidth, root.implicitHeight)
        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
        onDragCanceled: root.dragCanceled()
        onActivated: root.activated()
        onTapped: { root.tapped(); if (!root.paletteMode) root.insertionRequested(root.strandId, root.path); }
        onContextRequested: (x, y) => root.openMenu(x, y)
    }
    // Built on first use: a menu per block costs more than the block itself.
    property var contextMenu: null
    function openMenu(x, y) {
        if (!contextMenu) contextMenu = blockMenuComponent.createObject(grab);
        contextMenu.popup(x, y);
    }
    Component {
        id: blockMenuComponent
        BwMenu {
            id: menu
            BwMenuItem { visible: !root.paletteMode && root.isCustomBlock && root.type === "BlockHeader"; iconName: "blocks"; text: qsTr("Edit block"); onTriggered: root.menuActionRequested("edit_block", root.strandId, root.path, root.instruction) }
            BwMenuItem { visible: !root.paletteMode && BlockRegistry.recordingTargets; iconName: "target"; text: root.isRecordingTarget ? qsTr("Recording Target (current)") : qsTr("Set Recording Target"); onTriggered: root.recordingTargetRequested(root.strandId) }
            BwMenuItem { visible: !root.paletteMode; iconName: "copy"; text: qsTr("Copy block"); onTriggered: root.menuActionRequested("copy", root.strandId, root.path, root.instruction) }
            BwMenuItem { visible: !root.paletteMode; iconName: "layers"; text: qsTr("Copy all"); onTriggered: root.menuActionRequested("copy_all", root.strandId, root.path, root.instruction) }
            BwMenuItem { visible: !root.paletteMode; iconName: "corner-down-right"; text: qsTr("Duplicate block"); onTriggered: root.duplicateRequested(root.strandId, root.path, root.instruction) }
            BwMenuItem { visible: !root.paletteMode; iconName: "scissors"; text: qsTr("Cut block"); onTriggered: root.menuActionRequested("cut", root.strandId, root.path, root.instruction) }
            BwMenuItem { visible: !root.paletteMode; iconName: "message-square"; text: root.hasComment ? qsTr("Comment") : qsTr("Add Comment"); onTriggered: root.commentRequested(root.instruction) }
            BwMenuItem { visible: !root.paletteMode || !root.extraMenu || root.extraMenu.length === 0; iconName: "info"; text: qsTr("Details"); onTriggered: root.detailsRequested(root.isCustomBlock && root.instruction ? root.instruction.block_id : root.type) }
            BwMenuItem { id: deleteItem; visible: !root.paletteMode; iconName: "trash"; danger: true; text: qsTr("Delete block"); onTriggered: root.removeRequested(root.strandId, root.path) }
            Instantiator {
                model: root.paletteMode ? root.extraMenu : BlockRegistry.blockMenu
                delegate: BwMenuItem {
                    required property var modelData
                    iconName: modelData.icon || ""; text: modelData.text; danger: !!modelData.danger
                    onTriggered: !root.paletteMode ? root.menuActionRequested(modelData.id, root.strandId, root.path, root.instruction) : modelData.id === "details" ? root.detailsRequested(root.isCustomBlock && root.instruction ? root.instruction.block_id : root.type) : root.extraMenuTriggered(modelData.id)
                }
                onObjectAdded: (index, object) => root.paletteMode ? menu.addItem(object) : menu.insertItem(index, object)
                onObjectRemoved: (index, object) => menu.removeItem(object)
            }
        }
    }
    function fieldsComponent(t) {
        switch (t) {
        case "WhenRan": return fieldsFor0;
        case "WhenBatteryDischargedTo": return fieldsFor1;
        case "WhenBatteryChargedTo": return fieldsFor2;
        case "WhenTime": return fieldsFor3;
        case "WhenPowerPluggedIn": return fieldsFor4;
        case "WhenPowerUnplugged": return fieldsFor5;
        case "WhenClipboardChanged": return fieldsFor6;
        case "Wait": return fieldsFor7;
        case "Text": return fieldsFor8;
        case "Key": return fieldsFor9;
        case "WhenKey": return fieldsForWhenKey;
        case "Button": return fieldsFor10;
        case "MoveMouse": return fieldsFor11;
        case "Scroll": return fieldsFor12;
        case "Command": return fieldsFor13;
        case "OpenApp": return fieldsFor14;
        case "CloseApp": return fieldsFor15;
        case "SetVariable": return fieldsFor16;
        case "ChangeVariable": return fieldsFor17;
        case "SetClipboard": return fieldsFor18;
        case "AddToList": return fieldsFor19;
        case "DeleteOfList": return fieldsFor20;
        case "DeleteAllOfList": return fieldsFor21;
        case "ShiftList": return fieldsFor22;
        case "InsertIntoList": return fieldsFor24;
        case "ReplaceItemOfList": return fieldsFor25;
        case "ReverseList": return fieldsFor26;
        case "Return": return fieldsFor27;
        case "EscapeLoop": return fieldsFor28;
        case "ContinueLoop": return fieldsFor29;
        default: return null;
        }
    }
    Component { id: fieldsFor0
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN RAN"); color:Theme.text; font.pixelSize:12; font.weight:Font.DemiBold; font.letterSpacing:1; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    Component { id: fieldsFor1
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN BATTERY DISCHARGED TO"); color:Theme.text; font.pixelSize:11; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { valueData:instruction?instruction.threshold:null; location:root.fieldLocation("BatteryDischargeThreshold"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    Text { text:"%"; color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    Component { id: fieldsFor2
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN BATTERY CHARGED TO"); color:Theme.text; font.pixelSize:11; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { valueData:instruction?instruction.threshold:null; location:root.fieldLocation("BatteryChargeThreshold"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    Text { text:"%"; color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    Component { id: fieldsFor3
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN"); color:Theme.text; font.pixelSize:11; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:[qsTr("every day"),qsTr("weekly"),qsTr("monthly"),qsTr("yearly")]; implicitWidth:104; implicitHeight:30; font.pixelSize:12 }
                    Text { text:qsTr("at"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwTextField { anchors.verticalCenter:parent.verticalCenter; text: instruction&&instruction.schedule ? String(instruction.schedule.hour).padStart(2,"0")+":"+String(instruction.schedule.minute).padStart(2,"0") : "09:00"; implicitWidth:72; implicitHeight:30; font.pixelSize:12 }
        }
    }
    Component { id: fieldsFor4
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN POWER PLUGGED IN"); color:Theme.text; font.pixelSize:11; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    Component { id: fieldsFor5
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN POWER UNPLUGGED"); color:Theme.text; font.pixelSize:11; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    Component { id: fieldsFor6
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN CLIPBOARD CHANGES"); color:Theme.text; font.pixelSize:11; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    Component { id: fieldsFor7
        Row {
            spacing: 7
                    Text { text:qsTr("Wait"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Rectangle {
                        anchors.verticalCenter:parent.verticalCenter
                        implicitHeight:Math.max(27,coordWait.implicitHeight+4); implicitWidth:coordLabelWait.implicitWidth+coordWait.implicitWidth+21
                        width:implicitWidth; height:implicitHeight; radius:5; color:Theme.field
                        border.color:coordWait.dropHighlighted?Theme.accent:Theme.border
                        Text { id:coordLabelWait; x:7; text:qsTr("MS"); color:Theme.text; font.pixelSize:10; font.weight:Font.DemiBold; font.letterSpacing:0.5; anchors.verticalCenter:parent.verticalCenter }
                        ValueChip { id:coordWait; x:coordLabelWait.width+14; anchors.verticalCenter:parent.verticalCenter; flatLeaf:true; valueData:instruction?instruction.duration:null; location:root.fieldLocation("WaitDuration"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    }
        }
    }
    Component { id: fieldsFor8
        Row {
            spacing: 7
                    Text { text:qsTr("Text"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.text:null; location:root.fieldLocation("TextValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
        }
    }
    Component { id: fieldsFor9
        Row {
            spacing: 7
                    Text { text:qsTr("Key"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; keyMode:true; keyCapturing:root.isCapturingKey(); onKeyCaptureRequested:root.keyCaptureRequested(root.strandId,root.path); valueData:instruction?instruction.key:null; location:root.fieldLocation("KeyValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:[qsTr("Click"),qsTr("Press"),qsTr("Release")]; currentIndex: instruction?["Click","Press","Release"].indexOf(instruction.direction):0; implicitWidth:88; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("direction",["Click","Press","Release"][index]) }
        }
    }
    Component { id: fieldsForWhenKey
        Row {
            spacing: 7
                    Text { text:qsTr("WHEN"); color:Theme.text; font.pixelSize:11; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; keyMode:true; keyCapturing:root.isCapturingKey(); onKeyCaptureRequested:root.keyCaptureRequested(root.strandId,root.path); valueData:instruction?instruction.key:null; location:root.fieldLocation("KeyValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:[qsTr("Pressed"),qsTr("Released")]; currentIndex: instruction&&instruction.direction==="Release"?1:0; implicitWidth:104; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("direction",index===1?"Release":"Press") }
        }
    }
    Component { id: fieldsFor10
        Row {
            spacing: 7
                    Text { text:qsTr("Mouse button"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:[qsTr("Left"),qsTr("Right"),qsTr("Middle"),qsTr("Side"),qsTr("Extra")]; currentIndex:instruction?["Left","Right","Middle","Side","Extra"].indexOf(instruction.button):0; implicitWidth:82; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("button",["Left","Right","Middle","Side","Extra"][index]) }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:[qsTr("Click"),qsTr("Press"),qsTr("Release")]; currentIndex:instruction?["Click","Press","Release"].indexOf(instruction.direction):0; implicitWidth:88; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("direction",["Click","Press","Release"][index]) }
        }
    }
    Component { id: fieldsFor11
        Row {
            spacing: 7
                    Text { text:qsTr("Move mouse"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:[qsTr("Relative"),qsTr("Absolute")]; currentIndex:instruction&&instruction.coordinate==="Absolute"?1:0; implicitWidth:88; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("coordinate",["Relative","Absolute"][index]) }
                    Rectangle {
                        anchors.verticalCenter:parent.verticalCenter
                        implicitHeight:Math.max(27,coordX.implicitHeight+4); implicitWidth:coordLabelX.implicitWidth+coordX.implicitWidth+21
                        width:implicitWidth; height:implicitHeight; radius:5; color:Theme.field
                        border.color:coordX.dropHighlighted?Theme.accent:Theme.border
                        Text { id:coordLabelX; x:7; text:qsTr("X"); color:Theme.text; font.pixelSize:10; font.weight:Font.DemiBold; font.letterSpacing:0.5; anchors.verticalCenter:parent.verticalCenter }
                        ValueChip { id:coordX; x:coordLabelX.width+14; anchors.verticalCenter:parent.verticalCenter; flatLeaf:true; valueData:instruction?instruction.x:null; location:root.fieldLocation("MoveMouseX"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    }
                    Rectangle {
                        anchors.verticalCenter:parent.verticalCenter
                        implicitHeight:Math.max(27,coordY.implicitHeight+4); implicitWidth:coordLabelY.implicitWidth+coordY.implicitWidth+21
                        width:implicitWidth; height:implicitHeight; radius:5; color:Theme.field
                        border.color:coordY.dropHighlighted?Theme.accent:Theme.border
                        Text { id:coordLabelY; x:7; text:qsTr("Y"); color:Theme.text; font.pixelSize:10; font.weight:Font.DemiBold; font.letterSpacing:0.5; anchors.verticalCenter:parent.verticalCenter }
                        ValueChip { id:coordY; x:coordLabelY.width+14; anchors.verticalCenter:parent.verticalCenter; flatLeaf:true; valueData:instruction?instruction.y:null; location:root.fieldLocation("MoveMouseY"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    }
        }
    }
    Component { id: fieldsFor12
        Row {
            spacing: 7
                    Text { text:qsTr("Scroll"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Rectangle {
                        anchors.verticalCenter:parent.verticalCenter
                        implicitHeight:Math.max(27,coordScroll.implicitHeight+4); implicitWidth:coordLabelScroll.implicitWidth+coordScroll.implicitWidth+21
                        width:implicitWidth; height:implicitHeight; radius:5; color:Theme.field
                        border.color:coordScroll.dropHighlighted?Theme.accent:Theme.border
                        Text { id:coordLabelScroll; x:7; text:qsTr("AMOUNT"); color:Theme.text; font.pixelSize:10; font.weight:Font.DemiBold; font.letterSpacing:0.5; anchors.verticalCenter:parent.verticalCenter }
                        ValueChip { id:coordScroll; x:coordLabelScroll.width+14; anchors.verticalCenter:parent.verticalCenter; flatLeaf:true; valueData:instruction?instruction.amount:null; location:root.fieldLocation("ScrollAmount"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:[qsTr("Vertical"),qsTr("Horizontal")]; currentIndex:instruction&&instruction.axis==="Horizontal"?1:0; implicitWidth:92; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("axis",["Vertical","Horizontal"][index]) }
        }
    }
    Component { id: fieldsFor13
        Row {
            spacing: 7
                    Text { text:qsTr("Run command"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwTextField { anchors.verticalCenter:parent.verticalCenter; text:instruction&&instruction.command?instruction.command:""; placeholderText:qsTr("command"); implicitWidth:Math.max(92,Math.min(480,contentWidth+22)); implicitHeight:30; font.pixelSize:12; onEditingFinished:root.setField("command",text) }
        }
    }
    Component { id: fieldsFor14
        Row {
            spacing: 7
                    Text { text:qsTr("Open app"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwButton { anchors.verticalCenter:parent.verticalCenter; text:instruction&&instruction.name?instruction.name:qsTr("Choose app..."); iconSource:instruction&&instruction.icon?instruction.icon:""; iconName:instruction&&!instruction.icon&&instruction.name?"app-window":""; implicitHeight:30; font.pixelSize:12; onClicked: root.appPickerRequested(root.strandId, root.path, root.instruction) }
        }
    }
    Component { id: fieldsFor15
        Row {
            spacing: 7
                    Text { text:qsTr("Close app"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwButton { anchors.verticalCenter:parent.verticalCenter; text:instruction&&instruction.name?instruction.name:qsTr("Choose app..."); iconSource:instruction&&instruction.icon?instruction.icon:""; iconName:instruction&&!instruction.icon&&instruction.name?"app-window":""; implicitHeight:30; font.pixelSize:12; onClicked: root.appPickerRequested(root.strandId, root.path, root.instruction) }
        }
    }
    Component { id: fieldsFor16
        Row {
            spacing: 7
                    Text { text:qsTr("set"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:Array.from(root.variables||[]); currentIndex:instruction?Array.from(root.variables||[]).indexOf(instruction.name):-1; displayText:currentIndex>=0?currentText:(instruction&&instruction.name?instruction.name:qsTr("Choose variable")); implicitWidth:106; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("name",currentText) }
                    Text { text:qsTr("to"); color:Theme.text; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.value:null; location:root.fieldLocation("SetVariableValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
        }
    }
    Component { id: fieldsFor17
        Row {
            spacing: 7
                    Text { text:qsTr("change"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    BwComboBox { anchors.verticalCenter:parent.verticalCenter; model:Array.from(root.variables||[]); currentIndex:instruction?Array.from(root.variables||[]).indexOf(instruction.name):-1; displayText:currentIndex>=0?currentText:(instruction&&instruction.name?instruction.name:qsTr("Choose variable")); implicitWidth:106; implicitHeight:30; font.pixelSize:12; onActivated:index=>root.setField("name",currentText) }
                    Text { text:qsTr("by"); color:Theme.text; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.value:null; location:root.fieldLocation("ChangeVariableValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
        }
    }
    Component { id: fieldsFor18
        Row {
            spacing: 7
                    Text { text:qsTr("set clipboard to"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.value:null; location:root.fieldLocation("SetClipboardValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
        }
    }
    Component { id: fieldsFor19
        Row {
            spacing: 7
                    Text { text:qsTr("add"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.value:null; location:root.fieldLocation("AddToListValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    Text { text:qsTr("to"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: fieldsFor23 }
        }
    }
    Component { id: fieldsFor20
        Row {
            spacing: 7
                    Text { text:qsTr("delete"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.index:null; location:root.fieldLocation("DeleteOfListIndex"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    Text { text:qsTr("of"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: fieldsFor23 }
        }
    }
    Component { id: fieldsFor21
        Row {
            spacing: 7
                    Text { text:qsTr("delete all of"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: fieldsFor23 }
        }
    }
    Component { id: fieldsFor22
        Row {
            spacing: 7
                    Text { text:qsTr("shift"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: fieldsFor23 }
                    Text { text:qsTr("by"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.amount:null; location:root.fieldLocation("ShiftListAmount"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
        }
    }
    Component {
        id: listDropdown
        BwComboBox {
            readonly property var choices: BlockRegistry.listChoices(root.listNames())
            model: choices; textRole: "label"
            currentIndex: choices.findIndex(c => c.value === (root.instruction ? root.instruction.name : ""))
            displayText: currentIndex >= 0 ? currentText : (root.instruction && root.instruction.name ? root.instruction.name : qsTr("Choose list"))
            implicitWidth: 98; implicitHeight: 30; font.pixelSize: 12
            onActivated: index => {
                const choice = choices[index];
                if (choice.action) choice.action(name => root.setField("name", name));
                else root.setField("name", choice.value);
            }
        }
    }
    Component { id: fieldsFor23
        Row {
            spacing: 7
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: listDropdown }
        }
    }
    Component { id: fieldsFor24
        Row {
            spacing: 7
                    Text { text:qsTr("insert"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.value:null; location:root.fieldLocation("InsertIntoListValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    Text { text:qsTr("at"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.index:null; location:root.fieldLocation("InsertIntoListIndex"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    Text { text:qsTr("of"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: listDropdown }
        }
    }
    Component { id: fieldsFor25
        Row {
            spacing: 7
                    Text { text:qsTr("replace item"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.index:null; location:root.fieldLocation("ReplaceItemOfListIndex"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                    Text { text:qsTr("of"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: listDropdown }
                    Text { text:qsTr("with"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.value:null; location:root.fieldLocation("ReplaceItemOfListValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
        }
    }
    Component { id: fieldsFor26
        Row {
            spacing: 7
                    Text { text:qsTr("reverse"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    Loader { anchors.verticalCenter: parent.verticalCenter; sourceComponent: listDropdown }
        }
    }
    Component { id: fieldsFor27
        Row {
            spacing: 7
                    Text { text:qsTr("return"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                    ValueChip { anchors.verticalCenter:parent.verticalCenter; valueData:instruction?instruction.value:null; location:root.fieldLocation("ReturnValue"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
        }
    }
    Component { id: fieldsFor28
        Row {
            spacing: 7
                    Text { text:qsTr("break loop"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    Component { id: fieldsFor29
        Row {
            spacing: 7
                    Text { text:qsTr("continue loop"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    // Embedded call-site preview for a header with branches (see hasBranchHeader).
    Item {
        id: pvRoot
        visible: root.hasBranchHeader
        x: 14; y: root.pvTop; width: root.pvOuterWidth; height: root.pvOuterHeight
        Shape {
            visible: root.pvReporter
            id: pvShell
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer
            readonly property string outline: {
                const w = width, h = height;
                if (w <= 0 || h <= 0) return "";
                if (root.pvBool) {
                    const n = Math.min(w / 2, h / 2);
                    return "M" + n + " .5 L" + (w - n) + " .5 L" + (w - .5) + " " + h / 2 + " L" + (w - n) + " " + (h - .5) + " L" + n + " " + (h - .5) + " L.5 " + h / 2 + " Z";
                }
                const r = Math.min(24, h / 2), R = w - .5, B = h - .5;
                return "M" + (.5 + r) + " .5 L" + (R - r) + " .5 Q" + R + " .5 " + R + " " + (.5 + r) + " L" + R + " " + (B - r) + " Q" + R + " " + B + " " + (R - r) + " " + B
                    + " L" + (.5 + r) + " " + B + " Q.5 " + B + " .5 " + (B - r) + " L.5 " + (.5 + r) + " Q.5 .5 " + (.5 + r) + " .5 Z";
            }
            ShapePath {
                strokeColor: Qt.rgba(root.customColor.r * .3 + Theme.border.r * .7, root.customColor.g * .3 + Theme.border.g * .7, root.customColor.b * .3 + Theme.border.b * .7, 1)
                strokeWidth: 1.3
                joinStyle: ShapePath.RoundJoin
                fillColor: Theme.block
                PathSvg { path: pvShell.outline }
            }
        }
        Item {
            id: pvInner
            x: root.pvPadX; y: root.pvPadY; width: root.pvWidth; height: root.pvHeight
        BlockSurface {
            anchors.fill: parent
            shape: "wrap"; fill: Theme.block; outline: Theme.border; hoverColor: root.customColor
            headNotch: !root.pvReporter; footTab: !root.pvNoTab
            headHeight: root.pvHead; midHeight: root.pvMid; footHeight: root.pvFoot; spine: root.spine
            mouthHeights: root.branchModel.map(() => root.pvMouth)
        }
        Repeater {
            model: root.branchModel
            delegate: Text {
                required property var modelData
                required property int index
                visible: index < root.branchModel.length - 1 && modelData.s.length > 0
                x: root.spine + 8; y: root.pvHead + index * (root.pvMouth + root.pvMid) + root.pvMouth + Math.round((root.pvMid - height) / 2)
                text: modelData.s; color: Theme.text; font.pixelSize: 12
            }
        }
        Repeater {
            id: pvPieces
            model: root.branchModel
            delegate: Item {
                id: pv
                required property var modelData
                required property int index
                readonly property real pieceWidth: Math.max(150, pvRow.implicitWidth + 28)
                onPieceWidthChanged: root.recalcPieceMax()
                Component.onCompleted: root.recalcPieceMax()
                x: root.spine; y: root.pvHead + index * (root.pvMouth + root.pvMid)
                width: pieceWidth; height: root.pvMouth + root.tabDepth
                BlockSurface {
                    anchors.fill: parent
                    shape: "stack"; fill: Theme.panelRaised; outline: Theme.border; hoverColor: root.customColor
                    hovered: pvDrag.hovered
                }
                Row {
                    id: pvRow
                    x: 14; y: Math.round((root.pvMouth - height) / 2); spacing: 6
                    LucideIcon { name: "git-branch"; color: root.customColor; width: 16; height: 16; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: pv.modelData.n; color: Theme.text; font.pixelSize: 12; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
                }
                BlockDragArea {
                    id: pvDrag
                    anchors.fill: parent
                    onDragBegan: (sx, sy, ox, oy) => root.paletteDragStarted({ kind: "branch", color: root.customColor, instruction: { id: "palette-branch", type: "RunBranch", name: pv.modelData.n } }, sx, sy, ox, oy)
                    onDragMoved: (sx, sy) => root.paletteDragMoved(sx, sy)
                    onDragEnded: (sx, sy) => root.paletteDragEnded(sx, sy)
                    onDragCanceled: root.paletteDragCanceled()
                    onTapped: root.runBranchRequested(root.strandId, root.path, pv.modelData.n)
                }
            }
        }
        }
    }
    Row {
        id: fields; visible: !root.isWrap; x: root.hasBranchHeader ? 32 + root.pvPadX : 14; y: (root.hasBranchHeader ? root.pvTop + root.pvPadY : 0) + Math.round((root.rowHeight-height-8)/2); spacing: root.type === "RunBranch" ? 6 : 7
        LucideIcon { visible: !root.isWrap; name: root.isWrap ? "" : root.iconFor(root.type); color: root.type === "RunBranch" ? root.branchColor : root.isCustomBlock ? root.customColor : root.isHeader ? Theme.accent : Theme.text; width: 16; height: 16; anchors.verticalCenter: parent.verticalCenter }
        Loader {
            active: !!root.type && !root.isWrap && !root.rowSpec && !root.isCustomBlock && root.type !== "RunBranch"; visible: active; anchors.verticalCenter: parent.verticalCenter
            sourceComponent: root.fieldsComponent(root.type)
        }
        // Custom block calls and definitions, drawn from their definition.
        Loader {
            active: !root.rowSpec && (root.type === "CallBlock" || root.type === "BlockHeader" || root.type === "RunBranch"); visible: active; anchors.verticalCenter: parent.verticalCenter
            sourceComponent: Row {
                spacing: 7
                Repeater { model:root.type==="CallBlock"?root.callHeadPieces():[];delegate:Item {
                    required property var modelData;width:modelData.kind==="Label"?callPieceText.implicitWidth:callPieceValue.implicitWidth;height:30
                    Text{id:callPieceText;visible:modelData.kind==="Label";text:modelData.text;color:Theme.text;font.pixelSize:12;font.weight:Font.DemiBold;anchors.centerIn:parent}
                    ValueChip{id:callPieceValue;visible:modelData.kind==="Input";valueData:instruction&&(instruction.args||[])[modelData.argIndex]?(instruction.args||[])[modelData.argIndex]:(modelData.bool?{kind:"Bool"}:{kind:"Number",value:0});location:root.fieldLocation("CallArg:"+modelData.argIndex);boxed:!!modelData.bool;anchors.centerIn:parent;onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled()}
                }}
                Repeater {
                    model: root.type === "BlockHeader" ? root.headerGroups() : []
                    delegate: Rectangle {
                        id: headerGroup
                        required property var modelData
                        objectName: modelData.component ? "definitionComponentGroup" : "definitionBasePiece"
                        width: headerGroupRow.implicitWidth + (modelData.component ? 16 : 0)
                        height: modelData.component ? 40 : 30
                        anchors.verticalCenter: parent.verticalCenter
                        radius: 7
                        color: modelData.component ? Theme.panelRaised : "transparent"
                        border.width: modelData.component ? 1 : 0
                        border.color: Theme.border
                        Row {
                            id: headerGroupRow
                            anchors.centerIn: parent
                            spacing: 7
                            Repeater {
                                model: headerGroup.modelData.pieces
                                delegate: Item {
                                    required property var modelData;width:modelData.kind==="Label"?headerText.implicitWidth:headerParam.implicitWidth;height:30
                                    Text{id:headerText;visible:modelData.kind==="Label";text:modelData.text||"";color:Theme.text;font.pixelSize:12;font.weight:Font.DemiBold;anchors.centerIn:parent}
                                    PaletteValue{id:headerParam;visible:modelData.kind==="Input";anchors.centerIn:parent;editable:false;valueData:({kind:"Param",name:modelData.name||""});forceBoolean:modelData.value_type==="Bool";referenceColor:root.customColor;blockDefinitions:root.blockDefinitions
                                        spec:({kind:"value",value:valueData,forceBoolean:forceBoolean,color:root.customColor,originBlockId:root.instruction?root.instruction.block_id:null})
                                        onDragStarted:(sp,sx,sy,ox,oy)=>root.paletteDragStarted(sp,sx,sy,ox,oy);onDragMoved:(sx,sy)=>root.paletteDragMoved(sx,sy);onDragEnded:(sx,sy)=>root.paletteDragEnded(sx,sy);onDragCanceled:root.paletteDragCanceled()}
                                }
                            }
                        }
                    }
                }
                Text { visible:root.type==="RunBranch"; text:instruction?(instruction.name||""):""; color:Theme.text; font.pixelSize:12; font.weight:Font.DemiBold; anchors.verticalCenter:parent.verticalCenter }
            }
        }
        Repeater { model: root.isWrap ? [] : root.headGroups; delegate: pieceDelegate }
    }

    // Head bar content, then one mouth per body (nested blocks) with separator bars between them.
    Row {
        id:headContent; visible:root.isWrap; x:14; y:Math.round((root.headHeight-height)/2); spacing:7
        LucideIcon { visible:root.isWrap; name:root.isWrap?root.iconFor(root.type):""; color:root.isCustomBlock?root.customColor:Theme.text; width:16;height:16; anchors.verticalCenter:parent.verticalCenter }
        Loader {
            active: root.isWrap && !root.rowSpec && root.type !== "BranchCallBlock"; visible: active; anchors.verticalCenter: parent.verticalCenter
            sourceComponent: Row {
                spacing: 7
                Text { visible:root.type==="If"||root.type==="IfElse"; text:qsTr("if"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                ValueChip { visible:root.type==="If"||root.type==="IfElse"||root.type==="While"; valueData:instruction?instruction.condition:null; location:root.fieldLocation("Condition"); boxed:true; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() }
                Text { visible:root.type==="If"||root.type==="IfElse"; text:qsTr("then"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                Text { visible:root.type==="Repeat"; text:qsTr("repeat"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                Loader { active:root.type==="Repeat"; visible:active; sourceComponent:Component { ValueChip { valueData:instruction?instruction.count:null; location:root.fieldLocation("RepeatCount"); boxed:false; onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled() } } }
                Text { visible:root.type==="Forever"; text:qsTr("forever"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
                Text { visible:root.type==="While"; text:qsTr("while"); color:Theme.text; font.pixelSize:12; anchors.verticalCenter:parent.verticalCenter }
            }
        }
        Repeater { model: root.isWrap ? root.headGroups : []; delegate: pieceDelegate }
        Repeater { model:root.type==="BranchCallBlock" && !root.rowSpec ?root.callHeadPieces():[];delegate:Item {
            required property var modelData;width:modelData.kind==="Label"?branchPieceText.implicitWidth:branchPieceValue.implicitWidth;height:30
            Text{id:branchPieceText;visible:modelData.kind==="Label";text:modelData.text;color:Theme.text;font.pixelSize:12;font.weight:Font.DemiBold;anchors.centerIn:parent}
            ValueChip{id:branchPieceValue;visible:modelData.kind==="Input";valueData:instruction&&(instruction.args||[])[modelData.argIndex]?(instruction.args||[])[modelData.argIndex]:(modelData.bool?{kind:"Bool"}:{kind:"Number",value:0});location:root.fieldLocation("CallArg:"+modelData.argIndex);boxed:!!modelData.bool;anchors.centerIn:parent;onEditRequested:(l,t)=>root.valueEdited(l,t); blockDefinitions:root.blockDefinitions; onValueDragBegan:(loc,val,sx,sy,ox,oy,w,h)=>root.valueDragBegan(loc,val,sx,sy,ox,oy,w,h); onValueDragMoved:(sx,sy)=>root.valueDragMoved(sx,sy); onValueDragEnded:(sx,sy)=>root.valueDragEnded(sx,sy); onValueDragCanceled:()=>root.valueDragCanceled()}
        }}
    }

    Repeater {
        id: mouthSlots
        model: root.isWrap ? root.slotCount() : 0
        delegate: Item {
            id: slotDelegate; required property int index
            property real adjustment: root.mouthAdjustment(index)
            Behavior on adjustment {
                enabled: root.animateLayout
                NumberAnimation { duration: Motion.dur(140); easing.type: Easing.OutCubic }
            }
            readonly property real contentHeight: slotBody.implicitHeight
            readonly property bool capEnd: root.lastIsCap(index)
            // the block sitting last in a mouth overlaps the bar below by its connector tab
            readonly property real mouthHeight: Math.max(root.emptyMouth, contentHeight - (capEnd || contentHeight === 0 ? 0 : root.tabDepth))
            // Measure the remaining children and the incoming snap preview live.
            readonly property real contentWidth: {
                let widest = 0;
                for (let i = 0; i < childBlocks.count; ++i) {
                    const child = childBlocks.itemAt(i);
                    if (child && child.item && child.item.dragRole !== 1)
                        widest = Math.max(widest, child.item.implicitWidth);
                }
                const preview = root.snapLayout();
                if (root.isSnapMouth(index) && preview)
                    widest = Math.max(widest, preview.snapWidth || 0);
                return widest;
            }
            function report() { root.setMouth(index, mouthHeight, contentWidth, capEnd); }
            onMouthHeightChanged: report()
            onContentWidthChanged: report()
            onCapEndChanged: report()
            Component.onCompleted: report()
            x: 0; y: root.mouthTop(index); width: root.width; height: mouthHeight
            Column {
                id: slotBody; x: root.spine; spacing: -8
                InstructionList { id: slotBlocks; instructions: root.body(slotDelegate.index) }
                Repeater {
                    id: childBlocks
                    model: slotBlocks
                    delegate: Loader {
                        id: child
                        required property string payload; required property int index
                        readonly property var blockData: JSON.parse(payload)
                        width: item ? item.implicitWidth : 0; height: item ? item.implicitHeight : 0
                        // Supply bindings at creation to avoid rebuilding controls for each property.
                        Component.onCompleted: setSource("InstructionBlock.qml", {
                            instruction: Qt.binding(() => child.blockData),
                            strandId: Qt.binding(() => root.strandId),
                            path: Qt.binding(() => root.childPath(slotDelegate.index, child.index)),
                            tailCount: Qt.binding(() => slotBlocks.count - child.index),
                            variables: Qt.binding(() => root.variables),
                            lists: Qt.binding(() => root.lists),
                            blockDefinitions: Qt.binding(() => root.blockDefinitions),
                            branchColor: Qt.binding(() => root.branchColor),
                            keyCapture: Qt.binding(() => root.keyCapture),
                            locked: Qt.binding(() => root.locked),
                            dragState: Qt.binding(() => root.dragState)
                        })
                        onLoaded: {
                            item.removeRequested.connect((s, p) => root.removeRequested(s, p));
                            item.duplicateRequested.connect((s, p, i) => root.duplicateRequested(s, p, i));
                            item.commentRequested.connect(i => root.commentRequested(i));
                            item.recordingTargetRequested.connect(s => root.recordingTargetRequested(s));
                            item.instructionEdited.connect((s, p, i) => root.instructionEdited(s, p, i));
                            item.valueEdited.connect((l, t) => root.valueEdited(l, t));
                            item.valueDragBegan.connect((loc, val, sx, sy, ox, oy, w, h) => root.valueDragBegan(loc, val, sx, sy, ox, oy, w, h));
                            item.valueDragMoved.connect((sx, sy) => root.valueDragMoved(sx, sy));
                            item.valueDragEnded.connect((sx, sy) => root.valueDragEnded(sx, sy));
                            item.valueDragCanceled.connect(() => root.valueDragCanceled());
                            item.runBranchRequested.connect((s, p, n) => root.runBranchRequested(s, p, n));
                            item.keyCaptureRequested.connect((s, p) => root.keyCaptureRequested(s, p));
                            item.appPickerRequested.connect((s, p, i) => root.appPickerRequested(s, p, i));
                            item.detailsRequested.connect(t => root.detailsRequested(t));
                            item.menuActionRequested.connect((a, s, p, i) => root.menuActionRequested(a, s, p, i));
                            item.pieceActionRequested.connect((a, s, p, i) => root.pieceActionRequested(a, s, p, i));
                            item.paletteDragStarted.connect((sp, sx, sy, ox, oy) => root.paletteDragStarted(sp, sx, sy, ox, oy));
                            item.paletteDragMoved.connect((sx, sy) => root.paletteDragMoved(sx, sy));
                            item.paletteDragEnded.connect((sx, sy) => root.paletteDragEnded(sx, sy));
                            item.paletteDragCanceled.connect(() => root.paletteDragCanceled());
                            item.insertionRequested.connect(root.insertionRequested);
                            item.dragBegan.connect(root.dragBegan); item.dragMoved.connect(root.dragMoved);
                            item.dragEnded.connect(root.dragEnded); item.dragCanceled.connect(root.dragCanceled);
                        }
                    }
                }
            }
            // separator bar below this mouth (not for the last: that is the foot bar)
            Text {
                visible: slotDelegate.index < root.slotCount() - 1
                x: root.spine + 8; y: root.effMouthHeights[slotDelegate.index] + Math.round((root.midHeight - height) / 2)
                text: root.type === "BranchCallBlock" ? root.branchSeparator(slotDelegate.index) : BlockRegistry.separator(root.type, slotDelegate.index)
                color: Theme.text; font.pixelSize: 12
            }
        }
    }

    // One piece of a registered row: a label, a value slot, a dropdown, a
    // text field or a button. Only the control the piece needs is built.
    Component {
        id: pieceDelegate
        Loader {
            id: piece
            required property var modelData
            readonly property string kind: modelData.kind
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined
            sourceComponent: kind === "label" ? labelPiece : kind === "value" ? valuePiece : kind === "dropdown" ? dropdownPiece : kind === "button" ? buttonPiece
                : kind === "component" ? componentPiece : kind === "addComponent" ? addComponentPiece : kind === "removeComponent" ? removeComponentPiece : textPiece
            Component {
                id: componentPiece
                Rectangle {
                    objectName: "componentCapsule"
                    readonly property var componentSpec: piece.modelData.component
                    implicitWidth: componentRow.implicitWidth + 16
                    implicitHeight: componentRow.implicitHeight + 8
                    radius: 7
                    color: Theme.panelRaised
                    border.color: Theme.border
                    Accessible.role: Accessible.Grouping
                    Accessible.name: qsTr("Component: %1").arg(componentSpec.label || "")
                    Row {
                        id: componentRow
                        anchors.centerIn: parent; spacing: 7
                        Repeater { model: piece.modelData.pieces; delegate: pieceDelegate }
                    }
                }
            }
            Component {
                id: labelPiece
                // Event (header) labels use the same caption style as the
                // built-in event blocks above, so every hat reads alike.
                Text { text: piece.modelData.text || ""; color: Theme.text; font.pixelSize: root.isHeader ? 11 : 12; font.weight: root.isHeader ? Font.DemiBold : Font.Normal; height: 30; verticalAlignment: Text.AlignVCenter }
            }
            Component {
                id: valuePiece
                ValueChip {
                    valueData: root.instruction ? (root.slotValue(piece.modelData) || (piece.modelData.bool ? { kind: "Bool" } : { kind: "Number", value: 0 })) : null
                    location: root.paletteMode ? null : root.fieldLocation(piece.modelData.field)
                    leafOptions: piece.modelData.options ? BlockRegistry.choices(piece.modelData.options, root.instruction) : null
                    leafPlaceholder: piece.modelData.placeholder || qsTr("choose")
                    pickAction: piece.modelData.pickAction || ""
                    pickOnClick: !piece.modelData.expression || !root.instruction || !root.instruction[piece.modelData.expression]
                    boxed: !!piece.modelData.bool; forceBoolean: !!piece.modelData.bool && (!valueData || valueData.kind === "Bool")
                    pick: piece.modelData.pick || null
                    blockDefinitions: root.blockDefinitions
                    onEditRequested: (l, t) => root.valueEdited(l, t)
                    onLeafEdited: (p, leaf) => root.setLeaf(piece.modelData.key, p, leaf, piece.modelData.index)
                    onDetailsRequested: kind => root.detailsRequested(kind)
                    onValueDragBegan: (loc, val, sx, sy, ox, oy, w, h) => root.valueDragBegan(loc, val, sx, sy, ox, oy, w, h)
                    onValueDragMoved: (sx, sy) => root.valueDragMoved(sx, sy)
                    onValueDragEnded: (sx, sy) => root.valueDragEnded(sx, sy)
                    onValueDragCanceled: root.valueDragCanceled()
                }
            }
            Component {
                id: dropdownPiece
                BwComboBox {
                    id: pieceDrop
                    readonly property var choices: root.pieceOptions(piece.modelData)
                    readonly property string current: root.pieceValue(piece.modelData)
                    readonly property int chosen: { for (let i = 0; i < choices.length; ++i) if (choices[i].value === current) return i; return -1; }
                    model: choices; textRole: "label"; currentIndex: chosen
                    displayText: chosen >= 0 ? choices[chosen].label : (current.length ? current : (piece.modelData.placeholder || "choose"))
                    implicitWidth: Math.min(190, Math.max(56, dropMetrics.advanceWidth + 36)); implicitHeight: 28; font.pixelSize: 12
                    leftPadding: 8; rightPadding: 26
                    TextMetrics { id: dropMetrics; font: pieceDrop.font; text: pieceDrop.displayText }
                    onActivated: index => {
                        const choice = pieceDrop.choices[index];
                        if (choice.action) choice.action(value => root.setPiece(piece.modelData, value));
                        else root.setPiece(piece.modelData, choice.value);
                    }
                }
            }
            // { kind: "button", action, text, icon, image: key, palette } - with
            // `image`, shows that field's picture (a data: URI) as a thumbnail;
            // with `palette`, it can also be clicked in the palette.
            Component {
                id: buttonPiece
                Rectangle {
                    id: pieceButton
                    readonly property string picture: piece.modelData.image && root.instruction ? (root.instruction[piece.modelData.image] || "") : ""
                    readonly property string label: typeof piece.modelData.text === "function" ? piece.modelData.text(root.instruction) : (piece.modelData.text || "")
                    readonly property bool clickable: !root.paletteMode || !!piece.modelData.palette
                    implicitWidth: buttonRow.implicitWidth + 14; implicitHeight: picture ? 34 : 28; radius: 6
                    color: buttonMouse.containsMouse && clickable ? Theme.panelRaised : Theme.field
                    border.color: buttonMouse.containsMouse && clickable ? Theme.accent : Theme.border
                    Row {
                        id: buttonRow
                        anchors.centerIn: parent; spacing: 6
                        Image {
                            visible: pieceButton.picture.length > 0
                            source: pieceButton.picture
                            readonly property real aspect: implicitHeight > 0 ? implicitWidth / implicitHeight : 1
                            height: 26; width: Math.min(110, Math.max(14, height * aspect))
                            fillMode: Image.PreserveAspectFit; smooth: false; anchors.verticalCenter: parent.verticalCenter
                        }
                        LucideIcon {
                            visible: !pieceButton.picture && !!piece.modelData.icon
                            name: piece.modelData.icon || "info"; width: 14; height: 14; color: Theme.text
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Text {
                            visible: pieceButton.label.length > 0
                            text: pieceButton.label; color: Theme.text; font.pixelSize: 12
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                    MouseArea {
                        id: buttonMouse
                        anchors.fill: parent; hoverEnabled: true; enabled: pieceButton.clickable
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.pieceActionRequested(piece.modelData.action || "", root.strandId, root.path, root.instruction)
                    }
                }
            }
            // The row's "+": a menu of the components the block doesn't have yet.
            Component {
                id: addComponentPiece
                Rectangle {
                    id: addButton
                    objectName: "addComponentButton"
                    implicitWidth: 22; implicitHeight: 22; radius: 11
                    color: addMouse.containsMouse ? Theme.panelRaised : "transparent"
                    border.color: addMouse.containsMouse ? Theme.accent : Theme.border
                    LucideIcon { anchors.centerIn: parent; name: "plus"; width: 13; height: 13; color: addMouse.containsMouse ? Theme.accent : Theme.textDim }
                    ToolTip.visible: addMouse.containsMouse; ToolTip.delay: 400; ToolTip.text: qsTr("Add an option")
                    MouseArea {
                        id: addMouse
                        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: addMenu.popup(0, addButton.height + 2)
                    }
                    BwMenu {
                        id: addMenu
                        objectName: "addComponentMenu"
                        Instantiator {
                            model: BlockRegistry.missingComponents(root.rowSpec, root.instruction)
                            delegate: BwMenuItem {
                                required property var modelData
                                iconName: modelData.icon || "plus"; text: modelData.label
                                onTriggered: root.addComponent(modelData)
                            }
                            onObjectAdded: (index, object) => addMenu.insertItem(index, object)
                            onObjectRemoved: (index, object) => addMenu.removeItem(object)
                        }
                    }
                }
            }
            // The "x" that ends a component's pieces.
            Component {
                id: removeComponentPiece
                Item {
                    objectName: "removeComponentButton"
                    implicitWidth: 24; implicitHeight: 22
                    Rectangle {
                        width: 1; height: parent.height; color: Theme.border
                    }
                    LucideIcon { anchors.centerIn: parent; anchors.horizontalCenterOffset: 3; name: "x"; width: 12; height: 12; color: removeMouse.containsMouse ? Theme.danger : Theme.textDim }
                    ToolTip.visible: removeMouse.containsMouse; ToolTip.delay: 400
                    ToolTip.text: qsTr("Remove %1").arg(piece.modelData.component.label || "")
                    MouseArea {
                        id: removeMouse
                        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: root.removeComponent(piece.modelData.component)
                    }
                }
            }
            Component {
                id: textPiece
                BwTextField {
                    text: root.pieceValue(piece.modelData); placeholderText: piece.modelData.placeholder || ""
                    implicitWidth: Math.min(piece.modelData.maxWidth || 200, Math.max(piece.modelData.minWidth || 64, contentWidth + 20)); implicitHeight: 28; font.pixelSize: 12
                    leftPadding: 7; rightPadding: 7; topPadding: 2; bottomPadding: 2
                    onEditingFinished: if (text !== root.pieceValue(piece.modelData)) root.setPieceField(piece.modelData, text)
                }
            }
        }
    }

    Rectangle {
        visible: root.recordingTarget && !root.paletteMode
        z: 20; width: 8; height: 8; radius: 4; color: Theme.danger
        anchors.top: parent.top; anchors.topMargin: 6; anchors.right: parent.right; anchors.rightMargin: 8
        ToolTip.visible: dotHover.hovered; ToolTip.text: qsTr("Recording target")
        HoverHandler { id: dotHover }
    }

    Rectangle {
        visible: root.hasBreakpoint
        z: 30; width: 14; height: 14; radius: 7; color: Theme.danger
        border.width: 2; border.color: Theme.canvas
        x: -7; y: Math.max(4, Math.min(root.headHeight, root.height) / 2 - 7)
        ToolTip.visible: breakpointHover.hovered; ToolTip.delay: 300; ToolTip.text: qsTr("Breakpoint: runs pause here. Click to remove.")
        HoverHandler { id: breakpointHover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: BlockRegistry.breakpointClicked(root.instruction.id) }
    }

    Rectangle {
        visible: root.markText.length > 0
        z: 30; width: 18; height: 18; radius: 9; color: Theme.danger
        border.width: 2; border.color: Theme.canvas
        x: root.width - 12; y: -6
        Text { anchors.centerIn: parent; text: "!"; color: "white"; font.pixelSize: 11; font.weight: Font.Black }
        ToolTip.visible: markHover.hovered; ToolTip.delay: 200; ToolTip.text: root.markText
        HoverHandler { id: markHover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: BlockRegistry.markClicked(root.instruction.id) }
    }

    // Readable cap label for a captured key (a character or a web key code).
    function keyLabel(k) {
        if (k === " " || k === "Space") return "Space";
        const named = {
            ArrowLeft: "\u2190", ArrowRight: "\u2192", ArrowUp: "\u2191", ArrowDown: "\u2193",
            Escape: "Esc", Delete: "Del", Backspace: "Backspace", Enter: "Enter", NumpadEnter: "Enter", Tab: "Tab",
            Control: "Ctrl", ControlLeft: "Ctrl", ControlRight: "Ctrl", Shift: "Shift", ShiftLeft: "Shift", ShiftRight: "Shift",
            Alt: "Alt", AltLeft: "Alt", AltRight: "Alt", Meta: "Super", MetaLeft: "Super", MetaRight: "Super",
            PageUp: "PgUp", PageDown: "PgDn", Insert: "Ins", CapsLock: "Caps", Minus: "-", Equal: "=", Comma: ",", Period: ".",
            Slash: "/", Backslash: "\\", Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]", Backquote: "`"
        };
        if (named[k] !== undefined) return named[k];
        const m = /^(?:Key|Digit)(.)$/.exec(k);
        if (m) return m[1].toUpperCase();
        return k.length === 1 ? k.toUpperCase() : k;
    }
    function isCapturingKey() {
        if ((type !== "Key" && type !== "WhenKey") || !keyCapture) return false;
        if (paletteMode) return keyCapture.kind === "Standalone";
        return keyCapture.kind === "Strand" && keyCapture.strand_id === strandId && samePath(keyCapture.index, path);
    }
    // Paths from the daemon carry an explicit `slot: null` on every step; ours omit it.
    function samePath(a, b) {
        if (!a || !b || a.length !== b.length) return false;
        for (let i = 0; i < a.length; ++i)
            if (a[i].index !== b[i].index || (a[i].slot === undefined || a[i].slot === null ? -1 : a[i].slot) !== (b[i].slot === undefined || b[i].slot === null ? -1 : b[i].slot)) return false;
        return true;
    }
}
