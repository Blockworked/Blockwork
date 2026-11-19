import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    width: 640; height: 480
    PromptWindow {
    id: dialog
    property string blockworkAppearance: "dark"
    TestCase {
        id: tests
        name: "MacroPrompts"
        when: windowShown
        property var bridge
        SignalSpy { id: replies; target: tests.bridge; signalName: "finished" }
        function initTestCase() {
            bridge = findChild(dialog, "promptBridge");
            verify(bridge !== null);
        }
        function init() {
            bridge.parentRunning = true;
            dialog.answered = false;
            replies.clear();
        }
        function control(name) {
            function walk(item) {
                if (!item) return null;
                if (item.objectName === name) return item;
                if (item.contentItem && item.contentItem !== item) {
                    const found = walk(item.contentItem);
                    if (found) return found;
                }
                for (const child of (item.children || [])) {
                    const found = walk(child);
                    if (found) return found;
                }
                return null;
            }
            let result = null;
            tryVerify(() => { result = walk(dialog.contentItem); return result !== null; });
            return result;
        }
        function configure(kind, extra) {
            const request = Object.assign({kind: kind, title: "Prompt test", message: "Choose an answer", default: "", options: []}, extra || {});
            bridge.requestJson = JSON.stringify(request);
            dialog.requestActivate();
            tryVerify(() => dialog.active);
            waitForRendering(dialog.contentItem);
        }
        function reply(answer, text) {
            tryCompare(replies, "count", 1);
            compare(replies.signalArguments[0][0], answer);
            compare(replies.signalArguments[0][1], text);
        }
        function test_text_keyboard_and_return() {
            configure("text");
            const field = control("promptText");
            tryCompare(field, "activeFocus", true);
            compare(field.Accessible.name, "Choose an answer");
            keyClick(Qt.Key_A);
            keyClick(Qt.Key_B);
            compare(field.text, "ab");
            keyClick(Qt.Key_Return);
            reply("ok", "ab");
        }
        function test_choice_keyboard_selects_second() {
            configure("choice", {options: ["First", "Second"]});
            const list = control("promptChoices");
            tryCompare(list, "activeFocus", true);
            keyClick(Qt.Key_Down);
            compare(list.currentIndex, 1);
            const second = control("promptChoice_1");
            compare(second.Accessible.name, "Second");
            compare(second.Accessible.selected, true);
            keyClick(Qt.Key_Return);
            reply("ok", "Second");
        }
        function test_confirm_yes() {
            configure("confirm");
            const button = control("promptButton_yes");
            verify(button !== null);
            mouseClick(button);
            reply("yes", "");
        }
        function test_message_return() {
            configure("message");
            keyClick(Qt.Key_Return);
            reply("ok", "");
        }
        function test_parent_exit_cancels_prompt() {
            configure("message");
            bridge.parentRunning = false;
            reply("cancel", "");
        }
        function test_file_path_typed() {
            configure("file");
            const field = control("promptPath");
            tryCompare(field, "activeFocus", true);
            verify(control("promptBrowse") !== null);
            keyClick(Qt.Key_Return);
            compare(replies.count, 0);
            field.text = "/tmp/notes.txt";
            keyClick(Qt.Key_Return);
            reply("ok", "/tmp/notes.txt");
        }
        function test_menu_keyboard_picks_second() {
            configure("menu", {options: ["Copy", "Paste"], x: 40, y: 40});
            const list = control("promptMenu");
            tryCompare(list, "activeFocus", true);
            keyClick(Qt.Key_Down);
            keyClick(Qt.Key_Return);
            reply("ok", "Paste");
        }
        function test_tooltip_closes_after_its_time() {
            bridge.requestJson = JSON.stringify({kind: "tooltip", title: "tip", message: "Saved!", duration_ms: 200, default: "BottomRight"});
            compare(control("promptTooltip").text, "Saved!");
            reply("ok", "");
            verify(dialog.width < 440);
        }
        function test_local_path_from_url() {
            compare(dialog.localPath("file:///home/me/My%20File.txt"), "/home/me/My File.txt");
            compare(dialog.localPath("file:///C:/Users/me/a.txt"), "C:/Users/me/a.txt");
        }
        function test_text_cancel() {
            configure("text", {default: "unchanged"});
            mouseClick(control("promptButton_cancel"));
            reply("cancel", "");
        }
    }
}
}
