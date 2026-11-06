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
    MockBridge { id: bridge; calls: scene.calls; failure: scene.failure }
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: ({
                current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:["existing"], lists:[], comments:[],
                    floating_values:[{id:"param",x:40,y:250,value:{kind:"Param",name:"gain"},origin_block_id:"custom"}],
                    block_defs:[{id:"custom",identifier:"Custom",color:"#51b84b",shape:"Normal",pieces:[
                        {kind:"Label",text:"custom"}, {kind:"Input",name:"gain",value_type:"Number"},
                        {kind:"Input",name:"enabled",value_type:"Bool"}]}],
                    strands:[{id:"definition",x:40,y:40,instructions:[{id:"header",type:"BlockHeader",block_id:"custom"},{id:"wait",type:"Wait",duration:{kind:"Number",value:100}}]}]},
                recording_phase:{phase:"Idle"}, key_capture:null, can_undo:false, can_redo:false,
                record_mouse_movement:false, record_mouse_relative:false, absolute_mouse_position_available:true,
                browser:{unlocked:true}
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
    TestCase {
        name: "VariableCreation"
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
            const block = createTemporaryObject(blockComponent, scene, {instruction: instruction});
            verify(!!block);
            wait(20);
            const dropdown = all(block).find(o => o.choices && o.parent.modelData.key === key && o.choices.some(c => c.action));
            verify(!!dropdown, type + " has a variable dropdown");
            dropdown.activated(dropdown.choices.length - 1);
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
            const dialog = findChild(page, "variableDialog");
            verify(dialog.visible);
            return {page: page, block: block, dropdown: dropdown, dialog: dialog, name: findChild(page, "variableName")};
        }
        function test_pending_default_is_in_dropdown_without_creation_data() {
            return [
                {tag: "set", type: "SetVariable", key: "name"},
                {tag: "change", type: "ChangeVariable", key: "name"},
                {tag: "http", type: "HttpRequest", key: "variable"},
                {tag: "image", type: "FindImage", key: "x_variable"}
            ];
        }
        function test_pending_default_is_in_dropdown_without_creation(data) {
            scene.calls = []; scene.edits = []; scene.failure = "";
            const page = createTemporaryObject(editorComponent, scene);
            const state = JSON.parse(JSON.stringify(page.appState));
            state.current_macro.variables = [];
            page.appState = state;
            const block = createTemporaryObject(blockComponent, scene, {
                instruction: page.defaultInstruction(data.type), paletteMode: true
            });
            wait(20);
            const dropdown = all(block).find(o => o.choices && o.parent.modelData.key === data.key);
            verify(!!dropdown);
            compare(dropdown.choices.filter(choice => !choice.action), [{value: "my variable", label: "my variable"}]);
            compare(dropdown.currentIndex, 0);
            compare(dropdown.displayText, "my variable");
            dropdown.activated(0);
            compare(scene.calls.length, 0);
            compare(page.appState.current_macro.variables, []);
        }
        function test_create_and_select_data() {
            return [
                {tag: "image_x", type: "FindImage", key: "x_variable"},
                {tag: "image_y", type: "FindImage", key: "y_variable"},
                {tag: "detection_count", type: "DetectObjectsInWindow", key: "count_variable"},
                {tag: "aim_status", type: "AimAtDetection", key: "status_variable"},
                {tag: "set", type: "SetVariable", key: "name"},
                {tag: "change", type: "ChangeVariable", key: "name"},
                {tag: "browser", type: "BrowserScript", key: "variable"},
                {tag: "prompt", type: "AskText", key: "name"}
            ];
        }
        function test_create_and_select(data) {
            const ui = setup(data.type, data.key);
            ui.name.text = "  position  ";
            ui.dialog.accept();
            compare(scene.calls.length, 1);
            compare(scene.calls[0], {command: "create_variable", args: {name: "position"}});
            compare(scene.edits.length, 1);
            compare(scene.edits[0][data.key], "position");
            compare(scene.edits[0].id, ui.block.instruction.id);
        }
        function test_cancel_leaves_selection() {
            const ui = setup("FindImage", "x_variable");
            ui.dialog.reject();
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
            compare(ui.block.instruction.x_variable, "existing");
        }
        function test_failed_creation_keeps_dialog_and_selection() {
            const ui = setup("FindImage", "x_variable");
            scene.failure = "Variable already exists";
            ui.name.text = "existing";
            ui.dialog.accept();
            tryCompare(ui.dialog, "visible", true);
            compare(ui.dialog.errorText, scene.failure);
            compare(scene.edits.length, 0);
            scene.failure = "";
            ui.name.text = "new name";
            ui.dialog.accept();
            compare(scene.edits.length, 1);
            compare(scene.edits[0].x_variable, "new name");
        }
        function test_empty_name_keeps_dialog() {
            const ui = setup("SetVariable", "name");
            ui.name.text = "  ";
            ui.dialog.accept();
            tryCompare(ui.dialog, "visible", true);
            verify(ui.dialog.errorText.length > 0);
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
        }
        function test_existing_choice_edits_directly() {
            const ui = setup("SetVariable", "name");
            ui.dialog.reject();
            ui.dropdown.activated(ui.dropdown.choices.findIndex(choice => choice.value === "existing"));
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 1);
            compare(scene.edits[0].name, "existing");
        }
    }
}
