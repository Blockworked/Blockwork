import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1200; height: 900
    BlockCanvas { id: canvas; anchors.fill: parent }
    property var observedBlock: null
    property int observedSlot: 0
    property var targets: []
    property real adjustment: observedBlock ? observedBlock.mouthAdjustment(observedSlot) : 0
    onAdjustmentChanged: targets = targets.concat([adjustment])

    TestCase {
        name: "BracketSnapAnimation"
        when: windowShown

        function init() {
            scene.observedBlock = null;
            canvas.cancelDrag(); canvas.endSettle();
            canvas.strands = [
                { id: "target", x: 40, y: 240, instructions: [
                    { id: "outer", type: "IfElse", then_body: [], else_body: [] }
                ] },
                { id: "source", x: 400, y: 240, instructions: [
                    { id: "inner", type: "If", body: [] }
                ] }
            ];
            wait(30);
        }
        function test_stackIntoWhile_data() {
            return [
                { tag: "empty", occupied: false, nested: false, cap: false },
                { tag: "occupied", occupied: true, nested: false, cap: false },
                { tag: "nested-source", occupied: false, nested: true, cap: false },
                { tag: "cap-tail", occupied: false, nested: false, cap: true },
                { tag: "quick-drop", occupied: false, nested: false, cap: false, quick: true }
            ];
        }
        function test_stackIntoWhile(data) {
            const tail = [
                { id: "detect", type: "Wait" },
                { id: "condition", type: "If", body: [{ id: "aim", type: "Wait" }] }
            ];
            if (data.cap) tail.push({ id: "stop", type: "Return" });
            const prefix = data.occupied ? [{ id: "prefix", type: "Wait" }] : [];
            canvas.strands = [
                { id: "target", x: 40, y: 240, instructions: [{ id: "outer", type: "While", body: prefix }] },
                { id: "source", x: 600, y: 240, instructions: data.nested ? [{ id: "home", type: "If", body: tail }] : tail }
            ];
            wait(30);
            const outer = canvas.findBlockItem(canvas, "outer");
            const first = canvas.findBlockItem(canvas, "detect");
            const initial = outer.effMouthHeights[0];
            const expected = canvas.mouthHeightFor(prefix.concat(tail));
            const start = first.mapToItem(null, 4, 4);
            const path = data.nested ? [{ index: 0, slot: 0 }, { index: 0 }] : [{ index: 0 }];
            canvas.beginDrag("source", path, tail.length, start.x, start.y, 4, 4, first.width, first.height);
            const end = outer.mapToItem(null, 24, outer.headHeight + (prefix.length ? canvas.contentHeightFor(prefix) - 8 : 0) + 4);
            canvas.moveDrag(end.x, end.y);
            compare(canvas.branchDragState.snapPath, [{ index: 0, slot: 0 }, { index: prefix.length }]);
            compare(outer.mouthAdjustment(0), expected - initial);
            if (!data.quick) {
                wait(180);
                compare(outer.effMouthHeights[0], expected);
            }
            canvas.endDrag(end.x, end.y);
            wait(200);
            compare(outer.effMouthHeights[0], expected);
            canvas.strands = [{ id: "target", x: 40, y: 240, instructions: [{ id: "outer", type: "While", body: prefix.concat(tail) }] }];
            wait(30);
            compare(canvas.findBlockItem(canvas, "outer").effMouthHeights[0], expected);
        }
        function test_enterEmptyBranch_data() {
            return [
                { tag: "canvas-then", palette: false, slot: 0 },
                { tag: "canvas-else", palette: false, slot: 1 },
                { tag: "palette-then", palette: true, slot: 0 },
                { tag: "palette-else", palette: true, slot: 1 }
            ];
        }
        function test_enterEmptyBranch(data) {
            const outer = canvas.findBlockItem(canvas, "outer");
            const inner = canvas.findBlockItem(canvas, "inner");
            const height = inner.implicitHeight;
            function move(x, y) {
                const p = outer.mapToItem(null, x + 4, y + 4);
                if (data.palette) {
                    const at = canvas.workspacePoint(p.x, p.y);
                    canvas.updatePaletteSnap(at.x - 4, at.y - 4,
                        { id: "palette", type: "If", body: [] }, inner.width, height);
                } else {
                    canvas.moveDrag(p.x, p.y);
                }
            }
            if (!data.palette) {
                const start = inner.mapToItem(null, 4, 4);
                canvas.beginDrag("source", [{ index: 0 }], 1,
                    start.x, start.y, 4, 4, inner.width, height);
            }
            // Pass the outer block's top connector before entering its branch.
            move(0, -height + 8);
            compare(canvas.branchDragState.snapPath, [{ index: 0 }]);
            scene.observedSlot = data.slot;
            scene.observedBlock = outer;
            scene.targets = [];
            const mouthTop = outer.mouthTop(data.slot);
            move(20, mouthTop);
            compare(canvas.branchDragState.snapPath, [{ index: 0, slot: data.slot }, { index: 0 }]);
            const grow = height - 8 - outer.emptyMouth;
            // A stale top-level gap would start the animation toward height - 8.
            compare(scene.targets, [grow]);
            let previous = outer.effMouthHeights[data.slot];
            for (let i = 0; i < 12; ++i) {
                wait(20);
                move(20, mouthTop);
                const current = outer.effMouthHeights[data.slot];
                verify(current >= previous, "The opening mouth must not reverse");
                verify(current <= outer.emptyMouth + grow);
                previous = current;
            }
            compare(scene.targets, [grow]);
            compare(previous, height - 8);
            canvas.clearSnap();
            compare(scene.targets, [grow, 0]);
            wait(180);
            compare(outer.effMouthHeights[data.slot], outer.emptyMouth);
        }
    }
}
