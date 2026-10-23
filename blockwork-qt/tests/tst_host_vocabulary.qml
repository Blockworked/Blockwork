import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    function invoke(command, args) { calls.push({command: command, args: args}); }
    QtObject {
        id: bridge
        property int replySerial: 0
        property int previewSerial: 0
        property string activeBlocksJson: "[]"
        property string installedAppsError: ""
        function callCommand(key, command, args) {}
    }
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: ({
                current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:[], lists:[], comments:[],
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
    TestCase {
        name: "HostVocabulary"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function test_blockwork_registers_its_own_features() {
            const page = createTemporaryObject(editorComponent, scene);
            verify(!!page);
            wait(50);
            verify(BlockRegistry.recordingTargets);
            compare(BlockRegistry.operator("BatteryPercentage").result, "number");
            compare(BlockRegistry.operator("ClipboardHasFiles").result, "bool");
            compare(BlockRegistry.iconFor("MoveMouse"), "move");
            compare(BlockRegistry.iconFor("WhenRan"), "play");
            compare(BlockRegistry.iconFor("If"), "git-branch");
        }
        function test_palette_reporters_start_from_the_core_defaults() {
            const page = createTemporaryObject(editorComponent, scene);
            wait(50);
            const palette = all(page).find(o => o.extraOperators !== undefined && typeof o.operatorValue === "function");
            verify(!!palette);
            for (const spec of page.extraOperators) {
                const value = palette.operatorValue(spec);
                compare(value.op, spec[0], spec[0]);
                compare(value.args.length, spec[2], spec[0]);
                if (!spec[3]) compare(value, BlockRegistry.freshValue(spec[0]), spec[0]);
                verify(!!BlockRegistry.descriptor(spec[0]), spec[0]);
            }
            compare(palette.operatorValue(["ReadFile", "text", 1]).args, [{ kind: "Text", value: "~/notes.txt" }]);
            compare(palette.operatorValue(["LetterOf", "text", 2]).args, [{ kind: "Number", value: 1 }, { kind: "Text", value: "" }]);
            const detection = palette.operatorValue(["Detection", "text", 2]);
            compare(detection.args[1].op, "ListItem");
            compare(detection.args[1].args, [{ kind: "Number", value: 1 }, { kind: "Text", value: "detections" }]);
        }
    }
}
