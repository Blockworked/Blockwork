import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 800; height: 700
    QtObject {
        id: bridge
        property int replySerial: 0
        property string replyKey: ""
        property string replyJson: "null"
        property string replyError: ""
        property bool dependencies: false
        property bool deferReplies: false
        property var pending: []
        function reply(request) {
            replyKey = request.key;
            replyJson = JSON.stringify(request.ids);
            replySerial++;
        }
        function callCommand(key, command, args) {
            if (command !== "bundle_macro_ids") return;
            const ids = JSON.parse(args).macroIds;
            const included = dependencies && ids.indexOf("a") >= 0 ? ["a", "b", "c"] : ids;
            const request = { key: key, ids: included };
            if (deferReplies) pending.push(request);
            else Qt.callLater(() => reply(request));
        }
    }
    Component {
        id: filesComponent
        MacroFiles {
            appState: ({ library: [
                { id: "a", name: "First", folder: "Work" },
                { id: "b", name: "Second", folder: "" },
                { id: "c", name: "Third", folder: "Work" }
            ] })
            invoke: function(command, args) {}
            appBridge: bridge
        }
    }
    TestCase {
        name: "BundleExport"
        when: windowShown
        function init() {
            bridge.dependencies = false;
            bridge.deferReplies = false;
            bridge.pending = [];
        }

        function test_dependencies_are_checked_disabled_and_update() {
            bridge.dependencies = true;
            const files = createTemporaryObject(filesComponent, scene);
            files.exportBundle(["a"], "First");
            const picker = findChild(files, "bundlePicker");
            tryCompare(picker, "opened", true);
            tryCompare(picker, "includedIds", ["a", "b", "c"]);
            const second = findChild(picker.contentItem, "bundleMacro_b");
            const third = findChild(picker.contentItem, "bundleMacro_c");
            verify(second.checked && !second.enabled);
            verify(third.checked && !third.enabled);
            verify(second.opacity < 1);
            mouseClick(second);
            compare(picker.macroIds, ["a"]);
            picker.selectedIds = ["b"];
            tryCompare(picker, "includedIds", ["b"]);
            verify(second.checked && second.enabled);
            verify(!third.checked && third.enabled);
            picker.reject();
        }

        function test_old_dependency_reply_does_not_replace_new_selection() {
            bridge.dependencies = true;
            bridge.deferReplies = true;
            const files = createTemporaryObject(filesComponent, scene);
            files.exportBundle(["a"], "First");
            const picker = findChild(files, "bundlePicker");
            picker.selectedIds = ["b"];
            compare(bridge.pending.length, 2);
            bridge.reply(bridge.pending[1]);
            compare(picker.includedIds, ["b"]);
            bridge.reply(bridge.pending[0]);
            compare(picker.includedIds, ["b"]);
            picker.reject();
        }

        function test_choose_multiple_macros_before_saving() {
            const files = createTemporaryObject(filesComponent, scene);
            files.exportBundle(["a"], "First");
            const picker = findChild(files, "bundlePicker");
            const save = findChild(files, "bundleSaveDialog");
            tryCompare(picker, "opened", true);
            verify(!save.visible);
            compare(picker.macroIds, ["a"]);
            const second = findChild(picker.contentItem, "bundleMacro_b");
            verify(!!second);
            mouseClick(second);
            compare(picker.macroIds, ["a", "b"]);
            mouseClick(picker.standardButton(Dialog.Ok));
            compare(save.macroIds, ["a", "b"]);
            compare(save.bundleName, "First");
            save.reject();
        }

        function test_folder_preselection_empty_selection_and_cancel() {
            const files = createTemporaryObject(filesComponent, scene);
            files.exportBundle(["a", "c"], "Work");
            const picker = findChild(files, "bundlePicker");
            const save = findChild(files, "bundleSaveDialog");
            tryCompare(picker, "opened", true);
            compare(picker.macroIds, ["a", "c"]);
            picker.selectedIds = [];
            verify(!picker.standardButton(Dialog.Ok).enabled);
            picker.selectedIds = ["b"];
            verify(picker.standardButton(Dialog.Ok).enabled);
            picker.reject();
            verify(!save.visible);
            files.exportBundle(["a"], "First");
            compare(picker.macroIds, ["a"]);
            picker.reject();
        }
    }
}
