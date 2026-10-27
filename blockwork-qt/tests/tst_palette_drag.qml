import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    property bool failCalls: false
    function invoke(command, args) { calls.push({command: command, args: args}); }
    QtObject {
        id: bridge
        property int replySerial: 0
        property int previewSerial: 0
        property string activeBlocksJson: "[]"
        property string installedAppsError: ""
        property string replyKey: ""
        property string replyJson: "null"
        property string replyError: ""
        property string lastError: ""
        function callCommand(key, command, args) {
            scene.calls.push({command: command, args: JSON.parse(args)});
            replyKey = key; replyJson = "null"; replyError = scene.failCalls ? "Can't put that there" : ""; replySerial++;
        }
    }
    function macro(id, strands) {
        return {id: id, name: id, speed_multiplier: 1, settings: {always_listen: false}, signals: [], variables: [], lists: [], comments: [],
            floating_values: [], block_defs: [{id: "custom", identifier: "Custom", color: "#51b84b", shape: "Normal", pieces: [{kind: "Label", text: "custom"}]}],
            strands: strands};
    }
    function state(m, phase) {
        return {current_macro: m, recording_phase: {phase: phase || "Idle", countdown: phase === "Countdown" ? 3 : null}, key_capture: null, can_undo: false, can_redo: false,
            record_mouse_movement: false, record_mouse_relative: false, absolute_mouse_position_available: true};
    }
    readonly property var strands: [
        {id: "main", x: 40, y: 40, instructions: [{id: "wait", type: "Wait", duration: {kind: "Number", value: 100}}]},
        {id: "definition", x: 40, y: 300, instructions: [{id: "header", type: "BlockHeader", block_id: "custom"}]}
    ]
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: scene.state(scene.macro("macro", scene.strands))
        }
    }
    TestCase {
        name: "PaletteDrag"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function editor() {
            const page = createTemporaryObject(editorComponent, scene);
            verify(!!page);
            wait(100);
            scene.calls = []; scene.failCalls = false;
            return page;
        }
        // Scrolled into view in the palette's list.
        function paletteBlock(page, type) {
            const block = all(page).find(o => o.spec && o.spec.kind === "instruction" && o.spec.type === type && o.visible && o.width > 0);
            for (let p = block ? block.parent : null; p; p = p.parent)
                if (p.contentY !== undefined && p.flickableDirection !== undefined) { p.contentY += block.mapToItem(p, 0, 0).y - 100; wait(20); break; }
            return block;
        }
        function canvasBlock(page, id) { return all(page).find(o => o.instruction && o.instruction.id === id && o.paletteMode === false && o.width > 0); }
        function ghost(page) { return all(page).find(o => o.objectName === "paletteGhost"); }
        function dragFromPalette(page, type, x, y) {
            const source = paletteBlock(page, type);
            verify(!!source, type);
            const start = source.mapToItem(scene, 12, source.height / 2);
            mousePress(scene, start.x, start.y);
            mouseMove(scene, start.x + 30, start.y + 10, 20);
            mouseMove(scene, x, y, 20);
            mouseMove(scene, x + 1, y + 1, 20);
            mouseRelease(scene, x + 1, y + 1);
            wait(20);
        }
        function test_drop_into_gap_uses_blockwork_defaults() {
            const page = editor();
            const wait = canvasBlock(page, "wait");
            const below = wait.mapToItem(scene, 20, wait.height + 6);
            dragFromPalette(page, "Wait", below.x, below.y);
            const calls = scene.calls.filter(c => c.command === "add_instruction");
            compare(calls.length, 1);
            compare(calls[0].args.strandId, "main");
            compare(calls[0].args.path[0].index, 1);
            compare(calls[0].args.instruction.duration.value, 1000, "EditorPage.defaultInstruction, not the palette prefab");
        }
        function test_command_blocks_data() {
            const rows = [];
            for (const type of ["Command", "CommandWait"])
                for (const target of ["canvas", "gap"])
                    rows.push({tag: type + "_" + target, type: type, target: target});
            return rows;
        }
        function test_command_blocks(data) {
            const page = editor();
            const source = paletteBlock(page, data.type);
            verify(!!source);
            compare(source.instruction.command, "");
            const field = all(source).find(o => o.placeholderText === "command");
            verify(!!field);
            compare(field.width, 92);
            compare(field.height, 28);
            field.text = "printf hello";
            field.editingFinished();
            let destination = {x: 900, y: 650};
            if (data.target === "gap") {
                const block = canvasBlock(page, "wait");
                destination = block.mapToItem(scene, 20, block.height + 6);
            }
            dragFromPalette(page, data.type, destination.x, destination.y);
            const command = data.target === "gap" ? "add_instruction" : "add_strand";
            const calls = scene.calls.filter(c => c.command === command);
            compare(calls.length, 1);
            compare(calls[0].args.instruction.type, data.type);
            compare(calls[0].args.instruction.command, "printf hello");
            verify(calls[0].args.instruction.id !== source.instruction.id);
        }
        function test_edited_components_survive_insertion_data() {
            const rows = [];
            for (const type of ["BrowserClick", "BrowserType", "ClickImage"])
                for (const target of ["canvas", "gap", "activation"])
                    rows.push({tag: type + "_" + target, type: type, target: target});
            return rows;
        }
        function test_edited_components_survive_insertion(data) {
            const page = editor();
            const source = paletteBlock(page, data.type);
            verify(!!source);
            const row = all(source).find(o => o.paletteMode === true);
            const component = row.rowSpec.components[0];
            row.addComponent(component);
            verify(component.key in source.instruction);
            if (data.type.indexOf("Browser") === 0) {
                const edited = JSON.parse(JSON.stringify(source.instruction));
                edited.nth = {kind: "Number", value: 3};
                edited.selector = {kind: "Text", value: "button.save"};
                row.instructionEdited("", [], edited);
            }
            const expected = JSON.parse(JSON.stringify(source.instruction));
            if (data.target === "activation") source.activated();
            else if (data.target === "gap") {
                const wait = canvasBlock(page, "wait");
                const below = wait.mapToItem(scene, 20, wait.height + 6);
                dragFromPalette(page, data.type, below.x, below.y);
            } else dragFromPalette(page, data.type, 900, 650);
            const command = data.target === "gap" ? "add_instruction" : "add_strand";
            const calls = scene.calls.filter(c => c.command === command);
            compare(calls.length, 1);
            const inserted = JSON.parse(JSON.stringify(calls[0].args.instruction));
            verify(inserted.id !== expected.id, "the inserted block gets its own id");
            delete inserted.id;
            delete expected.id;
            compare(inserted, expected, "all edited palette fields survive insertion");
            compare(source.instruction.id, "palette-" + data.type);
        }
        function test_optional_wait_and_tolerance_components_data() {
            return [
                {tag: "condition", type: "WaitUntil", keys: ["timeout", "error_timeout"]},
                {tag: "image", type: "WaitForImage", keys: ["timeout", "error_timeout", "tolerance"]},
                {tag: "find", type: "FindImage", keys: ["tolerance"]},
                {tag: "click", type: "ClickImage", keys: ["tolerance"]},
                {tag: "mouse_keys", type: "WhenMouseButton", keys: ["modifiers"]}
            ];
        }
        function test_optional_wait_and_tolerance_components(data) {
            const page = editor();
            const source = paletteBlock(page, data.type);
            verify(!!source);
            const row = all(source).find(o => o.paletteMode === true);
            for (const key of data.keys) {
                verify(!(key in source.instruction), key + " is absent by default");
                const component = row.rowSpec.components.find(c => c.key === key);
                verify(!!component, key);
                row.addComponent(component);
                compare(source.instruction[key], key === "modifiers" ? {kind: "Text", value: "Control"} : {kind: "Number", value: key === "tolerance" ? 10 : 10000});
                const edited = JSON.parse(JSON.stringify(source.instruction));
                edited[key] = {kind: "Number", value: 42};
                row.instructionEdited("", [], edited);
                compare(source.instruction[key].value, 42);
                row.removeComponent(component);
                verify(!(key in source.instruction), key + " is removed");
            }
        }
        function test_sidebar_pickup_matches_canvas_zoom() {
            const page = editor();
            page.paletteDrags.canvas.setZoom(1.5);
            const source = paletteBlock(page, "Wait");
            const start = source.mapToItem(scene, 12, source.height / 2);
            mousePress(scene, start.x, start.y);
            mouseMove(scene, start.x + 30, start.y + 10, 20);
            const preview = ghost(page);
            verify(preview.visible);
            compare(preview.scale, 1.5);
            const grabbed = preview.mapToItem(scene, 12, source.height / 2);
            fuzzyCompare(grabbed.x, start.x + 30, 1);
            fuzzyCompare(grabbed.y, start.y + 10, 1);
            mouseMove(scene, 900, 650, 20);
            const top = preview.mapToItem(null, 0, 0);
            const at = page.paletteDrags.canvas.workspacePoint(top.x, top.y);
            mouseRelease(scene, 900, 650);
            const calls = scene.calls.filter(c => c.command === "add_strand");
            compare(calls.length, 1);
            compare(calls[0].args.x, Math.round(at.x));
            compare(calls[0].args.y, Math.round(at.y));
            verify(page.paletteDrags.holding);
            compare(preview.scale, 1.5);
        }
        function test_held_ghost_until_the_macro_shows_it() {
            const page = editor();
            dragFromPalette(page, "Wait", 900, 650);
            compare(scene.calls.filter(c => c.command === "add_strand").length, 1);
            verify(page.paletteDrags.holding);
            verify(ghost(page).visible);
            // An unrelated state push (same strands) keeps it.
            page.appState = scene.state(scene.macro("macro", scene.strands));
            wait(20);
            verify(page.paletteDrags.holding);
            const added = scene.strands.concat([{id: "new", x: 500, y: 500, instructions: [scene.calls[0].args.instruction]}]);
            page.appState = scene.state(scene.macro("macro", added));
            wait(20);
            verify(!page.paletteDrags.holding);
            verify(!ghost(page).visible);
        }
        function test_rejected_drop_lets_go_and_shows_the_error() {
            const page = editor();
            scene.failCalls = true;
            dragFromPalette(page, "Wait", 900, 650);
            verify(!page.paletteDrags.holding);
            compare(bridge.lastError, "Can't put that there");
            bridge.lastError = "";
        }
        function test_recording_countdown_locks_the_canvas() {
            const page = editor();
            page.appState = scene.state(scene.macro("macro", scene.strands), "Countdown");
            wait(20);
            dragFromPalette(page, "Wait", 900, 650);
            compare(scene.calls.length, 0);
            verify(!ghost(page).visible);
            page.appState = scene.state(scene.macro("macro", scene.strands), "Active");
            wait(20);
            dragFromPalette(page, "Wait", 900, 650);
            compare(scene.calls.length, 0);
        }
        function test_switching_macro_mid_drag_cancels() {
            const page = editor();
            const source = paletteBlock(page, "Wait");
            const start = source.mapToItem(scene, 12, source.height / 2);
            mousePress(scene, start.x, start.y);
            mouseMove(scene, start.x + 30, start.y + 10, 20);
            mouseMove(scene, 900, 650, 20);
            verify(!!page.paletteDrags.drag);
            page.appState = scene.state(scene.macro("other", scene.strands));
            wait(20);
            verify(!page.paletteDrags.drag);
            mouseRelease(scene, 900, 650);
            compare(scene.calls.length, 0);
        }
        function test_trashing_a_definition_deletes_the_block() {
            const page = editor();
            const sidebar = all(page).find(o => typeof o.customInstruction === "function");
            const at = sidebar.mapToItem(null, 40, 300);
            verify(page.paletteDrags.trashBlocks("definition", [{index: 0}], 1, at.x, at.y));
            verify(page.paletteDrags.trashBlocks("main", [{index: 0}], 1, at.x, at.y));
            verify(page.paletteDrags.trashBlocks("main", [{index: 1}], 3, at.x, at.y));
            compare(scene.calls.map(c => c.command), ["delete_block", "remove_strand", "remove_instruction"]);
            compare(scene.calls[0].args.blockId, "custom");
            compare(scene.calls[2].args.count, 3);
        }
        function test_double_click_adds_at_the_old_spot() {
            const page = editor();
            const source = paletteBlock(page, "Wait");
            source.activated();
            compare(scene.calls.length, 1);
            compare(scene.calls[0].command, "add_strand");
            compare(scene.calls[0].args.x, 120);
            compare(scene.calls[0].args.y, 120);
            compare(scene.calls[0].args.instruction.duration.value, 1000);
        }
    }
}
