import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    QtObject {
        id: bridge
        property int replySerial: 0
        property int previewSerial: 0
        property string activeBlocksJson: "[]"
        property string installedAppsError: ""
    }
    function state() {
        return {current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:[], lists:[], comments:[], floating_values:[], block_defs:[], strands:[]},
            recording_phase:{phase:"Idle"}, key_capture:null, can_undo:false, can_redo:false,
            record_mouse_movement:false, record_mouse_relative:false, absolute_mouse_position_available:true};
    }
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: function(command, args) { scene.calls.push({command:command, args:args}); }
            appBridge: bridge
            appState: scene.state()
        }
    }
    TestCase {
        name: "CustomBlockView"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function create(page, pieces) {
            scene.calls = [];
            const dialog = findChild(page, "blockDialog");
            verify(!!dialog);
            dialog.createRequested(pieces, "Normal", "#51b84b", "", "", "");
            compare(scene.calls.length, 1);
            compare(scene.calls[0].command, "create_block");
            return scene.calls[0].args;
        }
        function spawn(page, args) {
            const updated = JSON.parse(JSON.stringify(page.appState));
            updated.current_macro.block_defs.push({id:"custom", pieces:args.pieces, shape:args.shape, color:args.color});
            updated.current_macro.strands.push({id:"definition",x:args.x,y:args.y,instructions:[{id:"header",type:"BlockHeader",block_id:"custom"}]});
            page.appState = updated;
            wait(100);
        }
        function test_header_spawns_in_view_without_panning_data() {
            return [{tag:"normal",zoom:1,edge:false}, {tag:"zoomed_out_left",zoom:0.5,edge:true},
                {tag:"wide_header",zoom:1,edge:false,wide:true}, {tag:"branch_header",zoom:1,edge:false,branch:true}];
        }
        function test_header_spawns_in_view_without_panning(data) {
            const page = createTemporaryObject(editorComponent, scene);
            wait(30);
            const canvas = all(page).find(o => o.isBlockCanvas);
            canvas.setZoom(data.zoom);
            if (data.edge) all(canvas).find(o => o.contentX !== undefined && o.contentWidth !== undefined).contentX = 0;
            const point = canvas.mapToItem(null, 30, 100);
            const before = canvas.workspacePoint(point.x, point.y);
            const pieces = [{kind:"Label",text:data.wide ? "A much longer custom block header with several words" : "custom"}];
            if (data.branch) pieces.push({kind:"Branch",name:"body"});
            const args = create(page, pieces);
            spawn(page, args);
            const header = canvas.findBlockItem(canvas, "header");
            verify(!!header);
            const p = header.mapToItem(canvas, 0, 0);
            verify(p.x >= 0 && p.x + header.width * canvas.zoom <= canvas.width, "header fits horizontally");
            verify(p.y >= 0 && p.y + header.height * canvas.zoom <= canvas.height, "header fits vertically");
            fuzzyCompare(p.x + header.width * canvas.zoom / 2, canvas.width / 2, 1, "centered horizontally");
            fuzzyCompare(p.y + header.height * canvas.zoom / 2, canvas.height / 2, 1, "centered vertically");
            const after = canvas.workspacePoint(point.x, point.y);
            fuzzyCompare(after.x, before.x, 0.1);
            fuzzyCompare(after.y, before.y, 0.1);
        }
        function test_avoids_existing_blocks() {
            const page = createTemporaryObject(editorComponent, scene);
            const updated = JSON.parse(JSON.stringify(page.appState));
            updated.current_macro.strands = [{id:"existing",x:-80,y:-40,instructions:[{id:"wait",type:"Wait",duration:{kind:"Number",value:100}}]}];
            page.appState = updated;
            wait(40);
            const canvas = all(page).find(o => o.isBlockCanvas);
            const args = create(page, [{kind:"Label",text:"custom"}]);
            spawn(page, args);
            const header = canvas.findBlockItem(canvas, "header");
            const existing = canvas.findBlockItem(canvas, "wait");
            const a = header.mapToItem(canvas, 0, 0), b = existing.mapToItem(canvas, 0, 0);
            verify(a.x >= b.x + existing.width || a.x + header.width <= b.x
                || a.y >= b.y + existing.height || a.y + header.height <= b.y, "no block overlap");
        }
        function test_full_view_uses_adjacent_space_without_panning() {
            const page = createTemporaryObject(editorComponent, scene);
            wait(30);
            const canvas = all(page).find(o => o.isBlockCanvas);
            const point = canvas.mapToItem(null, 0, 0);
            const before = canvas.workspacePoint(point.x, point.y);
            const updated = JSON.parse(JSON.stringify(page.appState));
            for (let x = 0; x < canvas.width; x += 220)
                for (let y = 0; y < canvas.height; y += 113)
                    updated.current_macro.comments.push({id:x+":"+y,x:before.x+x,y:before.y+y,text:"",collapsed:false});
            page.appState = updated;
            wait(40);
            const args = create(page, [{kind:"Label",text:"custom"}]);
            spawn(page, args);
            const header = canvas.findBlockItem(canvas, "header");
            const p = header.mapToItem(canvas, 0, 0);
            verify(p.x + header.width <= 0 || p.y + header.height <= 0 || p.x >= canvas.width || p.y >= canvas.height);
            verify(p.x > -header.width - 250 && p.x < canvas.width + 250
                && p.y > -header.height - 250 && p.y < canvas.height + 250, "nearby free space");
            const after = canvas.workspacePoint(point.x, point.y);
            fuzzyCompare(after.x, before.x, 0.1);
            fuzzyCompare(after.y, before.y, 0.1);
        }
    }
}
