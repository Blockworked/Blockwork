import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 600; height: 400
    Component {
        id: chipComponent
        ValueChip {
            x: 80; y: 80
            valueData: ({kind: "Bool"})
            location: ({kind: "Field", strand_id: "main", index: [{index: 0, slot: 0}], field_id: "Condition", path: []})
            boxed: true
        }
    }
    Component {
        id: blockComponent
        InstructionBlock {
            x: 80; y: 80
            strandId: "main"
            path: [{index: 0, slot: 0}]
            instruction: ({id: "while", type: "While", condition: {kind: "Bool"}, body: []})
        }
    }
    SignalSpy { id: dragSpy; signalName: "valueDragBegan" }
    TestCase {
        name: "BooleanSlot"
        when: windowShown
        function all(object) {
            let result = [object];
            for (const child of object.children || []) result = result.concat(all(child));
            return result;
        }
        function init() { dragSpy.target = null; dragSpy.clear(); }
        function test_blank_is_only_a_drop_target() {
            const chip = createTemporaryObject(chipComponent, scene);
            dragSpy.target = chip;
            verify(chip.emptyBoolean);
            verify(chip.booleanShape);
            verify(chip.isValueChip);
            compare(chip.implicitWidth, 48);
            compare(chip.implicitHeight, 27);
            mouseDrag(chip, chip.width / 2, chip.height / 2, 100, 40, Qt.LeftButton, Qt.NoModifier, 20);
            compare(dragSpy.count, 0);
            compare(chip.draggedOut, false);
            compare(chip.hovered, false);
            mouseClick(chip, chip.width / 2, chip.height / 2, Qt.RightButton);
            compare(all(chip).filter(o => o.menu !== undefined && o.menu !== null).length, 0);
            chip.dropHighlighted = true;
            compare(chip.implicitWidth, 48);
            compare(chip.implicitHeight, 27);
        }
        function test_blank_in_while_cannot_be_extracted() {
            const block = createTemporaryObject(blockComponent, scene);
            verify(!!block);
            dragSpy.target = block;
            const chip = all(block).find(o => o.emptyBoolean === true && o.visible);
            verify(!!chip);
            mouseDrag(chip, chip.width / 2, chip.height / 2, 100, -40, Qt.LeftButton, Qt.NoModifier, 20);
            compare(dragSpy.count, 0);
            compare(chip.draggedOut, false);
        }
        function test_nested_blank_and_real_reporter() {
            const chip = createTemporaryObject(chipComponent, scene, {
                valueData: {kind: "Op", op: "Not", args: [{kind: "Bool"}], saved: {kind: "Bool"}}
            });
            dragSpy.target = chip;
            const nested = all(chip).find(o => o !== chip && o.emptyBoolean === true);
            verify(!!nested);
            verify(nested.isValueChip);
            // Pressing the socket belongs to the surrounding reporter.
            mouseDrag(nested, nested.width / 2, nested.height / 2, 100, 40, Qt.LeftButton, Qt.NoModifier, 20);
            compare(dragSpy.count, 1);
            compare(dragSpy.signalArguments[0][1].op, "Not");
        }
        function test_explicit_false_stays_draggable() {
            const chip = createTemporaryObject(chipComponent, scene, {
                valueData: {kind: "Op", op: "False", args: [], saved: {kind: "Bool"}}
            });
            dragSpy.target = chip;
            mouseDrag(chip, chip.width / 2, chip.height / 2, 100, 40, Qt.LeftButton, Qt.NoModifier, 20);
            compare(dragSpy.count, 1);
            compare(dragSpy.signalArguments[0][1].op, "False");
        }
    }
}
