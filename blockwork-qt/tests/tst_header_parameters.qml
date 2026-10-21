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
        name: "HeaderParameters"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function editor() {
            const page = createTemporaryObject(editorComponent, scene);
            verify(!!page);
            wait(100);
            scene.calls = [];
            return page;
        }
        function parameter(page, name) {
            return all(page).find(o => o.spec && o.spec.originBlockId === "custom" && o.valueData.name === name);
        }
        function startDrag(page, chip) {
            const start = chip.mapToItem(scene, chip.width / 2, chip.height / 2);
            mousePress(scene, start.x, start.y);
            mouseMove(scene, start.x + 30, start.y + 20, 20);
            verify(!!page.paletteDrag, "Header parameter starts the editor's palette drag");
        }
        function test_drag_to_canvas_data() { return [{tag:"number",name:"gain",boolean:false},{tag:"boolean",name:"enabled",boolean:true}]; }
        function test_drag_to_canvas(data) {
            const page = editor();
            const chip = parameter(page, data.name);
            verify(!!chip);
            const value = all(chip).find(o => o.isValueChip);
            compare(value.refColor, Qt.color("#51b84b"));
            compare(value.booleanShape, data.boolean);
            startDrag(page, chip);
            compare(page.paletteDrag.spec.originBlockId, "custom");
            const ghost = all(page).find(o => o.isValueChip && o.valueData && o.valueData.name === data.name && !o.paletteMode && !o.location);
            verify(!!ghost);
            compare(ghost.refColor, Qt.color("#51b84b"));
            mouseMove(scene, 1000, 550, 20);
            mouseRelease(scene, 1000, 550);
            compare(page.paletteDrag, null);
            const created = scene.calls.filter(c => c.command === "create_floating_value");
            compare(created.length, 1);
            compare(created[0].args.value.kind, "Param");
            compare(created[0].args.value.name, data.name);
            compare(created[0].args.originBlockId, "custom");
        }
        function test_drop_into_slot() {
            const page = editor();
            const target = all(page).find(o => o.isValueChip && o.visible && o.width > 0 && o.valueLocation && o.valueLocation.strand_id === "definition" && o.valueLocation.field_id === "WaitDuration");
            verify(!!target);
            startDrag(page, parameter(page, "gain"));
            const at = target.mapToItem(scene, target.width / 2, target.height / 2);
            mouseMove(scene, at.x, at.y, 20);
            mouseRelease(scene, at.x, at.y);
            const put = scene.calls.filter(c => c.command === "put_value");
            compare(put.length, 1);
            compare(put[0].args.location.field_id, "WaitDuration");
            compare(put[0].args.value.name, "gain");
            compare(scene.calls.filter(c => c.command === "create_floating_value").length, 0);
        }
        function test_floating_parameter_color() {
            const page = editor();
            const chip = all(page).find(o => o.isValueChip && o.originBlockId === "custom");
            verify(!!chip);
            compare(chip.refColor, Qt.color("#51b84b"));
        }
        function test_cancel() {
            const page = editor();
            const chip = parameter(page, "gain");
            startDrag(page, chip);
            page.cancelPaletteDrag();
            mouseRelease(scene, 1000, 550);
            compare(page.paletteDrag, null);
            compare(scene.calls.filter(c => c.command === "create_floating_value" || c.command === "put_value").length, 0);
        }
    }
}
