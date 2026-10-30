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
        property string replyKey: ""
        property string replyJson: "null"
        property string replyError: ""
        property string lastError: ""
        property int previewSerial: 0
        property string activeBlocksJson: "[]"
        property string installedAppsError: ""
        // Edits sent for a reply land in the same log, and succeed.
        function callCommand(key, command, args) {
            scene.calls.push({command: command, args: JSON.parse(args)});
            replyKey = key; replyJson = "null"; replyError = ""; replySerial++;
        }
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
                        {kind:"Input",name:"enabled",value_type:"Bool"},
                        {kind:"Input",name:"optional",value_type:"Bool",component:true}]}],
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
            mouseMove(scene, scene.width - 2, scene.height - 2, 0, Qt.NoButton);
            BlockRegistry.rows = ({});
            BlockRegistry.resetOperators();
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
            verify(!!page.paletteDrags.drag, "Header parameter starts the editor's palette drag");
        }
        function test_block_moves_preserve_palette_metadata() {
            const page = editor();
            const canvas = all(page).find(o => o.isBlockCanvas);
            const palette = all(page).find(o => typeof o.customInstruction === "function");
            const definitions = canvas.blockDefinitions;
            const variables = canvas.variables;
            const lists = canvas.lists;
            const moved = JSON.parse(JSON.stringify(page.appState));
            moved.current_macro.strands[0].instructions[1].duration.value = 200;
            page.appState = moved;
            wait(30);
            verify(canvas.blockDefinitions === definitions);
            verify(canvas.variables === variables);
            verify(canvas.lists === lists);
            verify(palette.blockDefinitions === definitions);
            verify(palette.variables === variables);
            verify(palette.lists === lists);

            const edited = JSON.parse(JSON.stringify(moved));
            edited.current_macro.variables = ["score"];
            edited.current_macro.lists = [{name:"players",items:[],editor_visible:false,editor_x:36,editor_y:36}];
            edited.current_macro.block_defs[0].pieces[1].name = "strength";
            page.appState = edited;
            wait(30);
            compare(canvas.variables, ["score"]);
            compare(palette.variables, ["score"]);
            compare(canvas.lists[0].name, "players");
            compare(palette.lists[0].name, "players");
            verify(!!parameter(canvas.findBlockItem(canvas, "header"), "strength"));
        }
        function test_new_custom_calls_keep_pack_defaults() {
            const page = editor();
            const definition = {id:"example",shape:"Normal",pieces:[
                {kind:"Label",text:"example"},
                {kind:"Input",name:"window",default_value:{kind:"Text",value:""}},
                {kind:"Input",name:"monitor",default_value:{kind:"Number",value:1}},
                {kind:"Branch",name:"body"}
            ]};
            page.paletteDrags.addAt(page.customEntry(definition), 100, 150);
            const call = scene.calls.find(c => c.command === "add_strand");
            verify(!!call);
            compare(call.args.instruction.type, "BranchCallBlock");
            compare(call.args.instruction.args[0].kind, "Text");
            compare(call.args.instruction.args[0].value, "");
            compare(call.args.instruction.args[1].value, 1);
            compare(call.args.instruction.branches.length, 1);
        }
        function test_drag_to_canvas_data() { return [{tag:"number",name:"gain",boolean:false},{tag:"boolean",name:"enabled",boolean:true},{tag:"component",name:"optional",boolean:true}]; }
        function test_drag_to_canvas(data) {
            const page = editor();
            const chip = parameter(page, data.name);
            verify(!!chip);
            const value = all(chip).find(o => o.isValueChip);
            compare(value.refColor, Qt.color("#51b84b"));
            compare(value.booleanShape, data.boolean);
            startDrag(page, chip);
            compare(page.paletteDrags.drag.spec.originBlockId, "custom");
            const ghost = all(page).find(o => o.isValueChip && o.valueData && o.valueData.name === data.name && !o.paletteMode && !o.location);
            verify(!!ghost);
            compare(ghost.booleanShape, data.boolean);
            compare(ghost.refColor, Qt.color("#51b84b"));
            verify(ghost.dragPreview);
            const outline = all(ghost).find(o => o.edge !== undefined);
            verify(!!outline);
            compare(outline.edge, Qt.color("#51b84b"));
            const cursor = all(page).find(o => o.objectName === "dragCursor");
            verify(cursor.visible);
            compare(cursor.cursorShape, Qt.ClosedHandCursor);
            mouseMove(scene, 1000, 700, 20);
            mouseRelease(scene, 1000, 700);
            compare(page.paletteDrags.drag, null);
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
            const chip = all(page).find(o => o.isValueChip && o.originBlockId === "custom" && o.location && o.location.kind === "Floating" && o.valueData.kind === "Param");
            verify(!!chip);
            compare(chip.refColor, Qt.color("#51b84b"));
        }
        function test_parameter_shapes_after_drop_data() {
            const out = [];
            for (const name of ["gain", "enabled", "optional"])
                for (const context of ["floating", "field", "nested"])
                    out.push({tag: name + "_" + context, name: name, context: context});
            return out;
        }
        function test_parameter_shapes_after_drop(data) {
            const page = editor();
            const state = JSON.parse(JSON.stringify(page.appState));
            const value = {kind: "Param", name: data.name};
            if (data.context === "floating") state.current_macro.floating_values[0].value = value;
            else state.current_macro.strands[0].instructions[1].duration = data.context === "nested"
                ? {kind: "Op", op: "Add", args: [value, {kind: "Number", value: 1}]}
                : value;
            page.appState = state;
            wait(30);
            const chip = all(page).find(o => o.isValueChip && o.location && o.valueData && o.valueData.kind === "Param"
                && o.valueData.name === data.name && (data.context === "floating" ? o.location.kind === "Floating" : o.location.strand_id === "definition"));
            verify(!!chip);
            compare(chip.booleanShape, data.name !== "gain");
            dragExisting(chip);
            const ghost = all(page).find(o => o.isValueChip && o.dragPreview && o.valueData && o.valueData.kind === "Param" && !o.location);
            verify(!!ghost);
            compare(ghost.booleanShape, data.name !== "gain");
            mouseRelease(scene, 1000, 700);
        }
        function canvas(page) {
            return all(page).find(o => typeof o.parameterOrigin === "function");
        }
        function dragExisting(chip) {
            waitForRendering(chip);
            const at = chip.mapToItem(scene, chip.width / 2, chip.height / 2);
            mouseMove(scene, at.x, at.y, 20, Qt.NoButton);
            mousePress(scene, at.x, at.y);
            mouseMove(scene, at.x + 80, at.y + 80, 20, Qt.LeftButton);
        }
        function test_second_drag_keeps_color() {
            const page = editor();
            const floating = all(page).find(o => o.isValueChip && o.location && o.location.floating_id === "param");
            verify(!!floating);
            dragExisting(floating);
            verify(canvas(page).dragIsValue);
            const ghost = all(page).find(o => o.isValueChip && o.dragPreview && o.valueData && o.valueData.kind === "Param" && !o.location);
            verify(!!ghost);
            compare(ghost.refColor, Qt.color("#51b84b"));
            compare(canvas(page).dragValueOrigin, "custom");
            mouseRelease(scene, 1000, 700);
        }
        function test_field_and_nested_parameter_color_data() {
            return [{tag:"field",nested:false}, {tag:"nested expression",nested:true}];
        }
        function test_field_and_nested_parameter_color(data) {
            const page = editor();
            const state = JSON.parse(JSON.stringify(page.appState));
            state.current_macro.strands[0].instructions[1].duration = data.nested
                ? {kind:"Op",op:"Add",args:[{kind:"Param",name:"gain"},{kind:"Var",name:"ordinary"}]}
                : {kind:"Param",name:"gain"};
            page.appState = state;
            wait(50);
            const param = all(page).find(o => o.isValueChip && o.location && o.location.strand_id === "definition" && o.valueData && o.valueData.kind === "Param");
            verify(!!param);
            compare(param.originBlockId, "custom");
            compare(param.refColor, Qt.color("#51b84b"));
            const variable = all(page).find(o => o.isValueChip && o.location && o.valueData && o.valueData.kind === "Var" && o.valueData.name === "ordinary");
            if (data.nested) { verify(!!variable); compare(variable.refColor, Theme.accent); }
            dragExisting(param);
            compare(canvas(page).dragValue.kind, "Param");
            const ghost = all(page).find(o => o.isValueChip && o.dragPreview && o.valueData && o.valueData.kind === "Param" && !o.location);
            verify(!!ghost);
            compare(ghost.refColor, Qt.color("#51b84b"));
            mouseMove(scene, 1000, 700, 20);
            mouseRelease(scene, 1000, 700);
            // Out of a field it's one move, which keeps the parameter's block.
            const moved = scene.calls.filter(c => c.command === "move_value");
            compare(moved.length, 1);
            compare(moved[0].args.to, null);
            compare(moved[0].args.originBlockId, "custom");
        }
        function test_cancel() {
            const page = editor();
            const chip = parameter(page, "gain");
            startDrag(page, chip);
            page.paletteDrags.cancel();
            mouseRelease(scene, 1000, 700);
            compare(page.paletteDrags.drag, null);
            compare(scene.calls.filter(c => c.command === "create_floating_value" || c.command === "put_value").length, 0);
        }
    }
}
