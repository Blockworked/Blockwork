import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    width: 800; height: 400

    ValueChip {
        id: chip
        x: 30; y: 30
        width: implicitWidth; height: implicitHeight
        location: ({ kind: "Field", strand_id: "s1", index: [{ index: 0 }], field_id: "TextValue", path: [] })
    }
    SignalSpy { id: began; target: chip; signalName: "valueDragBegan" }
    SignalSpy { id: moved; target: chip; signalName: "valueDragMoved" }
    SignalSpy { id: ended; target: chip; signalName: "valueDragEnded" }

    TestCase {
        name: "ValueChipDrag"
        when: windowShown

        function init() {
            BlockRegistry.settlingValueKey = "";
            chip.location = { kind: "Field", strand_id: "s1", index: [{ index: 0 }], field_id: "TextValue", path: [] };
            chip.valueData = { kind: "Op", op: "Join", args: [{ kind: "Text", value: "long first input" }, { kind: "Text", value: "long second input" }], saved: { kind: "Text", value: "hello" } };
            began.clear(); moved.clear(); ended.clear();
        }
        function cleanup() {
            mouseRelease(chip, 5, 5);
            BlockRegistry.settlingValueKey = "";
        }
        function startDrag() {
            waitForRendering(chip);
            const w = chip.width, h = chip.height;
            mousePress(chip, 5, 5);
            mouseMove(chip, 25, 60, 20);
            tryCompare(chip, "draggedOut", true);
            compare(began.count, 1);
            compare(began.signalArguments[0][6], w);
            compare(began.signalArguments[0][7], h);
        }
        function restored() { return findChild(chip, "restoredValue"); }

        function test_savedTextAppearsBeforeReleaseAndSlotShrinks() {
            waitForRendering(chip);
            const w = chip.width;
            startDrag();
            verify(restored().item !== null);
            compare(restored().item.valueData, { kind: "Text", value: "hello" });
            verify(restored().item.visible);
            verify(chip.width < w);
            compare(chip.valueData.kind, "Op");
            mouseMove(chip, 180, 100, 20);
            verify(moved.count > 1);
            mouseRelease(chip, 180, 100);
            compare(ended.count, 1);
            compare(chip.draggedOut, false);
            compare(chip.width, w);
        }
        function test_savedOperatorIsRenderedAsAChip() {
            chip.valueData = { kind: "Call", block_id: "reporter", args: [], saved: { kind: "Op", op: "True", args: [], saved: { kind: "Bool" } } };
            startDrag();
            compare(restored().item.valueData.op, "True");
            verify(restored().item.booleanShape);
        }
        function test_nestedSlotRestoresSavedNumber() {
            chip.location = { kind: "Floating", floating_id: "f1", path: [0] };
            chip.valueData = { kind: "Op", op: "True", args: [], saved: { kind: "Number", value: 42 } };
            startDrag();
            compare(restored().item.valueData, { kind: "Number", value: 42 });
        }
        function test_floatingRootLeavesNoRestoredValue() {
            chip.location = { kind: "Floating", floating_id: "f1", path: [] };
            startDrag();
            compare(restored().item, null);
        }
        function test_settlingKeepsSavedValueVisible() {
            BlockRegistry.settlingValueKey = BlockRegistry.locationKey(chip.location);
            tryCompare(chip, "draggedOut", true);
            compare(restored().item.valueData.value, "hello");
            BlockRegistry.settlingValueKey = "";
            compare(chip.draggedOut, false);
            compare(restored().item, null);
        }
    }
}
