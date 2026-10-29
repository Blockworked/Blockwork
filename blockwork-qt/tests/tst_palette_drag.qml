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
            let block = null;
            tryVerify(() => {
                block = all(page).find(o => o.spec && o.spec.kind === "instruction" && o.spec.type === type && o.visible && o.width > 0);
                return !!block;
            }, 5000, type);
            for (let p = block ? block.parent : null; p; p = p.parent)
                if (p.contentY !== undefined && p.flickableDirection !== undefined) { p.contentY += block.mapToItem(p, 0, 0).y - 100; wait(20); break; }
            return block;
        }
        function canvasBlock(page, id) { return all(page).find(o => o.instruction && o.instruction.id === id && o.paletteMode === false && o.width > 0); }
        function ghost(page) { return all(page).find(o => o.objectName === "paletteGhost"); }
        function test_key_input_drag_keeps_views_stationary_data() {
            return [{tag: "key", type: "Key"}, {tag: "trigger", type: "WhenKey"}];
        }
        function test_key_input_drag_keeps_views_stationary(data) {
            const page = editor();
            const source = paletteBlock(page, data.type);
            verify(!!source);
            const key = all(source).find(o => o.keyMode === true && o.width > 0);
            verify(!!key);
            const views = all(page).filter(o => o.contentX !== undefined && o.flickableDirection !== undefined);
            const positions = views.map(o => ({x: o.contentX, y: o.contentY}));
            const start = key.mapToItem(scene, key.width / 2, key.height / 2);
            mousePress(scene, start.x, start.y);
            for (let step = 1; step <= 5; ++step) {
                mouseMove(scene, start.x + step * 20, start.y - step * 15, 20, Qt.LeftButton);
                for (let i = 0; i < views.length; ++i) {
                    compare(views[i].contentX, positions[i].x, "view horizontal position");
                    compare(views[i].contentY, positions[i].y, "view vertical position");
                }
                verify(!page.paletteDrags.drag);
                verify(!page.paletteDrags.canvas.dragging);
            }
            mouseRelease(scene, start.x + 100, start.y - 75);
            wait(100);
            for (let i = 0; i < views.length; ++i) {
                compare(views[i].contentX, positions[i].x);
                compare(views[i].contentY, positions[i].y);
            }
            compare(scene.calls.length, 0);
            const click = key.mapToItem(scene, key.width / 2, key.height / 2);
            mouseClick(scene, click.x, click.y);
            compare(scene.calls.map(c => c.command), ["start_standalone_key_capture"]);
        }
        function test_sidebar_keeps_workspace_stationary_data() {
            const rows = [];
            for (const layout of ["rail", "chips", "accordion", "flyout", "tiles"])
                for (const zoom of [0.5, 1, 1.8])
                    rows.push({tag: layout + "_" + zoom, layout: layout, zoom: zoom});
            return rows;
        }
        function test_sidebar_keeps_workspace_stationary(data) {
            const page = editor();
            page.appState = Object.assign({}, page.appState, {palette_layout: data.layout});
            wait(30);
            const canvas = all(page).find(o => typeof o.workspacePoint === "function");
            verify(!!canvas);
            canvas.setZoom(data.zoom);
            const block = canvasBlock(page, "wait");
            verify(!!block);
            const before = block.mapToItem(scene, 0, 0);
            const at = scene.mapToItem(null, 900, 400);
            const world = canvas.workspacePoint(at.x, at.y);
            verify(!!world);
            function checkView() {
                const after = block.mapToItem(scene, 0, 0);
                fuzzyCompare(after.x, before.x, 0.1, "block stays fixed horizontally");
                fuzzyCompare(after.y, before.y, 0.1, "block stays fixed vertically");
                const point = canvas.workspacePoint(at.x, at.y);
                fuzzyCompare(point.x, world.x, 0.1, "world x stays fixed");
                fuzzyCompare(point.y, world.y, 0.1, "world y stays fixed");
            }
            for (const width of [180, 600, 348]) {
                page.sidebarWidth = width;
                wait(30);
                checkView();
            }
            for (const collapsed of [true, false]) {
                page.sidebarToggling = true;
                page.sidebarCollapsed = collapsed;
                for (let frame = 0; frame < 20; ++frame) {
                    wait(16);
                    checkView();
                }
            }
        }
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
        function test_wait_components_conflict_data() {
            return [{tag: "condition", type: "WaitUntil"}, {tag: "image", type: "WaitForImage"}];
        }
        function test_wait_components_conflict(data) {
            const page = editor();
            const source = paletteBlock(page, data.type);
            verify(!!source);
            const row = all(source).find(o => o.paletteMode === true);
            for (const key of ["timeout", "error_timeout"]) {
                const other = key === "timeout" ? "error_timeout" : "timeout";
                const component = row.rowSpec.components.find(c => c.key === key);
                row.addComponent(component);
                verify(key in source.instruction);
                verify(!(other in source.instruction));
                verify(!BlockRegistry.missingComponents(row.rowSpec, source.instruction).some(c => c.key === other));
                row.removeComponent(component);
                verify(BlockRegistry.missingComponents(row.rowSpec, source.instruction).some(c => c.key === other));
                row.addComponent(row.rowSpec.components.find(c => c.key === other));
                row.addComponent(component);
                verify(key in source.instruction);
                verify(!(other in source.instruction));
                row.removeComponent(component);
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
        function openClearDialog(page) {
            const canvas = all(page).find(o => typeof o.clearRequested === "function");
            verify(!!canvas);
            canvas.clearRequested();
            const dialog = findChild(page, "clearInstructionsDialog");
            verify(!!dialog);
            tryCompare(dialog, "opened", true);
            compare(scene.calls.length, 0);
            return dialog;
        }
        function test_clear_blocks_requires_popup_confirmation() {
            const dialog = openClearDialog(editor());
            const button = all(dialog.footer).find(o => o.text === "Delete");
            verify(!!button);
            mouseClick(button);
            compare(scene.calls.map(c => c.command), ["clear_instructions"]);
        }
        function test_clear_blocks_cancel_keeps_canvas() {
            const dialog = openClearDialog(editor());
            const button = all(dialog.footer).find(o => o.text === "Cancel");
            verify(!!button);
            mouseClick(button);
            compare(scene.calls.length, 0);
        }
        function test_clear_blocks_does_not_delete_another_macro() {
            const page = editor();
            const dialog = openClearDialog(page);
            page.appState = scene.state(scene.macro("other", scene.strands));
            dialog.accept();
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
        function test_canvas_release_over_sidebar_deletes_data() {
            const rows = [];
            for (const layout of ["rail", "chips", "accordion", "flyout", "tiles"])
                for (const kind of ["strand", "definition", "value"])
                    rows.push({tag: layout + "_" + kind, layout: layout, kind: kind});
            return rows;
        }
        function test_canvas_release_over_sidebar_deletes(data) {
            const page = editor();
            page.appState = Object.assign({}, page.appState, {palette_layout: data.layout});
            wait(30);
            const canvas = page.paletteDrags.canvas;
            const sidebar = all(page).find(o => typeof o.customInstruction === "function");
            const at = sidebar.mapToItem(null, 20, 300);
            compare(canvas.workspacePoint(at.x, at.y), null, "sidebar is outside the drop area");
            const start = canvas.mapToItem(null, 800, 300);
            if (data.kind === "value") {
                const location = {kind: "Instruction", strand_id: "main", path: [{index: 0}], field: "duration", value_path: []};
                canvas.beginValueDrag(location, {kind: "Number", value: 100}, start.x, start.y, 0, 0, 60, 30);
                canvas.endValueDrag(at.x, at.y);
                compare(scene.calls.map(c => c.command), ["take_value"]);
            } else {
                const id = data.kind === "definition" ? "definition" : "main";
                canvas.beginDrag(id, [{index: 0}], 1, start.x, start.y, 0, 0, 150, 60);
                canvas.endDrag(at.x, at.y);
                compare(scene.calls.map(c => c.command), [data.kind === "definition" ? "delete_block" : "remove_strand"]);
            }
            page.sidebarCollapsed = true;
            wait(30);
            verify(!!canvas.workspacePoint(at.x, at.y), "collapsed sidebar reveals the drop area");
        }
        function test_mouse_drag_to_sidebar_deletes() {
            const page = editor();
            const source = canvasBlock(page, "wait");
            const start = source.mapToItem(scene, 12, source.height / 2);
            mousePress(scene, start.x, start.y);
            mouseMove(scene, start.x + 40, start.y + 10, 20);
            mouseMove(scene, 20, 400, 20);
            mouseRelease(scene, 20, 400);
            compare(scene.calls.map(c => c.command), ["remove_strand"]);
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
