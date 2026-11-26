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
        SignalSpy { id: sent; target: tests.bridge; signalName: "sent" }
        function initTestCase() {
            bridge = findChild(dialog, "promptBridge");
            verify(bridge !== null);
        }
        function init() {
            bridge.parentRunning = true;
            dialog.answered = false;
            replies.clear();
            sent.clear();
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
        function test_window_controls_send_values_on_each_click() {
            configure("window", {controls: [
                {kind: "note", label: "Fill this in"},
                {kind: "text", label: "Name", name: "who", value: "Ann"},
                {kind: "check", label: "Loud", name: "loud", value: false},
                {kind: "choice", label: "Color", name: "color", value: "Blue", options: ["Red", "Blue"]},
                {kind: "button", label: "Apply"},
                {kind: "button", label: "Done", close: true}]});
            const name = control("promptFormText_1");
            compare(name.text, "Ann");
            name.forceActiveFocus();
            name.selectAll();
            keyClick(Qt.Key_B);
            compare(control("promptFormChoice_3").currentText, "Blue");
            mouseClick(control("promptFormCheck_2"));
            mouseClick(control("promptFormButton_Apply"));
            tryCompare(sent, "count", 1);
            compare(sent.signalArguments[0][0], "click");
            compare(sent.signalArguments[0][1], "Apply");
            compare(sent.signalArguments[0][2], JSON.stringify({who: "b", loud: true, color: "Blue"}));
            compare(dialog.answered, false);
            mouseClick(control("promptFormButton_Done"));
            tryCompare(sent, "count", 2);
            compare(sent.signalArguments[1][0], "ok");
            compare(sent.signalArguments[1][1], "Done");
            compare(replies.count, 0);
        }
        function test_live_window_updates_in_place() {
            configure("window", {live: true, inline_buttons: true, width: 600, height: 440, controls: [
                {kind: "row", id: "row", label: "", children: [
                    {kind: "text", id: "field", label: "Name", name: "__proto__", value: "Ann"},
                    {kind: "note", id: "mirror", label: "Ann"}
                ]},
                {kind: "scroll", id: "scroll", children: [
                    {kind: "grid", columns: 2, children: [
                        {kind: "table", label: "Files", value: '[["Name","Size"],["Example",10]]'},
                        {kind: "progress", label: "Used", value: "30"}
                    ]}
                ]},
                {kind: "tabs", id: "tabs", children: [
                    {kind: "group", id: "first", label: "One", children: [{kind: "heading", label: "First page"}]},
                    {kind: "group", id: "second", label: "Two", children: [{kind: "heading", label: "Second page"}]}
                ]},
                {kind: "button", id: "button1", label: "Done", close: true}
            ]});
            const name = control("promptFormText_0");
            compare(name.text, "Ann");
            compare(dialog.width, 600);
            compare(dialog.height, 440);
            name.forceActiveFocus();
            name.selectAll();
            keyClick(Qt.Key_B);
            tryCompare(sent, "count", 1);
            compare(sent.signalArguments[0][0], "change");
            compare(JSON.parse(sent.signalArguments[0][2])["__proto__"], "b");
            dialog.applyFormUpdates([{controls: {field: {value: "Old"}}}]);
            compare(name.text, "b");
            dialog.applyFormUpdates([{ack: "1", controls: {field: {value: "b"}, mirror: {label: "b"}},
                size: [640, 480], title: "Updated"}]);
            compare(dialog.title, "Updated");
            compare(dialog.width, 640);
            compare(control("promptFormLabel_1").text, "b");
            compare(control("promptFormText_0"), name);
            mouseClick(control("promptFormTab_second"));
            compare(control("promptFormLabel_0").text, "Second page");
            dialog.applyFormUpdates([{controls: {row: {enabled: false}}}]);
            compare(name.enabled, false);
            compare(control("promptFormLabel_0").text, "Second page");
            mouseClick(control("promptFormTab_first"));
            compare(control("promptFormLabel_0").text, "First page");
            dialog.applyFormUpdates([{controls: {first: {visible: false}}}]);
            compare(control("promptFormLabel_0").text, "Second page");
            mouseClick(control("promptFormButton_Done"));
            tryCompare(sent, "count", 2);
            compare(sent.signalArguments[1][0], "ok");
            compare(sent.signalArguments[1][1], "button1");
            compare(sent.signalArguments[1][2], "{}");
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
        function test_fill_height_opens_a_tall_enough_window() {
            configure("window", {live: true, controls: [
                {kind: "text", id: "a", label: "A", name: "a", value: "", size: "h:fill"},
                {kind: "button", id: "done", label: "Done", close: true}]});
            verify(dialog.height >= 360);
        }
        function test_display_kinds_set_their_own_type_size() {
            configure("window", {live: true, controls: [
                {kind: "heading", id: "h", label: "Title"},
                {kind: "subheading", id: "s", label: "Sub"},
                {kind: "body", id: "b", label: "Body"},
                {kind: "caption", id: "c", label: "Cap"},
                {kind: "button", id: "done", label: "Done", close: true}]});
            compare(control("promptFormLabel_0").font.pixelSize, 22);
            compare(control("promptFormLabel_1").font.pixelSize, 16);
            compare(control("promptFormLabel_2").font.pixelSize, 14);
            compare(control("promptFormLabel_3").font.pixelSize, 12);
        }
        function test_button_variants_override_the_close_look() {
            configure("window", {live: true, controls: [
                {kind: "button", id: "p", label: "Primary", variant: "primary", icon: "check"},
                {kind: "button", id: "d", label: "Danger", variant: "danger"},
                {kind: "button", id: "q", label: "Quiet", variant: "quiet"},
                {kind: "button", id: "done", label: "Done", close: true}]});
            const primary = control("promptFormButton_Primary");
            compare(primary.primary, true);
            compare(primary.iconName, "check");
            compare(control("promptFormButton_Danger").danger, true);
            compare(control("promptFormButton_Quiet").flat, true);
            compare(control("promptFormButton_Done").primary, true);
        }
        function test_selectable_table_sorts_and_writes_the_row() {
            configure("window", {live: true, controls: [
                {kind: "table-input", id: "rows", label: "Rows", name: "picked", value: "",
                 data: '[["Name","Size"],["B","2"],["A","1"]]'},
                {kind: "button", id: "done", label: "Done", close: true}]});
            const table = control("promptFormTable_0");
            compare(table.header.length, 2);
            compare(table.body.length, 2);
            table.sortBy(0);
            compare(table.body[0][0], "A");
            table.sortBy(0);
            compare(table.body[0][0], "B");
            table.selected(["A", "1"]);
            tryCompare(sent, "count", 1);
            compare(sent.signalArguments[0][0], "change");
            compare(JSON.parse(sent.signalArguments[0][2]).picked, '{"Name":"A","Size":"1"}');
        }
    }
}
}
