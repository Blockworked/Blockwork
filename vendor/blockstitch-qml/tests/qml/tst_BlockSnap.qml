import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    width: 1200; height: 900
    BlockCanvas { id: canvas; anchors.fill: parent }
    SignalSpy { id: merged; target: canvas; signalName: "strandsMerged" }
    SignalSpy { id: tailMerged; target: canvas; signalName: "tailMerged" }

    TestCase {
        name: "BlockSnap"
        when: windowShown

        function block(id, type) { return { id: id, type: type || "If", body: [] }; }
        function strand(id, x, y, instructions) { return { id: id, x: x, y: y, instructions: instructions }; }
        function init() {
            canvas.cancelDrag(); canvas.endSettle();
            canvas.setZoom(1);
            canvas.strands = [strand("target", 40, 240, [block("target-block")])];
            merged.clear(); tailMerged.clear();
            wait(30);
        }
        function container(id, nested) {
            return canvas.collectContainers().find(c => c.strandId === id && c.basePath.length === (nested ? 1 : 0));
        }
        function snap(c, top, height, cap) {
            return canvas.findSnap(c.x, top, null, [], false, null, height, !!cap);
        }
        function test_above_wrap() {
            const c = container("target");
            const height = canvas.blockHeight(block("dragged"));
            verify(height - 8 > canvas.snapThreshold);
            const result = snap(c, c.y - height + 8, height);
            verify(!!result);
            compare(result.targetId, "target");
            compare(result.path, [{ index: 0 }]);
            canvas.applySnap(result, 200, height, false);
            compare(canvas.branchDragState.snapY, c.y - height + 8);
            compare(canvas.branchDragState.snapShiftAmt, 0);
            compare(canvas.branchDragState.snapTargetY, 240 - height + 8);
            const target = canvas.findBlockItem(canvas, "target-block");
            compare(target.snapShift, 0);
        }
        function test_covering_wrap_does_not_prepend() {
            const c = container("target");
            const height = canvas.blockHeight(block("dragged"));
            const result = snap(c, c.y, height);
            verify(!result || result.path[0].index !== 0);
            canvas.updatePaletteSnap(c.x - canvas.originOffsetX, c.y - canvas.originOffsetY,
                block("palette"), 200, height);
            verify(!canvas.paletteSnapValid || canvas.paletteSnapPath[0].index !== 0);
        }
        function test_prepend_preview_stays_near_dragged_stack() {
            const c = container("target");
            const height = canvas.blockHeight(block("dragged"));
            for (let top = c.y - height; top <= c.y + height; top += 2) {
                const result = snap(c, top, height);
                if (result && result.path.length === 1 && result.path[0].index === 0)
                    verify(Math.abs(result.y - top) <= canvas.snapThreshold);
            }
        }
        function test_below_still_snaps() {
            const c = container("target");
            const result = snap(c, c.y + c.heights[0] - 8, 110);
            verify(!!result);
            compare(result.path, [{ index: 1 }]);
        }
        function test_above_nested_block() {
            canvas.strands = [strand("target", 40, 240, [{ id: "wrap", type: "If", body: [block("child")] }])];
            wait(20);
            const c = container("target", true);
            const result = snap(c, c.y - 102, 110);
            verify(!!result);
            compare(result.path, [{ index: 0, slot: 0 }, { index: 0 }]);
        }
        function test_drag_up_through_stack_data() {
            return [{ tag: "plain", header: false }, { tag: "with-header", header: true }];
        }
        function test_drag_up_through_stack(data) {
            const instructions = [];
            for (let i = 0; i < 8; ++i)
                instructions.push(block("row-" + i, i === 0 && data.header ? "WhenStarted" : "Key"));
            canvas.strands = [strand("target", 40, 240, instructions)];
            wait(20);
            const c = container("target");
            const step = c.heights[1] - 8;
            const height = step * 2.5 + 8;
            let previous = null;
            // Carry the sticky candidate through small upward pointer moves.
            for (let top = c.y + step * 5; top >= c.y + step; top -= 2) {
                const result = canvas.findSnap(c.x, top, null, [], false, previous, height, false);
                verify(!!result);
                verify(Math.abs(result.y - top) <= canvas.snapThreshold,
                    "Preview jumped below the pointer at " + top + " to " + result.y);
                previous = result;
            }
        }
        function test_definition_before_loop_data() {
            return [
                { tag: "palette", palette: true },
                { tag: "canvas", palette: false },
                { tag: "nested-tail", palette: false, nested: true }
            ];
        }
        function test_definition_before_loop(data) {
            const body = [block("child", "Wait")];
            if (data.nested) body.push({ id: "condition", type: "If", body: [block("dragged", "Wait")] });
            const strands = [strand("target", 40, 240, [
                { id: "header", type: "BlockHeader", block_id: "custom" },
                { id: "loop", type: "While", body: body }
            ])];
            if (!data.nested) strands.push(strand("source", 600, 240, [block("dragged", "Wait")]));
            canvas.strands = strands;
            wait(30);
            const c = container("target");
            const top = c.y + c.heights[0] - 8 + 12;
            const first = canvas.findBlockItem(canvas, "dragged");
            if (!data.palette) {
                const start = first.mapToItem(null, 4, 4);
                const path = data.nested ? [{ index: 1, slot: 0 }, { index: 1, slot: 0 }, { index: 0 }] : [{ index: 0 }];
                canvas.beginDrag(data.nested ? "target" : "source", path, 1,
                    start.x, start.y, 4, 4, first.width, first.height);
            }
            // Start inside the loop so the previous candidate has snap hysteresis.
            const mouth = container("target", true);
            if (data.palette) canvas.updatePaletteSnap(mouth.x - canvas.originOffsetX,
                mouth.y - canvas.originOffsetY, block("palette", "Wait"), first.width, first.height);
            else {
                const at = canvas.findBlockItem(canvas, "loop").mapToItem(null, 24, 54);
                canvas.moveDrag(at.x, at.y);
            }
            compare(canvas.branchDragState.snapPath, [{ index: 1, slot: 0 }, { index: 0 }]);
            for (let x = 0; x < 10; ++x) {
                if (data.palette) canvas.updatePaletteSnap(c.x + x - canvas.originOffsetX,
                    top - canvas.originOffsetY, block("palette", "Wait"), first.width, first.height);
                else {
                    const target = canvas.findBlockItem(canvas, "header").mapToItem(null, x + 4, c.heights[0] - 8 + 16);
                    canvas.moveDrag(target.x, target.y);
                }
                compare(canvas.branchDragState.snapTargetId, "target");
                compare(canvas.branchDragState.snapPath, [{ index: 1 }]);
            }
            // Moving into the mouth's lane still allows prepending to its body.
            const nested = container("target", true);
            const result = snap(nested, nested.y - first.height + 8, first.height);
            compare(result.path, [{ index: 1, slot: 0 }, { index: 0 }]);
        }
        function test_header_rejects_above() {
            canvas.strands = [strand("target", 40, 240, [block("header", "WhenStarted")])];
            const c = container("target");
            compare(snap(c, c.y - 102, 110), null);
        }
        function test_cap_rejects_above_but_allows_append() {
            const c = container("target");
            compare(snap(c, c.y - 42, 50, true), null);
            compare(snap(c, c.y, 50, true), null);
            const result = snap(c, c.y + c.heights[0] - 8, 50, true);
            verify(!!result);
            compare(result.path, [{ index: 1 }]);
        }
        function test_cap_target_accepts_above() {
            canvas.strands = [strand("target", 40, 240, [block("cap", "Return")])];
            const c = container("target");
            compare(snap(c, c.y - 102, 110).path, [{ index: 0 }]);
            compare(snap(c, c.y + c.heights[0] - 8, 110), null);
        }
        function test_distance_limits() {
            const c = container("target");
            compare(snap(c, c.y - 110 + 8 - canvas.snapThreshold - 1, 110), null);
            compare(canvas.findSnap(c.x + canvas.snapThreshold + 1, c.y - 102, null, [], false, null, 110, false), null);
        }
        function test_palette_uses_bottom_connector() {
            const c = container("target");
            const instruction = block("palette");
            const height = canvas.blockHeight(instruction);
            canvas.updatePaletteSnap(c.x - canvas.originOffsetX, c.y - canvas.originOffsetY - height + 8, instruction, 200, height);
            verify(canvas.paletteSnapValid);
            compare(canvas.paletteSnapPath, [{ index: 0 }]);
            compare(canvas.paletteSnapTargetY, 240 - height + 8);
        }
        function test_canvas_stack_data() {
            return [
                { tag: "single", count: 1, nested: false, cap: false, zoom: 1 },
                { tag: "whole-stack", count: 2, nested: false, cap: false, zoom: 1 },
                { tag: "tail", count: 2, nested: true, cap: false, zoom: 1 },
                { tag: "zoomed", count: 2, nested: false, cap: false, zoom: 0.7 },
                { tag: "cap-tail", count: 2, nested: false, cap: true, zoom: 1 }
            ];
        }
        function test_canvas_stack(data) {
            const tail = [block("first")];
            if (data.count === 2) tail.push(block("last", data.cap ? "Return" : "If"));
            const source = data.nested ? [{ id: "source-wrap", type: "If", body: tail }] : tail;
            canvas.strands = canvas.strands.concat([strand("source", 400, 240, source)]);
            canvas.setZoom(data.zoom);
            wait(30);
            const path = data.nested ? [{ index: 0, slot: 0 }, { index: 0 }] : [{ index: 0 }];
            const first = canvas.findBlockItem(canvas, "first");
            const target = canvas.findBlockItem(canvas, "target-block");
            const before = target.mapToItem(null, 0, 0);
            const start = first.mapToItem(null, 4, 4);
            const height = canvas.contentHeightFor(tail);
            const end = target.mapToItem(null, 4, 4 - height + 8);
            canvas.beginDrag("source", path, data.count, start.x, start.y, 4, 4, first.width, first.height);
            canvas.moveDrag(end.x, end.y);
            compare(canvas.branchDragState.snapValid, !data.cap);
            if (data.cap) return;
            compare(canvas.branchDragState.snapTargetId, "target");
            compare(canvas.branchDragState.snapPath, [{ index: 0 }]);
            compare(canvas.branchDragState.snapShiftAmt, 0);
            compare(target.snapShift, 0);
            const targetY = 240 - height + 8;
            compare(canvas.branchDragState.snapTargetY, targetY);
            canvas.endDrag(end.x, end.y);
            const spy = data.nested ? tailMerged : merged;
            compare(spy.count, 1);
            compare(spy.signalArguments[0][data.nested ? 3 : 2], [{ index: 0 }]);
            compare(spy.signalArguments[0][data.nested ? 4 : 3], targetY);
            canvas.strands = [strand("target", 40, targetY, tail.concat([block("target-block")]))];
            wait(20);
            const stationary = canvas.findBlockItem(canvas, "target-block");
            const landed = stationary.mapToItem(null, 0, 0);
            const expected = canvas.findBlockItem(canvas, "first").mapToItem(null, 0, height - 8);
            compare(landed.y, expected.y);
            fuzzyCompare(landed.y, before.y, 0.01);
        }
    }
}
