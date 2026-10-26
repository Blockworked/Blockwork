import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    property var edits: []
    property string failure: ""
    function invoke(command, args) { calls.push({command: command, args: args}); }
    QtObject {
        id: bridge
        property int replySerial: 0
        property int previewSerial: 0
        property string activeBlocksJson: "[]"
        property string installedAppsError: ""
        property string replyKey: ""
        property string replyJson: "null"
        property string replyError: ""
        function callCommand(key, command, args) {
            scene.calls.push({command: command, args: JSON.parse(args)});
            replyKey = key; replyError = scene.failure; replySerial++;
        }
    }
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: ({
                current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:["existing"], lists:[{name:"existing", items:[]}], comments:[],
                    floating_values:[{id:"param",x:40,y:250,value:{kind:"Param",name:"gain"},origin_block_id:"custom"}],
                    block_defs:[{id:"custom",identifier:"Custom",color:"#51b84b",shape:"Normal",pieces:[
                        {kind:"Label",text:"custom"}, {kind:"Input",name:"gain",value_type:"Number"},
                        {kind:"Input",name:"enabled",value_type:"Bool"}]}],
                    strands:[{id:"definition",x:40,y:40,instructions:[{id:"header",type:"BlockHeader",block_id:"custom"},{id:"wait",type:"Wait",duration:{kind:"Number",value:100}}]}]},
                recording_phase:{phase:"Idle"}, key_capture:null, can_undo:false, can_redo:false,
                record_mouse_movement:false, record_mouse_relative:false, absolute_mouse_position_available:true
            })
        }
    }
    Component {
        id: blockComponent
        InstructionBlock {
            strandId: "main"
            path: [{index: 0}]
            onInstructionEdited: (strandId, path, instruction) => scene.edits.push(instruction)
        }
    }
    Component {
        id: reporterComponent
        ValueChip {
            valueData: ({kind: "Op", op: "ListLength", args: [{kind: "Text", value: "existing"}]})
            onLeafEdited: (path, leaf) => scene.edits.push({path: path, leaf: leaf})
        }
    }
    TestCase {
        name: "ListCreation"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function setup(type, key) {
            scene.calls = []; scene.edits = []; scene.failure = "";
            const page = createTemporaryObject(editorComponent, scene);
            wait(50);
            const instruction = page.defaultInstruction(type);
            instruction[key] = "existing";
            const block = createTemporaryObject(blockComponent, scene, {instruction: instruction, lists: page.macro.lists});
            verify(!!block);
            wait(20);
            const dropdown = all(block).find(o => o.choices && o.choices.some(c => c.action && c.label === "Make a List…"));
            verify(!!dropdown, type + " has a list dropdown");
            dropdown.activated(dropdown.choices.length - 1);
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
            const dialog = findChild(page, "listDialog");
            verify(dialog.visible);
            return {page: page, block: block, dropdown: dropdown, dialog: dialog, name: findChild(page, "listName")};
        }
        function test_create_and_select_data() {
            return [
                {tag: "choice", type: "AskChoice", key: "list"},
                {tag: "split", type: "SplitIntoList", key: "name"},
                {tag: "detection", type: "DetectObjectsInWindow", key: "results_list"},
                {tag: "aim", type: "AimAtDetection", key: "results_list"},
                ...["AddToList", "DeleteOfList", "DeleteAllOfList", "ShiftList", "InsertIntoList", "ReplaceItemOfList", "ReverseList"]
                    .map(type => ({tag: type, type: type, key: "name"}))
            ];
        }
        function test_create_and_select(data) {
            const ui = setup(data.type, data.key);
            ui.name.text = "  position  ";
            ui.dialog.accept();
            compare(scene.calls.length, 1);
            compare(scene.calls[0], {command: "create_list", args: {name: "position"}});
            compare(scene.edits.length, 1);
            compare(scene.edits[0][data.key], "position");
            compare(scene.edits[0].id, ui.block.instruction.id);
        }
        function test_reporter_creation_data() {
            return [{tag: "palette", located: false}, {tag: "canvas", located: true}];
        }
        function test_reporter_creation(data) {
            scene.calls = []; scene.edits = []; scene.failure = "";
            const page = createTemporaryObject(editorComponent, scene);
            wait(50);
            const location = {kind: "Floating", floating_id: "reporter", path: []};
            const reporter = createTemporaryObject(reporterComponent, scene, {location: data.located ? location : null});
            wait(20);
            const dropdown = all(reporter).find(o => o.choices && o.choices.some(c => c.action));
            verify(!!dropdown);
            dropdown.activated(dropdown.choices.length - 1);
            if (data.located) { reporter.destroy(); wait(20); }
            const dialog = findChild(page, "listDialog");
            findChild(page, "listName").text = "new list";
            dialog.accept();
            compare(scene.calls[0], {command: "create_list", args: {name: "new list"}});
            if (data.located) {
                compare(scene.calls[1], {command: "edit_value_field", args: {location: {kind: "Floating", floating_id: "reporter", path: [0]}, text: "new list"}});
            } else {
                compare(scene.edits[0], {path: [0], leaf: {kind: "Text", value: "new list"}});
            }
        }
        function test_first_list_can_be_created() {
            scene.calls = []; scene.edits = []; scene.failure = "";
            const page = createTemporaryObject(editorComponent, scene);
            const state = JSON.parse(JSON.stringify(page.appState));
            state.current_macro.lists = [];
            page.appState = state;
            wait(50);
            const block = createTemporaryObject(blockComponent, scene, {instruction: page.defaultInstruction("AskChoice")});
            wait(20);
            const dropdown = all(block).find(o => o.choices && o.choices.some(c => c.action && c.label === "Make a List…"));
            verify(!!dropdown);
            dropdown.activated(dropdown.choices.length - 1);
            findChild(page, "listName").text = "first list";
            findChild(page, "listDialog").accept();
            compare(scene.edits[0].list, "first list");
        }
        function test_cancel_leaves_selection() {
            const ui = setup("AskChoice", "list");
            ui.dialog.reject();
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
            compare(ui.block.instruction.list, "existing");
        }
        function test_failed_creation_keeps_dialog_and_selection() {
            const ui = setup("AskChoice", "list");
            scene.failure = "List already exists";
            ui.name.text = "existing";
            ui.dialog.accept();
            tryCompare(ui.dialog, "visible", true);
            compare(ui.dialog.errorText, scene.failure);
            compare(scene.edits.length, 0);
            scene.failure = "";
            ui.name.text = "new name";
            ui.dialog.accept();
            compare(scene.edits.length, 1);
            compare(scene.edits[0].list, "new name");
        }
        function test_empty_name_keeps_dialog() {
            const ui = setup("AddToList", "name");
            ui.name.text = "  ";
            ui.dialog.accept();
            tryCompare(ui.dialog, "visible", true);
            verify(ui.dialog.errorText.length > 0);
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
        }
        function test_existing_choice_edits_directly() {
            const ui = setup("AddToList", "name");
            ui.dialog.reject();
            ui.dropdown.activated(0);
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 1);
            compare(scene.edits[0].name, "existing");
        }
    }
}
