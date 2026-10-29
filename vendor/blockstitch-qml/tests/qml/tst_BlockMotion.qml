import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    width: 1000; height: 700
    QtObject {
        id: drag
        property bool active: false
        property bool settling: false
        property bool applyingDocument: false
        property var snapLayout: null
    }
    InstructionBlock {
        id: bracket
        strandId: "target"; path: [{ index: 0 }]
        instruction: ({ id: "bracket", type: "If", body: [] })
        dragState: drag
    }
    InstructionBlock {
        id: row
        strandId: "target"; path: [{ index: 0, slot: 0 }, { index: 0 }]
        instruction: ({ id: "row", type: "Wait" })
        dragState: drag
    }
    TestCase {
        name: "BlockMotion"
        when: windowShown

        function init() {
            Motion.durationScale = 0;
            drag.snapLayout = null;
            wait(30);
        }
        function cleanup() {
            Motion.durationScale = 0;
            drag.snapLayout = null;
            Motion.durationScale = 1;
        }
        function test_layoutFollowsAnimationSpeed_data() {
            return [
                { tag: "instant", scale: 0 },
                { tag: "fast", scale: 0.25 },
                { tag: "normal", scale: 1 },
                { tag: "slow", scale: 3 }
            ];
        }
        function test_layoutFollowsAnimationSpeed(data) {
            const initialWidth = bracket.implicitWidth;
            const initialHeight = bracket.implicitHeight;
            const targetWidth = bracket.spine + 700 + 24;
            Motion.durationScale = data.scale;
            drag.snapLayout = {
                snapValid: true, snapTargetId: "target",
                snapPath: [{ index: 0, slot: 0 }, { index: 0 }],
                snapWidth: 700, snapGrowAmt: 100, snapShiftAmt: 100
            };
            function checkFinished() {
                compare(bracket.implicitWidth, targetWidth);
                compare(bracket.implicitHeight, initialHeight + 100);
                compare(row.displayedSnapShift, 100);
            }
            if (data.scale === 0) {
                checkFinished();
                return;
            }
            compare(bracket.implicitWidth, initialWidth);
            compare(bracket.implicitHeight, initialHeight);
            compare(row.displayedSnapShift, 0);
            const sampleTime = data.scale === 3 ? 200 : 70;
            wait(sampleTime);
            if (data.scale === 0.25) checkFinished();
            else {
                verify(bracket.implicitWidth > initialWidth && bracket.implicitWidth < targetWidth);
                verify(bracket.implicitHeight > initialHeight && bracket.implicitHeight < initialHeight + 100);
                verify(row.displayedSnapShift > 0 && row.displayedSnapShift < 100);
                wait(140 * data.scale - sampleTime + 60);
                checkFinished();
            }
        }
    }
}
