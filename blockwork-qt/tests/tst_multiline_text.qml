import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1100; height: 800
    property var definition: ({id: "post", shape: "Normal", pieces: [
        {kind: "Label", id: "name", text: "post to"},
        {kind: "Input", id: "url", name: "url", value_type: "Any"},
        {kind: "Input", id: "body", name: "body", value_type: "Any", multiline: true}
    ]})
    Component { id: rowFactory; InstructionBlock {
        strandId: "s"; path: [{index: 0}]
        blockDefinitions: [scene.definition]
    } }
    TestCase {
        name: "MultilineText"
        when: windowShown
        function named(item, name) {
            if (!item) return null;
            if (item.objectName === name) return item;
            for (const child of item.children || []) {
                const found = named(child, name);
                if (found) return found;
            }
            return null;
        }
        function test_customBlockCallDrawsItsMultilineInputAsABox() {
            const args = BlockRegistry.freshCallArgs(scene.definition);
            compare(args[1].kind, "Text", "a multi-line input starts as text, so lines can be typed");
            args[1] = {kind: "Text", value: "{\n  \"a\": 1\n}"};
            const row = createTemporaryObject(rowFactory, scene, {instruction: {type: "CallBlock", id: "call", block_id: "post", args: args, branches: []}});
            const boxes = named(row, "blockBoxes");
            verify(boxes && boxes.visible);
            const area = named(boxes, "multilineBoxArea");
            verify(area);
            compare(area.text, "{\n  \"a\": 1\n}");
            tryVerify(() => row.implicitHeight > 58 + 3 * 12);
        }
    }
}
