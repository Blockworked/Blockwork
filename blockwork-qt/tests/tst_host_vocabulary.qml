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
        function test_android_filters_palette_and_preserves_imported_desktop_blocks() {
            const page = createTemporaryObject(editorComponent, scene);
            const state = JSON.parse(JSON.stringify(page.appState));
            state.capabilities = {platform: "android", services: {touch: {supported: true, ready: false}}};
            state.current_macro.strands.push({id: "imported", x: 400, y: 40, instructions: [
                {id: "desktop-key", type: "Key", key: {kind: "Text", value: "a"}, direction: "Click"}
            ]});
            page.appState = state;
            wait(50);
            verify(page.instructionTypes.indexOf("Touch") >= 0);
            compare(page.instructionTypes.indexOf("Key"), -1);
            compare(page.instructionTypes.indexOf("WhenApp"), -1);
            verify(!BlockRegistry.recordingTargets);
            compare(page.macro.strands[1].instructions[0].type, "Key");
            verify(!!page.blockMarks["desktop-key"]);
            compare(page.defaultInstruction("Touch").action, "Tap");
            verify(page.instructionTypes.indexOf("Element") >= 0);
            verify(page.instructionTypes.indexOf("WhenTime") >= 0);
            verify(page.instructionTypes.indexOf("WhenCron") >= 0);
            verify(page.instructionTypes.indexOf("WhenAt") >= 0);
            compare(page.defaultInstruction("Element").selector.package, "");
            verify(page.extraOperators.some(spec => spec[0] === "AndroidElementExists"));
            verify(page.extraOperators.some(spec => spec[0] === "AndroidElementText"));
        }
        function test_android_picker_uses_confirmed_values_without_recapturing_editor() {
            const page = createTemporaryObject(editorComponent, scene);
            const state = JSON.parse(JSON.stringify(page.appState));
            state.capabilities = {platform: "android", services: {screenshots: {supported: true, ready: true}}};
            page.appState = state;
            wait(50);
            const overlay = all(page).find(o => o.objectName === "screenOverlay");
            verify(!!overlay);
            scene.calls = [];
            const instruction = page.defaultInstruction("Touch");
            page.pickRequest = {kind: "touch", strandId: "strand", path: [{index: 1, slot: null}], instruction: instruction, end: true};
            overlay.captureResult = {x: 1079, y: 2399, width: 0, height: 0, color: "#ff0000", image: ""};
            overlay.picked(1079, 2399, 0, 0);
            compare(scene.calls.length, 1);
            compare(scene.calls[0].command, "edit_instruction");
            compare(scene.calls[0].args.instruction.end_x, {kind: "Number", value: 1079});
            compare(scene.calls[0].args.instruction.end_y, {kind: "Number", value: 2399});
            compare(scene.calls[0].args.instruction.x, instruction.x);
            compare(page.pickRequest, null);
            scene.calls = [];
            page.pickRequest = {kind: "image", strandId: "strand", path: [], instruction: page.defaultInstruction("FindImage")};
            overlay.captureResult = {x: 12, y: 34, width: 56, height: 78, color: "#000000", image: "data:image/png;base64,confirmed"};
            overlay.picked(12, 34, 56, 78);
            compare(scene.calls.length, 1);
            compare(scene.calls[0].args.instruction.image, "data:image/png;base64,confirmed");
            verify(!Object.keys(page.callbacks).length);
        }
        function test_android_element_pick_preserves_action_and_other_fields() {
            const page = createTemporaryObject(editorComponent, scene);
            const state = JSON.parse(JSON.stringify(page.appState));
            state.capabilities = {platform: "android", services: {}};
            page.appState = state;
            wait(50);
            const overlay = all(page).find(o => o.objectName === "screenOverlay");
            const instruction = page.defaultInstruction("Element");
            instruction.action = "SetText";
            instruction.text = {kind: "Text", value: "hello"};
            page.pickRequest = {kind: "element", strandId: "strand", path: [], instruction: instruction};
            scene.calls = [];
            const selector = {package: "fixture.app", resource_id: "fixture.app:id/name", text: "", description: "", class: "android.widget.EditText"};
            overlay.elementPicked(selector);
            compare(scene.calls.length, 1);
            compare(scene.calls[0].args.instruction.selector, selector);
            compare(scene.calls[0].args.instruction.action, "SetText");
            compare(scene.calls[0].args.instruction.text, instruction.text);
            verify(page.instructionTypes.indexOf("FindImage") >= 0);
            verify(page.extraOperators.some(spec => spec[0] === "PixelColor"));
            verify(!page.extraOperators.some(spec => spec[0] === "ScreenText"));
        }
        function test_android_app_blocks_and_package_picker_keep_process_blocks_separate() {
            const page = createTemporaryObject(editorComponent, scene);
            const state = JSON.parse(JSON.stringify(page.appState));
            state.capabilities = {platform: "android", services: {foreground_events: {supported: true, ready: true}}};
            page.appState = state;
            wait(50);
            verify(page.instructionTypes.indexOf("WhenForegroundApp") >= 0);
            verify(page.instructionTypes.indexOf("LaunchAndroidApp") >= 0);
            compare(page.instructionTypes.indexOf("WhenApp"), -1);
            compare(page.instructionTypes.indexOf("OpenApp"), -1);
            scene.calls = [];
            const launch = page.defaultInstruction("LaunchAndroidApp");
            page.appPickerRequest = {strandId: "strand", path: [], instruction: launch};
            page.applyPickedApp({command: "fixture.app", name: "Fixture", icon: null});
            compare(scene.calls[0].args.instruction.package, {kind: "Text", value: "fixture.app"});
            compare(scene.calls[0].args.instruction.command, undefined);
            scene.calls = [];
            const event = page.defaultInstruction("WhenForegroundApp");
            event.entered = false;
            page.appPickerRequest = {strandId: "strand", path: [], instruction: event};
            page.applyPickedApp({command: "fixture.app", name: "Fixture", icon: null});
            compare(scene.calls[0].args.instruction.package, "fixture.app");
            compare(scene.calls[0].args.instruction.entered, false);
            const desktop = JSON.parse(JSON.stringify(page.appState));
            desktop.capabilities = {platform: "linux", services: {}};
            desktop.current_macro.strands.push({id: "android-import", x: 400, y: 40, instructions: [launch, event]});
            page.appState = desktop;
            wait(50);
            compare(page.instructionTypes.indexOf("LaunchAndroidApp"), -1);
            compare(page.instructionTypes.indexOf("WhenForegroundApp"), -1);
            verify(!!page.blockMarks[launch.id]);
            verify(!!page.blockMarks[event.id]);
            compare(page.macro.strands[1].instructions[1].entered, false);
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
