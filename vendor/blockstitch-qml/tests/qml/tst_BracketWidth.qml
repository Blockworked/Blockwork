import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    width: 1800; height: 1000
    BlockCanvas { id: canvas; anchors.fill: parent }
    TestCase {
        name: "BracketWidth"
        when: windowShown

        function wide(id) { return { id: id, type: "Wait", duration: { kind: "Text", value: "a very wide block that should resize all enclosing brackets immediately" } }; }
        function init() {
            canvas.cancelDrag(); canvas.endSettle();
            canvas.strands = [{ id: "source", x: 40, y: 100, instructions: [
                { id: "outer", type: "While", body: [
                    { id: "inner", type: "IfElse", then_body: [{ id: "prefix", type: "Wait" }, wide("wide")], else_body: [] }
                ] }
            ] }, { id: "target", x: 400, y: 100, instructions: [
                { id: "target-outer", type: "While", body: [{ id: "target-inner", type: "If", body: [] }] }
            ] }];
            wait(350);
        }
        function block(id) { return canvas.findBlockItem(canvas, id); }
        function test_sourceShrinksBeforeRelease() {
            const outer = block("outer"), inner = block("inner"), child = block("wide");
            const outerWidth = outer.width, innerWidth = inner.width;
            const start = child.mapToItem(null, 4, 4);
            canvas.beginDrag("source", [{ index: 0, slot: 0 }, { index: 0, slot: 0 }, { index: 1 }], 1,
                start.x, start.y, 4, 4, child.width, child.height);
            tryVerify(() => inner.width < innerWidth, 200);
            tryVerify(() => outer.width < outerWidth);
            verify(canvas.branchDragState.active);
            canvas.cancelDrag();
            tryCompare(inner, "width", innerWidth);
            tryCompare(outer, "width", outerWidth);
        }
        function test_widthAnimatesDuringDrag() {
            const inner = block("inner"), child = block("wide");
            const initialWidth = inner.width;
            const start = child.mapToItem(null, 4, 4);
            canvas.beginDrag("source", [{ index: 0, slot: 0 }, { index: 0, slot: 0 }, { index: 1 }], 1,
                start.x, start.y, 4, 4, child.width, child.height);
            compare(inner.width, initialWidth);
            wait(40);
            const intermediateWidth = inner.width;
            verify(intermediateWidth < initialWidth);
            wait(160);
            verify(inner.width < intermediateWidth);
            verify(canvas.branchDragState.active);
            canvas.cancelDrag();
            wait(40);
            verify(inner.width > intermediateWidth);
            verify(inner.width < initialWidth);
            tryCompare(inner, "width", initialWidth);
        }
        function test_incomingPreview_data() {
            return [
                { tag: "canvas", palette: false },
                { tag: "canvas-wide-tail", palette: false, tail: true },
                { tag: "palette", palette: true }
            ];
        }
        function test_incomingPreview(data) {
            const outer = block("target-outer"), inner = block("target-inner"), child = block("wide");
            const outerWidth = outer.width, innerWidth = inner.width;
            const childWidth = child.width;
            if (!data.palette) {
                const first = data.tail ? block("prefix") : child;
                const start = first.mapToItem(null, 4, 4);
                canvas.beginDrag("source", [{ index: 0, slot: 0 }, { index: 0, slot: 0 }, { index: data.tail ? 0 : 1 }], data.tail ? 2 : 1,
                    start.x, start.y, 4, 4, first.width, first.height);
            }
            const end = inner.mapToItem(null, inner.spine + 4, inner.headHeight + 4);
            if (data.palette) {
                const at = canvas.workspacePoint(end.x, end.y);
                canvas.updatePaletteSnap(at.x - 4, at.y - 4, wide("palette"), childWidth, child.height);
            } else canvas.moveDrag(end.x, end.y);
            compare(canvas.branchDragState.snapTargetId, "target");
            compare(canvas.branchDragState.snapPath, [{ index: 0, slot: 0 }, { index: 0, slot: 0 }, { index: 0 }]);
            tryVerify(() => inner.width >= inner.spine + childWidth + 24, 200);
            tryVerify(() => outer.width >= outer.spine + inner.width + 24);
            canvas.clearSnap();
            tryCompare(inner, "width", innerWidth);
            tryCompare(outer, "width", outerWidth);
        }
    }
}
