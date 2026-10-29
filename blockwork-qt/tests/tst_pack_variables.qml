import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 900; height: 760
    property string invoked: ""
    property var args: ({})
    QtObject {
        id: bridge
        property int replySerial: 0
        property string replyKey: ""
        property string replyError: ""
        property string replyJson: "null"
        function callCommand(key, command, args) {
            if (command === "pack_variable_candidates") {
                replyKey = key;
                replyJson = JSON.stringify(["scratch", "count", "width"]);
                replySerial++;
            }
        }
    }
    BlockLibrary {
        id: library
        appState: ({current_macro: {id:"macro", variables:["my variable"], block_defs:[{id:"block",pieces:[{kind:"Label",text:"example"}]}], pack_data:[{
            id:"example", name:"Example", variables:[
                {name:"Example: width", label:"width", role:"setting", value:{kind:"Number",value:320}},
                {name:"Example: scratch", label:"scratch", role:"internal", value:{kind:"Number",value:7}},
                {name:"Example: count", label:"count", role:"output", value:{kind:"Number",value:2}}
            ]
        }]}})
        invoke: function(command, args) { scene.invoked = command; scene.args = args; }
        appBridge: bridge
        macroFiles: null
    }
    TestCase {
        name: "PackVariables"
        when: windowShown
        function test_pack_authors_choose_private_outputs_and_settings() {
            library.share({id:"block"});
            const dialog = findChild(library, "sharePackDialog");
            tryCompare(dialog, "opened", true);
            compare(dialog.variableCandidates.length, 3);
            compare(dialog.showVariables, false);
            dialog.showVariables = true;
            const rows = findChild(dialog, "packVariableRoleRows");
            compare(rows.count, 3);
            const output = findChild(rows.itemAt(1), "packVariableRole");
            output.currentIndex = 1;
            output.activated(1);
            compare(dialog.variableRoles.count, "output");
            const setting = findChild(rows.itemAt(2), "packVariableRole");
            setting.currentIndex = 2;
            setting.activated(2);
            compare(dialog.variableRoles.width, "setting");
            verify(!dialog.variableRoles.scratch);
            dialog.close();
        }
        function test_settings_and_collapsed_inspector() {
            library.showPackData("example");
            const dialog = findChild(library, "packDataDialog");
            tryCompare(dialog, "opened", true);
            compare(dialog.settings.length, 1);
            compare(dialog.inspect, false);
            const rows = findChild(dialog, "packSettingRows");
            compare(rows.count, 1);
            const field = findChild(rows.itemAt(0), "packSettingValue");
            compare(field.text, "320");
            field.text = "480";
            field.editingFinished();
            compare(scene.invoked, "set_pack_setting");
            compare(scene.args.name, "Example: width");
            compare(scene.args.value.value, 480);
            field.text = "bad";
            field.editingFinished();
            compare(field.text, "320");
            mouseClick(findChild(dialog, "packInspectButton"));
            compare(dialog.inspect, true);
            dialog.close();
            library.showPackData("example");
            compare(dialog.inspect, false);
            dialog.close();
        }
    }
}
