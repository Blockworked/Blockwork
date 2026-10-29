import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    width: 1200; height: 900
    BlockCanvas { id: canvas; anchors.fill: parent }
    SignalSpy { id: edited; target: canvas; signalName: "commentEdited" }
    TestCase {
        name: "Comments"
        when: windowShown
        function init() {
            canvas.strands = [{id: "strand", x: -250, y: -100, instructions: [
                {id: "outer", type: "If", body: [{id: "nested", type: "Wait", ms: 10}]}
            ]}];
            canvas.comments = [{id: "note", x: 100, y: -100, text: "hello", collapsed: false, attached_to: "nested"}];
            edited.clear();
            wait(50);
        }
        function cleanup() {
            canvas.forceActiveFocus();
            canvas.comments = [];
            wait(20);
            compare(findChild(canvas, "commentConnector_note"), null);
        }
        function test_labelAndStandalone() {
            const card = findChild(canvas, "commentCard_note");
            function hasLabel(item) {
                if (item.text === "COMMENT") return true;
                return (item.children || []).some(hasLabel);
            }
            verify(hasLabel(card));
            verify(findChild(canvas, "commentConnector_note").visible);
            canvas.comments = [{id: "note", x: 100, y: -100, text: "hello", collapsed: false}];
            wait(30);
            verify(!findChild(canvas, "commentConnector_note").visible);
        }
        function test_scrollLongTextAndSave() {
            const text = findChild(canvas, "commentText_note");
            const scroll = findChild(canvas, "commentScroll_note");
            const card = findChild(canvas, "commentCard_note");
            text.forceActiveFocus();
            text.text = Array(80).fill("A line of comment text").join("\n");
            text.cursorPosition = text.length;
            wait(50);
            compare(card.height, 279);
            verify(scroll.contentHeight > scroll.height);
            const flick = scroll.contentItem;
            verify(flick.contentY > 0, "Cursor must scroll into view");
            flick.contentY = 0;
            const before = flick.contentY;
            const canvasY = card.mapToItem(canvas, 0, 0).y;
            mouseWheel(scroll, scroll.width / 2, scroll.height / 2, 0, -120);
            wait(150);
            verify(flick.contentY > before, "Wheel must scroll comment text");
            compare(card.mapToItem(canvas, 0, 0).y, canvasY, "Comment scrolling must not pan the canvas");
            canvas.forceActiveFocus();
            compare(edited.count, 1);
            compare(edited.signalArguments[0][1], text.text);
        }
        function test_connectorFollowsNestedBlock() {
            const card = findChild(canvas, "commentCard_note");
            const block = canvas.findBlockItem(canvas, "nested");
            verify(!!block);
            verify(card.attachedBlock === block);
            const startX = card.blockAnchor.x;
            const startY = card.blockAnchor.y;
            block.parent.x += 40;
            block.parent.y += 60;
            compare(card.blockAnchor.x, startX + 40);
            compare(card.blockAnchor.y, startY + 60);
            const movedX = card.blockAnchor.x;
            const movedY = card.blockAnchor.y;
            const drag = canvas.branchDragState;
            drag.strandId = "strand";
            drag.path = [{index: 0}];
            drag.active = true;
            drag.dx = 35;
            drag.dy = 25;
            compare(card.blockAnchor.x, movedX + 35);
            compare(card.blockAnchor.y, movedY + 25);
            canvas.cancelDrag();
            canvas.endSettle();
            canvas.strands = [];
            wait(30);
            verify(!findChild(canvas, "commentConnector_note").visible);
        }
    }
}
