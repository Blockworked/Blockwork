import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 700; height: 700
    property var moves: []
    property var edits: []
    property var canvas
    property var palette
    property var controller
    Component {
        id: fixtureComponent
        Item {
            width: 700; height: 700
            property alias canvas: canvasView
            property alias palette: paletteEntry
            property alias controller: paletteController
            BlockCanvas {
                id: canvasView
                width: 700; height: 600
                strands: [{id: "s", x: -180, y: -150, instructions: [{id: "w", type: "Wait", duration: {kind: "Number", value: 1}}]}]
                onStrandMoved: (id,x,y) => scene.moves = scene.moves.concat([{id:id,x:x,y:y}])
            }
            PaletteBlock {
                id: paletteEntry
                x: 40; y: 620
                instruction: ({id:"p",type:"Wait",duration:{kind:"Number",value:1}})
                onActivated: paletteController.addAtView({kind:"instruction",instruction:instruction})
            }
            PaletteDragController {
                id: paletteController
                canvas: canvasView
                submit: (edit, reply) => { scene.edits = scene.edits.concat([edit]); reply(true); }
            }
        }
    }
    TestCase {
        name: "TouchCanvas"
        when: windowShown
        function all(o) { let out = [o]; for (const c of o.children || []) out=out.concat(all(c)); return out; }
        function block() { return all(canvas).find(o=>o.instruction && o.instruction.id==="w" && o.paletteMode===false); }
        function flick() { return all(canvas).find(o=>o.contentX!==undefined && o.flickableDirection!==undefined); }
        function init() { Motion.durationScale=0; Theme.touchMode=true; const fixture=createTemporaryObject(fixtureComponent,scene); canvas=fixture.canvas;palette=fixture.palette;controller=fixture.controller;scene.moves=[];scene.edits=[];canvas.insertionPoint=null;canvas.cancelDrag();canvas.setZoom(1);canvas.centerOnOrigin();wait(50); }
        function cleanup() { Theme.touchMode=false; for(const o of all(canvas)) if(o.contextMenu) o.contextMenu.close(); wait(60); }
        function test_empty_pan() {
            const f=flick(), x=f.contentX, y=f.contentY;
            const t=touchEvent(canvas);
            t.press(0,canvas,600,400).commit();wait(25);
            t.move(0,canvas,580,380).commit(); wait(25);
            t.move(0,canvas,540,340).commit(); wait(25);
            t.move(0,canvas,500,300).commit(); wait(25);
            t.release(0,canvas,500,300).commit();wait(40);
            verify(f.contentX > x+30, "pan x="+f.contentX+" from "+x);verify(f.contentY > y+30, "pan y="+f.contentY+" from "+y);compare(scene.moves.length,0);
        }
        function test_block_drag() {
            const b=block();verify(!!b);
            const p=b.mapToItem(canvas,25,25), t=touchEvent(canvas);
            t.press(0,canvas,p.x,p.y).commit();wait(20);
            t.move(0,canvas,p.x+25,p.y+25).commit();wait(20);
            t.move(0,canvas,p.x+85,p.y+75).commit();wait(20);
            tryCompare(canvas,"dragging",true);
            t.release(0,canvas,p.x+85,p.y+75).commit();wait(30);
            compare(scene.moves.length,1);verify(scene.moves[0].x > -130);
        }
        function test_long_press_menu() {
            const b=block(),p=b.mapToItem(canvas,25,25),t=touchEvent(canvas);
            t.press(0,canvas,p.x,p.y).commit();wait(1000);
            verify(!!b.contextMenu);verify(b.contextMenu.visible);
            t.release(0,canvas,p.x,p.y).commit();wait(20);
            compare(scene.moves.length,0);
        }
        function test_two_fingers_cancel_block_and_zoom() {
            const b=block(),p=b.mapToItem(canvas,25,25),t=touchEvent(canvas);
            t.press(0,canvas,p.x,p.y).commit();wait(20);
            t.move(0,canvas,p.x+30,p.y).commit();wait(20);
            tryCompare(canvas,"dragging",true);
            t.stationary(0).press(1,canvas,p.x+130,p.y+100).commit();wait(20);
            verify(!canvas.dragging,"second contact cancels before pinch movement");
            t.move(0,canvas,p.x+10,p.y-10).move(1,canvas,p.x+150,p.y+120).commit();wait(30);
            t.move(0,canvas,p.x,p.y-30).move(1,canvas,p.x+180,p.y+150).commit();wait(30);
            t.move(0,canvas,p.x-20,p.y-50).move(1,canvas,p.x+200,p.y+170).commit();wait(30);
            verify(canvas.touchNavigationActive,"pinch active");verify(!canvas.dragging,"drag canceled");verify(canvas.zoom>1.1,"zoom="+canvas.zoom);
            t.release(0,canvas,p.x-20,p.y-50).release(1,canvas,p.x+200,p.y+170).commit();wait(20);
            compare(scene.moves.length,0);
        }
        function test_two_finger_pan_keeps_scale() {
            const t=touchEvent(canvas),f=flick(),x=f.contentX,y=f.contentY;
            t.press(0,canvas,400,300).press(1,canvas,500,400).commit();wait(20);
            t.move(0,canvas,420,320).move(1,canvas,520,420).commit();wait(20);
            const beforeX=f.contentX,beforeY=f.contentY;
            t.move(0,canvas,445,345).move(1,canvas,545,445).commit();wait(50);
            t.move(0,canvas,470,370).move(1,canvas,570,470).commit();wait(50);
            t.move(0,canvas,500,400).move(1,canvas,600,500).commit();wait(50);
            fuzzyCompare(canvas.zoom,1,0.01);verify(f.contentX < beforeX-35,"x "+beforeX+" -> "+f.contentX);verify(f.contentY < beforeY-35,"y "+beforeY+" -> "+f.contentY);
            t.release(0,canvas,500,400).release(1,canvas,600,500).commit();
        }
        function test_palette_tap_inserts_after_selection() {
            const b=block(),p=b.mapToItem(canvas,25,25),t=touchEvent(canvas);
            t.press(0,canvas,p.x,p.y).commit();t.release(0,canvas,p.x,p.y).commit();wait(20);
            verify(!!canvas.insertionPoint);
            const tp=touchEvent(palette);tp.press(0,palette,25,25).commit();tp.release(0,palette,25,25).commit();wait(20);
            compare(scene.edits.length,1);compare(scene.edits[0].kind,"insert_instruction");compare(scene.edits[0].strandId,"s");compare(scene.edits[0].path,[{index:1}]);
        }
        function test_second_finger_cancels_armed_drag_and_survivor_does_not_drop() {
            const b=block(),p=b.mapToItem(canvas,25,25),t=touchEvent(canvas);
            t.press(0,canvas,p.x,p.y).commit();wait(20);
            t.stationary(0).press(1,canvas,p.x+130,p.y+100).commit();wait(20);
            t.move(0,canvas,p.x+20,p.y+20).move(1,canvas,p.x+150,p.y+120).commit();wait(20);
            t.stationary(0).release(1,canvas,p.x+150,p.y+120).commit();wait(20);
            t.move(0,canvas,p.x+90,p.y+90).commit();wait(20);
            t.release(0,canvas,p.x+90,p.y+90).commit();wait(20);
            compare(scene.moves.length,0);verify(!canvas.dragging);
        }
        function test_keyboard_resize_keeps_focused_field_visible() {
            canvas.strands=[{id:"s",x:-180,y:150,instructions:[{id:"w",type:"Wait",duration:{kind:"Number",value:1}}]}];wait(30);
            const field=all(block()).find(o=>o.selectByMouse!==undefined && o.text==="1");verify(!!field);
            field.forceActiveFocus();canvas.height=300;wait(50);
            const p=field.mapToItem(canvas,0,0);
            verify(p.y>=15);verify(p.y+field.height*canvas.zoom<=285);
        }
        function test_nested_block_selects_its_own_insertion_gap() {
            canvas.strands=[{id:"s",x:-180,y:-150,instructions:[{id:"if",type:"If",condition:{kind:"Bool"},body:[{id:"w",type:"Wait",duration:{kind:"Number",value:1}}]}]}];wait(40);
            const b=block(),p=b.mapToItem(canvas,25,25),t=touchEvent(canvas);
            t.press(0,canvas,p.x,p.y).commit();t.release(0,canvas,p.x,p.y).commit();wait(20);
            compare(canvas.insertionPoint.path,[{index:0,slot:0},{index:1}]);
        }
        function test_selection_survives_state_push_but_not_document_edit() {
            canvas.selectInsertion("s",[{index:0}]);
            canvas.strands=JSON.parse(JSON.stringify(canvas.strands));wait(20);
            verify(!!canvas.insertionPoint);
            const changed=JSON.parse(JSON.stringify(canvas.strands));changed[0].instructions[0].duration.value=2;
            canvas.strands=changed;wait(20);compare(canvas.insertionPoint,null);
        }
        function test_unselected_add_uses_visible_free_space() {
            controller.addAtView({kind:"instruction",instruction:palette.instruction});
            compare(scene.edits.length,1);compare(scene.edits[0].kind,"add_strand");
            const s=canvas.mapToItem(null,0,0),e=scene.edits[0],p=canvas.workspacePoint(s.x+350,s.y+300);
            verify(Math.abs(e.x-p.x)<400);verify(Math.abs(e.y-p.y)<300);
        }
    }
}
