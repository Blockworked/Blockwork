import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 900; height: 760
    property var savedExports: []
    property var definitions: [
        {id: "long", shape: "Normal", color: "#51b84b", pieces: [
            {kind: "Label", text: "Aim to Minecraft Player"},
            {kind: "Input", name: "gain", default_value: {kind: "Number", value: 1}},
            {kind: "Label", text: "max step deadzone hold (ms) max hold (ms) with a very long label"}
        ]},
        {id: "reporter", shape: "ReturnsValue", pieces: [{kind: "Label", text: "player count"}]},
        {id: "bool", shape: "ReturnsBool", pieces: [{kind: "Label", text: "aim active"}]},
        {id: "branch", shape: "Normal", pieces: [{kind: "Label", text: "aim while"}, {kind: "Branch", name: "body"}]}
    ]
    QtObject {
        id: bridge
        property int replySerial: 0
        property string replyKey: ""
        property string replyError: ""
        property string replyJson: "null"
        function callCommand(key, command, args) {
            replyKey = key;
            replyJson = JSON.stringify(["player count", "aim status"]);
            replySerial++;
        }
    }
    BlockLibrary {
        id: library
        appState: ({current_macro: {name: "Example", block_defs: scene.definitions, pack_exports: scene.savedExports}})
        invoke: function() {}
        appBridge: bridge
        macroFiles: null
    }
    TestCase {
        name: "PackShare"
        when: windowShown
        function cleanup() {
            findChild(library, "sharePackDialog").close();
            scene.savedExports = [];
            scene.width = 900;
            scene.height = 760;
        }
        function test_restores_pack_metadata_and_version_without_leaking_to_other_blocks() {
            scene.savedExports = [{info: {id: "aim", name: "Aim helpers", description: "Aim at the nearest player.",
                author: "Alice", version: "1.3.0"}, block_ids: ["long", "reporter"],
                variable_roles: [{name: "player count", role: "output"}]}];
            library.share(definitions[0]);
            const dialog = findChild(library, "sharePackDialog");
            tryCompare(dialog, "opened", true);
            compare(findChild(dialog, "sharePackName").text, "Aim helpers");
            const description = findChild(dialog, "sharePackDescription");
            const version = findChild(dialog, "sharePackVersion");
            compare(description.text, "Aim at the nearest player.");
            compare(findChild(dialog, "sharePackAuthor").text, "Alice");
            compare(version.text, "1.3.0");
            compare(dialog.picked.length, 2);
            compare(dialog.variableRoles["player count"], "output");
            description.text = "Unsaved edit";
            version.text = "9.0.0";
            dialog.close();
            library.share(definitions[1]);
            tryCompare(dialog, "opened", true);
            compare(description.text, "Aim at the nearest player.");
            compare(version.text, "1.3.0");
            dialog.close();
            library.share(definitions[2]);
            tryCompare(dialog, "opened", true);
            compare(description.text, "");
            compare(version.text, "1.0.0");
            compare(findChild(dialog, "sharePackAuthor").text, "");
            compare(dialog.picked.length, 1);
            compare(dialog.picked[0], "bool");
        }
        function test_constrained_form_and_real_previews_data() {
            return [{tag: "desktop", width: 900, height: 760}, {tag: "small", width: 420, height: 540}];
        }
        function test_constrained_form_and_real_previews(data) {
            scene.width = data.width;
            scene.height = data.height;
            library.share(definitions[0]);
            const dialog = findChild(library, "sharePackDialog");
            tryCompare(dialog, "opened", true);
            dialog.showVariables = true;
            wait(100);
            verify(dialog.width <= scene.width - 32);
            verify(dialog.height <= scene.height - 32);
            const scroll = findChild(dialog, "sharePackScroll");
            const name = findChild(dialog, "sharePackName");
            verify(name.width <= scroll.availableWidth);
            verify(name.text.indexOf("(gain)") < 0);
            name.text = Array(50).join("long pack name ");
            wait(50);
            verify(name.width <= scroll.availableWidth);
            const rows = findChild(dialog, "sharePackBlockRows");
            compare(rows.count, 4);
            for (let i = 0; i < rows.count; ++i) {
                const row = rows.itemAt(i);
                verify(row.width <= scroll.availableWidth);
                const loader = findChild(row, "sharePackBlockPreview");
                verify(!!loader.item);
                verify(loader.width > 0 && loader.height > 0);
                if (i === 0 || i === 3) {
                    compare(loader.item.instruction.block_id, definitions[i].id);
                    compare(loader.item.instruction.type, i === 3 ? "BranchCallBlock" : "CallBlock");
                } else {
                    compare(loader.item.valueData.kind, "Call");
                    compare(loader.item.forceBoolean, i === 2);
                }
            }
            const viewport = findChild(rows.itemAt(0), "sharePackBlockViewport");
            verify(viewport.contentWidth > viewport.width);
            verify(viewport.clip);
            const check = findChild(rows.itemAt(0), "sharePackBlockCheck");
            compare(check.checked, true);
            mouseClick(check);
            compare(dialog.picked.length, 0);
            mouseClick(check);
            compare(dialog.picked[0], "long");
        }
    }
}
