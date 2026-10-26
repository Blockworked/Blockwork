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
                globals: [{name:"existing", value:{kind:"Number",value:0}}],
                current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:["existing"], lists:[], comments:[],
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
            onValueEdited: (location, text) => scene.edits.push({location: location, text: text})
            onInstructionEdited: (strandId, path, instruction) => scene.edits.push(instruction)
        }
    }
    Component {
        id: reporterComponent
        ValueChip {
            valueData: ({kind: "Op", op: "GlobalVariable", args: [{kind: "Text", value: "existing"}]})
            onLeafEdited: (path, leaf) => scene.edits.push({path: path, leaf: leaf})
            onEditRequested: (location, text) => scene.edits.push({location: location, text: text})
        }
    }
    TestCase {
        name: "GlobalCreation"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function setup(type, palette, empty) {
            scene.calls = []; scene.edits = []; scene.failure = "";
            const page = createTemporaryObject(editorComponent, scene);
            if (empty) {
                const state = JSON.parse(JSON.stringify(page.appState));
                state.globals = []; page.appState = state;
            }
            wait(50);
            const slot = {kind: "Floating", floating_id: "global", path: []};
            const target = type === "GlobalVariable"
                ? createTemporaryObject(reporterComponent, scene, {location: palette ? null : slot, paletteMode: palette})
                : createTemporaryObject(blockComponent, scene, {instruction: page.defaultInstruction(type), paletteMode: palette});
            wait(20);
            const dropdown = all(target).find(o => o.choices && o.choices.some(c => c.label === "Make a Global Variable…"));
            verify(!!dropdown, type + " has global choices");
            dropdown.activated(dropdown.choices.length - 1);
            const picker = findChild(page, "globalPicker");
            verify(picker.visible);
            verify(picker.creating);
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
            return {page: page, target: target, dropdown: dropdown, picker: picker, name: findChild(picker, "namePickerName")};
        }
        function test_create_and_select_data() {
            const rows = [];
            for (const type of ["SetGlobal", "ChangeGlobal", "GlobalVariable"])
                for (const palette of [true, false]) rows.push({tag: type + (palette ? "_palette" : "_canvas"), type: type, palette: palette});
            return rows;
        }
        function test_create_and_select(data) {
            const ui = setup(data.type, data.palette);
            ui.name.text = "  shared score  ";
            if (data.type === "GlobalVariable" && !data.palette) { ui.target.destroy(); wait(20); }
            ui.picker.create();
            compare(scene.calls[0], {command: "create_global", args: {name: "shared score"}});
            tryCompare(ui.picker, "visible", false);
            if (!data.palette) {
                compare(scene.calls[1].command, "put_value");
                compare(scene.calls[1].args.value, {kind: "Text", value: "shared score"});
                if (data.type === "GlobalVariable") compare(scene.calls[1].args.location, {kind: "Floating", floating_id: "global", path: [0]});
                else compare(scene.calls[1].args.location.field_id, "GlobalName");
            } else if (data.type === "GlobalVariable") {
                compare(scene.edits[0], {path: [0], leaf: {kind: "Text", value: "shared score"}});
            } else {
                compare(scene.edits[0].name, {kind: "Text", value: "shared score"});
            }
        }
        function test_first_global_can_be_created() {
            const ui = setup("SetGlobal", true, true);
            compare(ui.dropdown.choices.length, 1);
            ui.name.text = "first global"; ui.picker.create();
            compare(scene.calls[0].command, "create_global");
            compare(scene.edits[0].name.value, "first global");
        }
        function test_name_slot_still_accepts_reporters() {
            const ui = setup("SetGlobal", true);
            ui.picker.reject();
            const instruction = JSON.parse(JSON.stringify(ui.target.instruction));
            instruction.name = {kind: "Op", op: "Join", args: [{kind:"Text",value:"shared"},{kind:"Text",value:" score"}]};
            ui.target.instruction = instruction;
            wait(20);
            verify(all(ui.target).some(o => o.valueData && o.valueData.op === "Join"));
            verify(!all(ui.target).some(o => o.choices && o.choices.some(c => c.label === "Make a Global Variable…")));
            compare(scene.calls.length, 0);
        }
        function test_cancel_preserves_selection() {
            const ui = setup("SetGlobal", true);
            ui.picker.reject();
            compare(scene.calls.length, 0);
            compare(scene.edits.length, 0);
            compare(ui.target.instruction.name.value, "existing");
        }
        function test_empty_and_duplicate_names_do_not_submit() {
            const ui = setup("SetGlobal", true);
            ui.name.text = "  "; ui.picker.create();
            compare(scene.calls.length, 0);
            ui.name.text = "existing"; ui.picker.create();
            compare(scene.calls.length, 0);
            verify(ui.picker.exists);
            verify(ui.picker.visible);
        }
        function test_server_error_allows_retry() {
            const ui = setup("ChangeGlobal", true);
            scene.failure = "Could not save globals";
            ui.name.text = "new global"; ui.picker.create();
            compare(ui.picker.error, scene.failure);
            verify(ui.picker.visible);
            compare(scene.edits.length, 0);
            scene.failure = "";
            ui.picker.create();
            compare(scene.edits[0].name.value, "new global");
        }
        function test_existing_choice_selects_directly() {
            const ui = setup("SetGlobal", true);
            ui.picker.reject();
            ui.dropdown.activated(0);
            compare(scene.calls.length, 0);
            compare(scene.edits[0].name.value, "existing");
        }
        function test_globals_refresh_dropdowns() {
            const ui = setup("SetGlobal", true);
            ui.picker.reject();
            const state = JSON.parse(JSON.stringify(ui.page.appState));
            state.globals.push({name: "second", value: {kind: "Number", value: 0}});
            ui.page.appState = state;
            wait(20);
            verify(ui.dropdown.choices.some(c => c.value === "second"));
        }
    }
}
