import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    width: 600; height: 600
    QtObject {
        id: drag
        property bool active: false
        property bool settling: false
        property bool applyingDocument: false
        property bool snapValid: false
        property string snapTargetId: "target"
        property var snapPath: [{ index: 0, slot: 0 }, { index: 0 }]
        property real snapGrowAmt: 100
        property string sourceStrandId: ""
        property var sourceBasePath: []
        property real sourceShrink: 0
    }
    InstructionBlock {
        id: block
        strandId: "target"
        path: [{ index: 0 }]
        instruction: ({ id: "wrap", type: "IfElse", then_body: [], else_body: [] })
        dragState: drag
    }
    TestCase {
        name: "BracketLabel"
        when: windowShown

        function descendants(item) {
            let result = [item];
            for (const child of item.children || []) result = result.concat(descendants(child));
            return result;
        }
        function label() { return descendants(block).find(item => item.text === "else"); }
        function checkAlignment() {
            const text = label();
            verify(!!text);
            const expected = block.headHeight + block.effMouthHeights[0]
                + Math.round((block.midHeight - text.height) / 2);
            compare(text.mapToItem(block, 0, 0).y, expected);
        }
        function init() {
            drag.active = false;
            drag.snapValid = false;
            drag.sourceStrandId = "";
            drag.sourceBasePath = [];
            drag.sourceShrink = 0;
            block.instruction = { id: "wrap", type: "IfElse", then_body: [], else_body: [] };
            wait(180);
        }
        function test_previewGrowthAndCollapse() {
            checkAlignment();
            const initial = block.effMouthHeights[0];
            drag.snapValid = true;
            wait(50);
            verify(block.effMouthHeights[0] > initial);
            verify(block.effMouthHeights[0] < initial + drag.snapGrowAmt);
            checkAlignment();
            wait(160);
            checkAlignment();
            drag.snapValid = false;
            wait(50);
            checkAlignment();
            wait(160);
            compare(block.effMouthHeights[0], initial);
            checkAlignment();
        }
        function test_sourceMouthCollapse() {
            block.instruction = { id: "wrap", type: "IfElse",
                then_body: [{ id: "child", type: "If", body: [] }], else_body: [] };
            wait(30);
            const initial = block.effMouthHeights[0];
            drag.sourceStrandId = "target";
            drag.sourceBasePath = [{ index: 0, slot: 0 }];
            drag.sourceShrink = initial;
            drag.active = true;
            wait(50);
            verify(block.effMouthHeights[0] < initial);
            checkAlignment();
            wait(160);
            checkAlignment();
        }
    }
}
