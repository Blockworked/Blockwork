import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1000; height: 700
    Component {
        id: canvasComponent
        BlockCanvas {
            width: scene.width; height: scene.height
            dicts: [{name: "groceries", editor_visible: true, editor_x: 36, editor_y: 36, entries: []}]
            property var pendingItems: null
            property var editorMoves: []
            onDictEditorStateChanged: (name, visible, x, y) => editorMoves.push({name: name, visible: visible, x: x, y: y})
            onDictEntriesEdited: (name, entries) => {
                pendingItems = entries;
                reply.restart();
            }
            Timer {
                id: reply
                interval: 20
                onTriggered: {
                    const next = JSON.parse(JSON.stringify(dicts));
                    next[0].entries = pendingItems;
                    dicts = next;
                }
            }
        }
    }
    TestCase {
        name: "DictEditor"
        when: windowShown
        function setup() {
            const canvas = createTemporaryObject(canvasComponent, scene);
            verify(!!canvas);
            wait(30);
            return canvas;
        }
        function field(canvas, index) {
            return findChild(canvas, "dictKey_" + index);
        }
        function test_overlay_stays_fixed_when_panning_and_zooming() {
            const canvas = setup();
            const card = findChild(canvas, "dictEditor_groceries");
            const before = card.mapToItem(scene, 0, 0);
            const flick = canvas.children.find(child => child.contentX !== undefined);
            verify(!!flick);
            flick.contentX += 120;
            flick.contentY += 80;
            canvas.zoom = 1.5;
            wait(30);
            const after = card.mapToItem(scene, 0, 0);
            compare(after.x, before.x);
            compare(after.y, before.y);
            compare(card.mapToItem(scene, card.width, 0).x - after.x, 360);
        }
        function test_header_keeps_drag_during_state_refresh() {
            const canvas = setup();
            const card = findChild(canvas, "dictEditor_groceries");
            const flick = canvas.children.find(child => child.contentX !== undefined);
            const originalX = flick.contentX, originalY = flick.contentY;
            const start = card.mapToItem(scene, 80, 18);
            mousePress(scene, start.x, start.y);
            mouseMove(scene, start.x + 30, start.y + 20, 20);
            mouseMove(scene, start.x + 60, start.y + 40, 20);
            const movedX = card.x, movedY = card.y;
            verify(movedX > 36);
            const refreshed = JSON.parse(JSON.stringify(canvas.dicts));
            refreshed[0].entries.push({key: "live", value: {kind: "Text", value: "update"}});
            canvas.dicts = refreshed;
            wait(20);
            compare(findChild(canvas, "dictEditor_groceries"), card);
            mouseMove(scene, start.x + 120, start.y + 80, 20);
            verify(card.x > movedX);
            verify(card.y > movedY);
            compare(canvas.editorMoves.length, 0);
            mouseRelease(scene, start.x + 120, start.y + 80);
            compare(canvas.editorMoves.length, 1);
            compare(canvas.editorMoves[0], {name: "groceries", visible: true, x: Math.round(card.x), y: Math.round(card.y)});
            compare(flick.contentX, originalX);
            compare(flick.contentY, originalY);
        }
        function test_header_owns_press_before_drag_threshold() {
            const canvas = setup();
            const card = findChild(canvas, "dictEditor_groceries");
            const header = findChild(card, "dictHeaderDrag");
            verify(!!header);
            mousePress(card, 80, 18);
            verify(header.pressed);
            mouseRelease(card, 80, 18);
            compare(canvas.editorMoves.length, 0);
        }
        function test_close_button_still_works() {
            const canvas = setup();
            const card = findChild(canvas, "dictEditor_groceries");
            function descendants(item) {
                let result = [item];
                for (const child of item.children || []) result = result.concat(descendants(child));
                return result;
            }
            const close = descendants(card).find(item => item.iconName === "x");
            verify(!!close);
            tryVerify(() => close.width > 0 && close.height > 0);
            waitForRendering(close);
            mouseMove(close, close.width / 2, close.height / 2, 0, Qt.NoButton);
            mouseClick(close, close.width / 2, close.height / 2);
            tryVerify(() => canvas.editorMoves.length === 1);
            compare(canvas.editorMoves[0].visible, false);
        }
        function test_repeated_header_drags_with_position_echo() {
            const canvas = setup();
            const card = findChild(canvas, "dictEditor_groceries");
            const flick = canvas.children.find(child => child.contentX !== undefined);
            const originalX = flick.contentX, originalY = flick.contentY;
            for (let attempt = 0; attempt < 5; ++attempt) {
                const startX = card.x, startY = card.y;
                const start = card.mapToItem(scene, 100, 18);
                mousePress(scene, start.x, start.y);
                for (let step = 1; step <= 20; ++step) {
                    mouseMove(scene, start.x + step * 3, start.y + step * 2, 5);
                    if (step === 8) {
                        const refreshed = JSON.parse(JSON.stringify(canvas.dicts));
                        refreshed[0].editor_x += 1;
                        refreshed[0].editor_y += 1;
                        canvas.dicts = refreshed;
                    }
                }
                mouseRelease(scene, start.x + 60, start.y + 40);
                verify(card.x > startX + 30);
                verify(card.y > startY + 20);
                compare(canvas.editorMoves.length, attempt + 1);
                const echoed = JSON.parse(JSON.stringify(canvas.dicts));
                echoed[0].editor_x = canvas.editorMoves[attempt].x;
                echoed[0].editor_y = canvas.editorMoves[attempt].y;
                canvas.dicts = echoed;
                wait(20);
                compare(flick.contentX, originalX);
                compare(flick.contentY, originalY);
            }
        }
        function test_drag_stops_at_viewport_edges() {
            const canvas = setup();
            const card = findChild(canvas, "dictEditor_groceries");
            for (const delta of [{x: 1200, y: 900}, {x: -1200, y: -900}]) {
                const start = card.mapToItem(scene, 80, 18);
                mousePress(scene, start.x, start.y);
                mouseMove(scene, start.x + Math.sign(delta.x) * 20, start.y + Math.sign(delta.y) * 20, 20);
                mouseMove(scene, start.x + delta.x, start.y + delta.y, 20);
                mouseRelease(scene, start.x + delta.x, start.y + delta.y);
                compare(card.x, delta.x > 0 ? canvas.width - card.width : 0);
                compare(card.y, delta.y > 0 ? canvas.height - card.height : 0);
            }
        }
        function test_saved_position_and_resize_stay_visible() {
            const canvas = setup();
            const restored = JSON.parse(JSON.stringify(canvas.dicts));
            restored[0].editor_x = -100;
            restored[0].editor_y = 2000;
            canvas.dicts = restored;
            const card = findChild(canvas, "dictEditor_groceries");
            compare(card.x, 0);
            compare(card.y, Math.max(0, canvas.height - card.height));
            card.x = 600;
            canvas.width = 380;
            canvas.height = 130;
            compare(card.x, canvas.width - card.width);
            compare(card.y, Math.max(0, canvas.height - card.height));
            canvas.width = 200;
            canvas.height = 80;
            compare(card.x, 0);
            compare(card.y, 0);
        }
        function test_add_preserves_current_edit() {
            const canvas = setup();
            mouseClick(findChild(canvas, "addDictEntry"));
            tryVerify(() => field(canvas, 0) && field(canvas, 0).activeFocus);
            keyClick("a");
            findChild(canvas, "dictValue_0").text = "42";
            mouseClick(findChild(canvas, "addDictEntry"));
            tryVerify(() => field(canvas, 1) && field(canvas, 1).activeFocus);
            compare(canvas.dicts[0].entries[0], {key:"a", value:{kind:"Number", value:42}});
        }
        function test_add_and_enter_focus_each_new_entry_data() {
            return [{tag:"key-return", field:"dictKey_", key:Qt.Key_Return},
                {tag:"value-enter", field:"dictValue_", key:Qt.Key_Enter}];
        }
        function test_add_and_enter_focus_each_new_entry(data) {
            const canvas = setup();
            mouseClick(findChild(canvas, "addDictEntry"));
            tryVerify(() => field(canvas, 0) && field(canvas, 0).activeFocus);
            field(canvas, 0).text = "score";
            const value = findChild(canvas, "dictValue_0");
            value.text = "42";
            findChild(canvas, data.field + "0").forceActiveFocus();
            keyClick(data.key);
            tryVerify(() => field(canvas, 1) && field(canvas, 1).activeFocus);
            compare(canvas.dicts[0].entries, [{key:"score", value:{kind:"Number", value:42}}, {key:"", value:{kind:"Text", value:""}}]);
            for (let i = 2; i < 14; ++i) {
                keyClick(Qt.Key_Return);
                tryVerify(() => field(canvas, i) && field(canvas, i).activeFocus);
            }
            const last = field(canvas, 13);
            const card = findChild(canvas, "dictEditor_groceries");
            const position = last.mapToItem(card, 0, 0);
            verify(position.y >= 44);
            verify(position.y + last.height <= card.height - 46);
        }
    }
}
