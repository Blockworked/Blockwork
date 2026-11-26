import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// Window parts wear a square tab and only stack inside a show window block.
Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    function invoke(command, args) { calls.push({command: command, args: args}); }
    MockBridge { id: bridge; calls: scene.calls }
    readonly property var text: s => ({kind: "Text", value: s})
    function macro(strands) {
        return {id: "m", name: "m", speed_multiplier: 1, settings: {always_listen: false}, signals: [], variables: [], lists: [], comments: [],
            floating_values: [], block_defs: [], strands: strands};
    }
    function state(m) {
        return {current_macro: m, recording_phase: {phase: "Idle", countdown: null}, key_capture: null, can_undo: false, can_redo: false,
            record_mouse_movement: false, record_mouse_relative: false, absolute_mouse_position_available: true, browser: {unlocked: true}};
    }
    readonly property var strands: [
        {id: "main", x: 40, y: 40, instructions: [{id: "win", type: "ShowWindow", title: text("Settings"), body: [
            {id: "note", type: "WindowNote", text: text("Hello")},
            {id: "old", type: "OpenApp", command: "ark", name: "Ark", icon: null},
            {id: "button", type: "WindowButton", label: text("Save"), close: true, body: [
                {id: "wait", type: "Wait", duration: {kind: "Number", value: 100}}]}]}]},
        {id: "loose", x: 420, y: 40, instructions: [{id: "app", type: "OpenApp", command: "ark", name: "Ark", icon: null}]},
        {id: "part", x: 420, y: 220, instructions: [{id: "label", type: "WindowNote", text: text("Loose")}]}
    ]
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: scene.state(scene.macro(scene.strands))
        }
    }
    TestCase {
        name: "WindowParts"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function editor() {
            mouseMove(scene, scene.width - 2, scene.height - 2, 0, Qt.NoButton);
            BlockRegistry.rows = ({});
            BlockRegistry.resetOperators();
            const page = createTemporaryObject(editorComponent, scene);
            verify(!!page);
            tryVerify(() => !all(page).some(o => o.pendingEntries > 0), 10000, "palette finished building");
            wait(100);
            scene.calls = [];
            return page;
        }
        function block(page, id) { return all(page).find(o => o.instruction && o.instruction.id === id && o.paletteMode === false && o.width > 0); }
        function drag(page, id, target) {
            const b = block(page, id);
            const start = b.mapToItem(scene, 40, 25);
            mouseMove(scene, start.x, start.y);
            mousePress(scene, start.x, start.y);
            mouseMove(scene, start.x + 10, start.y + 10, 10, Qt.LeftButton);
            mouseMove(scene, target.x, target.y, 30, Qt.LeftButton);
            mouseMove(scene, target.x + 1, target.y, 30, Qt.LeftButton);
            wait(120);
        }
        function test_parts_have_a_square_tab_and_a_tint() {
            const page = editor();
            const note = block(page, "note"), win = block(page, "win"), button = block(page, "button");
            verify(note.isPart && button.isPart && !win.isPart);
            compare(win.partMouths, [true]);
            compare(button.partMouths, [false]);
            compare(String(note.displayColor), String(Theme.partBlock));
            verify(!block(page, "wait").misplaced);
            verify(block(page, "old").misplaced, "an older file's Open app inside a window is flagged");
        }
        function test_designer_applies_one_edit_to_the_original_window() {
            const page = editor(), window = scene.strands[0].instructions[0];
            page.onPieceAction("designWindow", "main", [{index: 0}], window);
            const designer = findChild(page, "windowDesigner");
            tryCompare(designer, "opened", true);
            designer.add(designer.parts.find(p => p.layout === "row"));
            designer.add(designer.parts.find(p => p.display === "progress"));
            designer.setLiteral("label", "Storage");
            designer.setField("data", {kind: "Var", name: "used"});
            compare(designer.previewControls(designer.draft.body).slice(-1)[0].children[0].value, 68);
            compare(scene.calls.length, 0);
            designer.save();
            tryCompare(designer, "opened", false);
            compare(scene.calls.length, 1);
            const edit = scene.calls[0];
            compare(edit.command, "edit_instruction");
            compare(edit.args.strandId, "main");
            compare(edit.args.instruction.id, "win");
            compare(edit.args.instruction.body[2].body, window.body[2].body);
            const meter = edit.args.instruction.body[3].body[0];
            compare(meter.label.value, "Storage");
            compare(meter.data.name, "used");
        }
        function test_designer_navigates_and_moves_nested_components() {
            const page = editor(), window = scene.strands[0].instructions[0];
            page.onPieceAction("designWindow", "main", [{index: 0}], window);
            const designer = findChild(page, "windowDesigner");
            tryCompare(designer, "opened", true);
            const outline = findChild(designer, "windowDesignerOutline");
            const tree = findChild(outline, "windowDesignerTree");
            const add = findChild(outline, "designerAddComponent");
            const picker = findChild(outline, "designerComponentPicker");
            const search = findChild(picker, "designerComponentSearch");

            designer.add(designer.parts.find(p => p.layout === "tabs"));
            const tabs = designer.selectedId;
            compare(add.text, "Add tab");
            compare(designer.addChoices.length, 1);
            mouseClick(add);
            tryCompare(picker, "opened", true);
            tryCompare(search, "activeFocus", true);
            keyClick(Qt.Key_Return);
            tryCompare(picker, "opened", false);
            const first = designer.selectedId;
            compare(designer.block.label.value, "Tab");

            mouseClick(add);
            tryCompare(picker, "opened", true);
            tryCompare(search, "activeFocus", true);
            keyClick(Qt.Key_P);
            keyClick(Qt.Key_R);
            keyClick(Qt.Key_O);
            keyClick(Qt.Key_G);
            compare(picker.choices.length, 1);
            keyClick(Qt.Key_Return);
            tryCompare(picker, "opened", false);
            const meter = designer.selectedId;
            compare(designer.block.display, "progress");
            compare(outline.addTarget, "After Progress bar");

            tree.forceActiveFocus();
            keyClick(Qt.Key_Left);
            compare(designer.selectedId, first);
            keyClick(Qt.Key_Left);
            verify(!outline.entries.some(row => row.block.id === meter));
            keyClick(Qt.Key_Right);
            verify(outline.entries.some(row => row.block.id === meter));
            keyClick(Qt.Key_Right);
            compare(designer.selectedId, meter);

            designer.selectedId = tabs;
            verify(!designer.moveChoices.some(choice => choice.id === first), "a layout cannot move into its own child");
            designer.add(designer.addChoices[0]);
            const second = designer.selectedId;
            designer.setLiteral("label", "Details");
            designer.selectedId = meter;
            mouseClick(findChild(outline, "designerMoveTo"));
            tryCompare(picker, "opened", true);
            tryCompare(search, "activeFocus", true);
            keyClick(Qt.Key_D);
            keyClick(Qt.Key_E);
            keyClick(Qt.Key_T);
            compare(picker.choices.length, 1);
            keyClick(Qt.Key_Return);
            tryCompare(picker, "opened", false);
            compare(designer.selectedId, meter);
            compare(designer.block.display, "progress");
            compare(designer.draft.body[3].body[0].body.length, 0);
            compare(designer.draft.body[3].body[1].body[0].id, meter);
            tryVerify(() => {
                const button = findChild(designer.contentItem, "promptFormTab_" + second);
                return button && button.checked;
            });
            compare(scene.calls.length, 0);
            designer.close();
        }
        function test_a_normal_block_does_not_snap_into_the_window() {
            const page = editor();
            const canvas = all(page).find(o => o.isBlockCanvas);
            const note = block(page, "note");
            const target = note.mapToItem(scene, 40, 25);
            drag(page, "app", target);
            const state = canvas.branchDragState;
            verify(!state.snapValid, "Open app must not snap into the window");
            mouseRelease(scene, target.x + 1, target.y);
            verify(!scene.calls.some(c => c.command === "merge_tail" || c.command === "merge_strand"));
        }
        function test_a_part_snaps_into_the_window_but_not_a_normal_stack() {
            let page = editor();
            const canvas = all(page).find(o => o.isBlockCanvas);
            const button = block(page, "button");
            const below = button.mapToItem(scene, 40, button.height + 4);
            drag(page, "label", below);
            verify(canvas.branchDragState.snapValid, "a label snaps under the button");
            mouseRelease(scene, below.x + 1, below.y);
            verify(scene.calls.some(c => c.command === "merge_tail" || c.command === "merge_strand"));

            page = editor();
            const app = block(page, "app");
            const under = app.mapToItem(scene, 40, app.height + 4);
            drag(page, "label", under);
            const state = all(page).find(o => o.isBlockCanvas).branchDragState;
            verify(!state.snapValid, "a label must not snap under Open app");
            mouseRelease(scene, under.x + 1, under.y);
        }
    }
}
