import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1100; height: 760
    property var edits: []
    property var replies: []
    // replies left pending, answered by the test
    property bool deferReplies: false
    property bool rejectAll: false
    function submit(edit, reply) {
        edits = edits.concat([JSON.parse(JSON.stringify(edit))]);
        if (deferReplies) replies = replies.concat([reply]);
        else reply(!rejectAll, rejectAll ? "no" : undefined);
    }
    function num(n) { return { kind: "Number", value: n }; }
    readonly property var pristine: [
        { id: "s1", x: 40, y: 40, instructions: [
            { id: "w1", type: "Wait", duration: num(100) },
            { id: "if1", type: "If", condition: { kind: "Bool" }, body: [{ id: "w2", type: "Wait", duration: num(200) }] }
        ] }
    ]
    property var strands: JSON.parse(JSON.stringify(pristine))
    property var floatingValues: []

    Rectangle { id: trash; x: 0; y: 0; width: 200; height: scene.height; color: "#333" }
    BlockCanvas {
        id: canvas
        x: 200; y: 0; width: scene.width - 200; height: scene.height
        strands: scene.strands
        floatingValues: scene.floatingValues
    }
    PaletteDragController {
        id: controller
        anchors.fill: parent
        canvas: canvas
        documentId: "doc-a"
        document: [scene.strands, scene.floatingValues]
        trashContains: (sx, sy) => trash.contains(trash.mapFromItem(null, sx, sy))
        submit: (edit, reply) => scene.submit(edit, reply)
    }

    TestCase {
        name: "PaletteDragController"
        when: windowShown
        readonly property var waitSpec: ({ kind: "instruction", type: "Wait", instruction: { id: "palette", type: "Wait", duration: { kind: "Number", value: 5 } } })
        readonly property var addSpec: ({ kind: "value", value: { kind: "Op", op: "Add", args: [{ kind: "Number", value: 1 }, { kind: "Number", value: 2 }] } })

        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function block(id) { return all(canvas).find(o => o.instruction && o.instruction.id === id && o.paletteMode === false && o.width > 0); }
        function chip(fieldId, strandId) {
            return all(canvas).find(o => o.isValueChip && o.visible && o.width > 0 && o.valueLocation && o.valueLocation.kind === "Field"
                && o.valueLocation.field_id === fieldId && JSON.stringify(o.valueLocation.index) === JSON.stringify([{ index: 0 }]));
        }
        function ghost() { return all(controller).find(o => o.objectName === "paletteGhost"); }
        function flick() { return all(canvas).find(o => o.contentX !== undefined && o.flickableDirection !== undefined); }
        function init() {
            scene.edits = []; scene.replies = []; scene.deferReplies = false; scene.rejectAll = false;
            canvas.cancelDrag(); canvas.cancelValueDrag();
            scene.floatingValues = [];
            controller.cancel(); controller.locked = false; controller.documentId = "doc-a";
            canvas.setZoom(1);
            scene.strands = JSON.parse(JSON.stringify(scene.pristine));
            wait(50);
        }
        // Drags spec with the ghost's top-left at (gx, gy) in scene coordinates.
        function dragTo(spec, gx, gy) {
            // Start well away from any gap: a shown gap is sticky for a few pixels.
            controller.begin(spec, 1000, 60, 6, 6);
            for (let i = 1; i <= 4; ++i) { controller.move(1000 + (gx + 6 - 1000) * i / 4, 60 + (gy + 6 - 60) * i / 4); wait(10); }
        }
        function drop(spec, gx, gy) { dragTo(spec, gx, gy); controller.end(gx + 6, gy + 6); wait(20); }
        function below(id) { const b = block(id); verify(!!b, id); return b.mapToItem(null, 0, b.height); }

        function test_above_keeps_lower_stack_fixed_data() {
            return [{ tag: "above", covering: false }, { tag: "covering", covering: true }];
        }
        function test_above_keeps_lower_stack_fixed(data) {
            const target = block("w1");
            const before = target.mapToItem(null, 0, 0);
            const height = canvas.blockHeight(waitSpec.instruction);
            dragTo(waitSpec, before.x, data.covering ? before.y : before.y - height + 8);
            if (data.covering) {
                verify(!canvas.paletteSnapValid || canvas.paletteSnapPath[0].index !== 0);
                controller.cancel();
                compare(scene.edits.length, 0);
                return;
            }
            verify(canvas.paletteSnapValid);
            fuzzyCompare(target.mapToItem(null, 0, 0).y, before.y, 0.01);
            controller.end(before.x + 6, (data.covering ? before.y : before.y - height + 8) + 6);
            wait(20);
            compare(scene.edits.length, 1);
            const edit = scene.edits[0];
            compare(edit.kind, "insert_instruction");
            compare(edit.path, [{ index: 0 }]);
            compare(edit.targetY, scene.pristine[0].y - height + 8);
            const during = target.mapToItem(null, 0, 0);
            fuzzyCompare(during.y, before.y, 0.01);
        }
        function test_top_level_gap() {
            drop(waitSpec, below("if1").x + 2, below("if1").y + 4);
            compare(scene.edits.length, 1);
            const e = scene.edits[0];
            compare(e.kind, "insert_instruction");
            compare(e.strandId, "s1");
            compare(e.path.length, 1);
            compare(e.path[0].index, 2);
            compare(e.instruction.type, "Wait");
            verify(e.instruction.id !== "palette", "a fresh id");
        }
        function test_nested_gap() {
            const w2 = below("w2");
            drop(waitSpec, w2.x + 2, w2.y + 2);
            compare(scene.edits.length, 1);
            compare(scene.edits[0].kind, "insert_instruction");
            compare(scene.edits[0].path.length, 2);
            compare(scene.edits[0].path[1].index, 1);
        }
        function test_open_canvas() {
            drop(waitSpec, 800, 600);
            compare(scene.edits.length, 1);
            const e = scene.edits[0];
            compare(e.kind, "add_strand");
            const at = canvas.workspacePoint(806, 606);
            compare(e.x, Math.round(at.x - 6));
            compare(e.y, Math.round(at.y - 6));
        }
        function test_value_into_slot() {
            const target = chip("WaitDuration");
            verify(!!target);
            const p = target.mapToItem(null, 0, 0);
            drop(addSpec, p.x, p.y);
            compare(scene.edits.length, 1);
            compare(scene.edits[0].kind, "put_value");
            compare(scene.edits[0].location.field_id, "WaitDuration");
            compare(scene.edits[0].value.op, "Add");
        }
        function test_value_onto_canvas() {
            drop({ kind: "value", value: { kind: "Param", name: "gain" }, originBlockId: "custom" }, 800, 600);
            compare(scene.edits.length, 1);
            compare(scene.edits[0].kind, "create_floating_value");
            compare(scene.edits[0].value.name, "gain");
            compare(scene.edits[0].originBlockId, "custom");
        }
        function test_cancel_outside_workspace() {
            dragTo(waitSpec, 600, 400);
            verify(ghost().visible);
            controller.end(50, 50);
            compare(scene.edits.length, 0);
            verify(!ghost().visible);
            verify(!canvas.paletteSnapValid);
        }
        function test_rejected_drop_lets_go() {
            scene.rejectAll = true;
            drop(waitSpec, below("if1").x + 2, below("if1").y + 4);
            compare(scene.edits.length, 1);
            verify(!controller.holding);
            verify(!ghost().visible);
        }
        function test_held_until_the_document_shows_it() {
            scene.deferReplies = true;
            drop(waitSpec, 800, 600);
            verify(controller.holding);
            verify(ghost().visible);
            // An unrelated push keeps it, before and after the reply.
            scene.strands = JSON.parse(JSON.stringify(scene.strands)); wait(10);
            verify(controller.holding);
            scene.replies[0](true);
            verify(controller.holding);
            scene.strands = scene.strands.concat([{ id: "s2", x: 600, y: 500, instructions: [scene.edits[0].instruction] }]);
            wait(10);
            verify(!controller.holding);
            verify(!ghost().visible);
        }
        function test_state_before_reply() {
            scene.deferReplies = true;
            drop(waitSpec, 800, 600);
            scene.strands = scene.strands.concat([{ id: "s2", x: 600, y: 500, instructions: [scene.edits[0].instruction] }]);
            wait(10);
            verify(controller.holding, "waits for the reply");
            scene.replies[0](true);
            verify(!controller.holding);
        }
        function test_hold_times_out() {
            scene.deferReplies = true;
            controller.holdTimeout = 100;
            drop(waitSpec, 800, 600);
            verify(controller.holding);
            tryVerify(() => !controller.holding, 1000);
            controller.holdTimeout = 1500;
        }
        function test_document_switch_mid_drag() {
            dragTo(waitSpec, 800, 600);
            verify(controller.active);
            controller.documentId = "doc-b";
            verify(!controller.active);
            verify(!ghost().visible);
            controller.end(806, 606);
            compare(scene.edits.length, 0);
        }
        function test_document_switch_drops_a_held_ghost() {
            scene.deferReplies = true;
            drop(waitSpec, 800, 600);
            verify(controller.holding);
            controller.documentId = "doc-b";
            verify(!controller.holding);
            // A late reply from the old document changes nothing.
            scene.replies[0](true);
            verify(!ghost().visible);
        }
        function test_lock() {
            controller.locked = true;
            dragTo(waitSpec, 800, 600);
            verify(!controller.active, "no drag starts while locked");
            controller.locked = false;
            dragTo(waitSpec, 800, 600);
            verify(controller.active);
            controller.locked = true;
            verify(!controller.active, "locking cancels the drag");
            controller.end(806, 606);
            compare(scene.edits.length, 0);
            verify(!controller.addAt(waitSpec, 0, 0));
        }
        function test_zoom_and_scroll() {
            canvas.setZoom(1.5); wait(50);
            const f = flick();
            verify(!!f);
            f.contentX += 120; f.contentY += 80; wait(20);
            drop(waitSpec, 800, 600);
            const at = canvas.workspacePoint(806, 606);
            compare(scene.edits[0].kind, "add_strand");
            compare(scene.edits[0].x, Math.round(at.x - 6));
            compare(scene.edits[0].y, Math.round(at.y - 6));
            // Gaps still follow the blocks.
            scene.edits = [];
            drop(waitSpec, below("if1").x + 2, below("if1").y + 4);
            compare(scene.edits[0].kind, "insert_instruction");
            compare(scene.edits[0].path[0].index, 2);
        }
        function test_pickup_matches_canvas_zoom_data() {
            return [
                { tag: "instruction_in", spec: waitSpec, zoom: 1.5 },
                { tag: "instruction_out", spec: waitSpec, zoom: 0.6 },
                { tag: "value_in", spec: addSpec, zoom: 1.5 },
                { tag: "value_out", spec: addSpec, zoom: 0.6 }
            ];
        }
        function test_pickup_matches_canvas_zoom(data) {
            scene.strands = [];
            canvas.setZoom(data.zoom);
            wait(20);
            controller.begin(data.spec, 100, 100, 12, 16);
            const g = ghost();
            verify(g.visible);
            compare(g.scale, data.zoom);
            const grabbed = g.mapToItem(null, 12, 16);
            fuzzyCompare(grabbed.x, 100, 0.01);
            fuzzyCompare(grabbed.y, 100, 0.01);
            controller.move(900, 650);
            const top = g.mapToItem(null, 0, 0);
            const edge = g.mapToItem(null, g.width, g.height);
            verify(edge.x > top.x && edge.y > top.y);
            fuzzyCompare(edge.x - top.x, g.width * data.zoom, 0.01);
            fuzzyCompare(edge.y - top.y, g.height * data.zoom, 0.01);
            const at = canvas.workspacePoint(top.x, top.y);
            controller.end(900, 650);
            compare(scene.edits.length, 1);
            compare(scene.edits[0].kind, data.spec.kind === "value" ? "create_floating_value" : "add_strand");
            compare(scene.edits[0].x, Math.round(at.x));
            compare(scene.edits[0].y, Math.round(at.y));
            verify(controller.holding);
            compare(g.scale, data.zoom);
            const held = g.mapToItem(null, 0, 0);
            fuzzyCompare(held.x, top.x, 0.01);
            fuzzyCompare(held.y, top.y, 0.01);
        }
        function test_canvas_drag_above_sidebar_data() {
            return [
                { tag: "block", value: false, zoom: 1 },
                { tag: "block_zoomed", value: false, zoom: 1.5 },
                { tag: "value", value: true, zoom: 1 },
                { tag: "value_zoomed", value: true, zoom: 0.6 }
            ];
        }
        function test_canvas_drag_above_sidebar(data) {
            canvas.setZoom(data.zoom);
            wait(20);
            const sx = 220, sy = 150;
            if (data.value)
                canvas.beginValueDrag({ kind: "Floating", floating_id: "f1", path: [] }, addSpec.value, sx, sy, 60, 12, 120, 32);
            else
                canvas.beginDrag("s1", [{ index: 0 }], 2, sx, sy, 60, 12, 200, 58);
            const g = all(controller).find(o => o.objectName === "canvasDragGhost");
            verify(g.visible);
            verify(!controller.overTrash, "pointer is still on the canvas");
            fuzzyCompare(g.x, sx - 60 * data.zoom, 0.01);
            fuzzyCompare(g.y, sy - 12 * data.zoom, 0.01);
            verify(g.x < canvas.x, "drag overlaps the sidebar");
            if (!data.value) compare(block("w1").opacity, 0);
            if (data.value) canvas.moveValueDrag(180, sy);
            else canvas.moveDrag(180, sy);
            verify(g.visible);
            verify(controller.overTrash);
            if (data.value) canvas.cancelValueDrag();
            else canvas.cancelDrag();
            verify(!g.visible);
            compare(block("w1").opacity, 1);
            compare(scene.edits.length, 0);
        }
        function test_trash() {
            verify(!controller.trashBlocks("s1", [{ index: 1 }], 1, 600, 300));
            verify(controller.trashBlocks("s1", [{ index: 1 }], 1, 50, 300));
            verify(controller.trashBlocks("s1", [{ index: 0 }], 2, 50, 300));
            verify(controller.trashValue({ kind: "Floating", floating_id: "f1", path: [] }, null, 50, 300));
            verify(controller.trashValue({ kind: "Field", strand_id: "s1", index: [{ index: 0 }], field_id: "WaitDuration", path: [] }, null, 50, 300));
            compare(scene.edits.map(e => e.kind), ["remove_instructions", "remove_strand", "remove_floating_value", "take_value"]);
            compare(scene.edits[0].count, 1);
        }
        function test_add_at_view() {
            verify(controller.addAtView(waitSpec));
            verify(controller.addAt(addSpec, 10.4, 20.6));
            compare(scene.edits.map(e => e.kind), ["add_strand", "create_floating_value"]);
            compare(scene.edits[1].x, 10);
            compare(scene.edits[1].y, 21);
        }
        function test_host_factory() {
            controller.instructionFor = spec => spec.type ? { id: "made", type: spec.type, duration: { kind: "Number", value: 1000 } } : null;
            drop({ kind: "instruction", type: "Wait" }, 800, 600);
            controller.instructionFor = null;
            compare(scene.edits[0].instruction.id, "made");
            compare(scene.edits[0].instruction.duration.value, 1000);
        }
    }
}
