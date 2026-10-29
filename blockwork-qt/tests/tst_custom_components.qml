import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1100; height: 800
    property var definition: ({id: "custom", shape: "Normal", pieces: [
        {kind: "Label", id: "name", text: "example"},
        {kind: "Input", id: "required", name: "required", value_type: "Any"},
        {kind: "Input", id: "timeout", name: "timeout", value_type: "Bool", component: true},
        {kind: "Label", id: "for", text: "for up to"},
        {kind: "Input", id: "duration", name: "duration", value_type: "Any", default_value: {kind: "Number", value: 100}},
        {kind: "Input", id: "retry", name: "retry", value_type: "Bool", component: true}
    ]})
    property var replacement: null
    Component { id: rowFactory; InstructionBlock {
        strandId: "s"; path: [{index: 0}]
        blockDefinitions: [scene.definition]
        onInstructionEdited: (sid, path, value) => instruction = value
    } }
    Component { id: chipFactory; ValueChip {
        editable: true
        blockDefinitions: [scene.definition]
        onLeafEdited: (path, leaf) => {
            const copy = JSON.parse(JSON.stringify(valueData));
            copy.args[path[0]] = leaf;
            valueData = copy;
        }
    } }
    Component { id: editorFactory; BlockDefinitionEditor { width: 1000; height: 760 } }
    Component { id: dialogFactory; MakeBlockDialog {} }
    TestCase {
        name: "CustomComponents"
        when: windowShown
        function all(item) {
            let out = [item];
            for (const child of item.children || []) out = out.concat(all(child));
            return out;
        }
        function call() { return {type: "CallBlock", id: "call", block_id: "custom", args: BlockRegistry.freshCallArgs(scene.definition), branches: []}; }
        function init() { scene.replacement = null; }
        function cleanup() { BlockRegistry.valueReplacementHandler = null; }
        function test_definition_preview_wheel_matches_canvas_step() {
            const editor = createTemporaryObject(editorFactory, scene);
            editor.reset();
            const viewport = findChild(editor, "definitionPreviewViewport");
            tryCompare(viewport, "contentY", 96);
            const step = Qt.styleHints.wheelScrollLines * 24;
            waitForRendering(viewport);
            mouseMove(viewport, 40, 40);
            mouseWheel(viewport, 40, 40, 0, 120);
            tryCompare(viewport, "contentY", Math.max(0, 96 - step));
            mouseWheel(viewport, 40, 40, -120, 0);
            tryCompare(viewport, "contentX", Math.min(192, 96 + step));
            mouseWheel(viewport, 40, 40, 0, 1200);
            tryCompare(viewport, "contentY", 0);
            editor.reset();
            tryCompare(viewport, "contentX", 96);
            tryCompare(viewport, "contentY", 96);
        }
        function test_definition_preview_pan_is_bounded_and_resets() {
            const editor = createTemporaryObject(editorFactory, scene);
            editor.reset();
            const viewport = findChild(editor, "definitionPreviewViewport");
            const block = findChild(editor, "definitionPreviewBlock");
            verify(!!viewport);
            tryCompare(viewport, "contentX", 96);
            tryCompare(viewport, "contentY", 96);
            mouseDrag(viewport, 40, 40, 60, 40);
            viewport.cancelFlick();
            verify(viewport.contentX < 96);
            verify(viewport.contentY < 96);
            mouseDrag(viewport, 40, 40, 700, 300);
            viewport.cancelFlick();
            compare(viewport.contentX, 0);
            compare(viewport.contentY, 0);
            editor.reset();
            tryCompare(viewport, "contentX", 96);
            tryCompare(viewport, "contentY", 96);
            editor.load({pieces: [{kind: "Label", text: "long block ".repeat(40)}]});
            tryVerify(() => block.width > viewport.width);
            verify(viewport.contentWidth >= block.width + 192);
            compare(block.scale, 1);
        }
        function test_instruction_menu_and_removal_preserve_arguments() {
            const row = createTemporaryObject(rowFactory, scene, {instruction: call()});
            verify(!!row.rowSpec);
            compare(row.headPieces.map(p => p.kind), ["label", "value", "addComponent"]);
            const plus = all(row).find(o => o.objectName === "addComponentButton");
            verify(!!plus);
            mouseClick(plus);
            const menu = findChild(plus, "addComponentMenu");
            tryCompare(menu, "opened", true);
            compare(menu.count, 2);
            mouseClick(menu.itemAt(0));
            tryVerify(() => BlockRegistry.componentEnabled(row.instruction.args[1]));
            compare(row.headPieces.map(p => p.kind), ["label", "value", "label", "label", "value", "removeComponent", "addComponent"]);
            const updated = JSON.parse(JSON.stringify(row.instruction));
            updated.args[2] = {kind: "Number", value: 250};
            row.instruction = updated;
            const remove = all(row).find(o => o.objectName === "removeComponentButton");
            verify(!!remove);
            waitForRendering(remove);
            wait(30);
            mouseClick(remove);
            tryVerify(() => !BlockRegistry.componentEnabled(row.instruction.args[1]));
            compare(row.instruction.args[2].value, 250);
            row.addComponent(row.rowSpec.components[0]);
            compare(row.instruction.args[2].value, 250);
        }
        function test_reporter_menu_and_nested_location() {
            const value = call(); delete value.type; value.kind = "Call"; value.saved = {kind: "Number", value: 0};
            const chip = createTemporaryObject(chipFactory, scene, {valueData: value});
            const plus = all(chip).find(o => o.objectName === "addCustomComponentButton" && o.visible);
            verify(!!plus);
            mouseClick(plus);
            const menu = findChild(plus, "customComponentMenu");
            tryCompare(menu, "opened", true);
            mouseClick(menu.itemAt(0));
            tryVerify(() => BlockRegistry.componentEnabled(chip.valueData.args[1]));
            const remove = all(chip).find(o => o.objectName === "removeCustomComponentButton" && o.visible);
            verify(!!remove);
            waitForRendering(remove);
            wait(30);
            mouseClick(remove);
            tryVerify(() => !BlockRegistry.componentEnabled(chip.valueData.args[1]));
            chip.location = {strand_id: "s", field: "Wait", path: [3]};
            BlockRegistry.valueReplacementHandler = (location, value) => scene.replacement = {location: location, value: value};
            chip.toggleComponent(chip.customRow.components[0], true);
            compare(scene.replacement.location.path, [3, 1]);
            verify(BlockRegistry.componentEnabled(scene.replacement.value));
        }
        function test_authoring_and_editing_keeps_component_metadata() {
            const editor = createTemporaryObject(editorFactory, scene);
            editor.reset();
            editor.addComponent();
            editor.commitPiece();
            editor.addPiece("Input", "Any");
            editor.commitPiece();
            const pieces = editor.resultPieces();
            compare(pieces.length, 3);
            verify(pieces[1].component);
            compare(pieces[1].value_type, "Bool");
            compare(pieces[1].default_value, {kind: "Bool", value: false});
            editor.load({id: "edited", pieces: pieces, shape: "ReturnsBool"});
            compare(editor.resultPieces(), pieces);
            const spec = BlockRegistry.customCallRow({pieces: pieces});
            compare(spec.components.length, 1);
            compare(spec.components[0].pieces.length, 2);
            wait(20);
        }
        function test_branch_calls_keep_mouths_and_component_fields() {
            const value = call(); value.type = "BranchCallBlock"; value.branches = [[]];
            const row = createTemporaryObject(rowFactory, scene, {instruction: value});
            verify(row.isWrap);
            compare(row.slotCount(), 1);
            row.addComponent(row.rowSpec.components[0]);
            verify(row.headPieces.some(p => p.field === "CallArg:2"));
        }
        function test_definition_and_dialog_contain_component_fields() {
            const dialog = createTemporaryObject(dialogFactory, scene);
            dialog.openForEdit(scene.definition);
            tryCompare(dialog, "opened", true);
            wait(50);
            const groups = all(dialog.contentItem).filter(o => o.objectName === "prototypeComponentGroup" && o.visible);
            compare(groups.length, 2);
            verify(groups[0].width > groups[1].width);
            const appearance = findChild(dialog.contentItem, "appearanceButton");
            verify(!!appearance);
            mouseClick(appearance);
            const popup = findChild(dialog.contentItem, "appearancePopup");
            tryCompare(popup, "opened", true);
            popup.close();
            const row = createTemporaryObject(rowFactory, scene, {instruction: {type: "BlockHeader", id: "head", block_id: "custom"}});
            wait(30);
            const headers = all(row).filter(o => o.objectName === "definitionComponentGroup");
            compare(headers.length, 2);
            verify(headers[0].width > headers[1].width);
            dialog.close();
        }
    }
}
