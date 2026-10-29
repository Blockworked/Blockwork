import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 800; height: 800
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
            width: 360; height: 640
            showToolbar: false
            invoke: function(command, args) {}
            appBridge: bridge
            appState: ({current_macro: {id:"macro",name:"Phone",speed_multiplier:1,settings:{},signals:[],variables:[],lists:[],comments:[],floating_values:[],block_defs:[],strands:[]},
                capabilities:{platform:"android",services:{}},recording_phase:{phase:"Idle"},key_capture:null,can_undo:false,can_redo:false})
        }
    }
    TestCase {
        name: "PhoneEditor"
        when: windowShown
        function all(o) { let out=[o];for(const c of o.children||[])out=out.concat(all(c));return out; }
        function init() { Motion.durationScale=0;Theme.touchMode=true; }
        function cleanup() { Theme.touchMode=false; }
        function test_compact_sheet_search_and_back() {
            const p=createTemporaryObject(editorComponent,scene);verify(!!p);verify(p.compact);
            const canvas=all(p).find(o=>o.isBlockCanvas);compare(canvas.width,360);
            p.openBlocks();tryCompare(p,"blocksOpen",true);
            const palette=findChild(p,"editorPalette");verify(!!palette);verify(palette.visible);compare(palette.width,360);
            palette.searchText="does not exist";verify(palette.noResults);
            verify(p.dismissSheet());tryCompare(p,"blocksOpen",false);tryCompare(findChild(p,"addBlocksSheet"),"visible",false);verify(!p.dismissSheet());
        }
        function test_sheets_exclude_each_other_and_resize() {
            const p=createTemporaryObject(editorComponent,scene);
            p.openBlocks();tryCompare(p,"blocksOpen",true);
            p.openActivity();tryCompare(p,"activityOpen",true);tryCompare(p,"blocksOpen",false);
            verify(p.dismissSheet());tryCompare(p,"activityOpen",false);
            p.openBlocks();tryCompare(p,"blocksOpen",true);p.width=720;wait(30);verify(!p.compact);tryCompare(p,"blocksOpen",false);
        }
        function test_palette_tap_submits_once_and_closes() {
            const p=createTemporaryObject(editorComponent,scene);
            p.openBlocks();tryCompare(p,"blocksOpen",true);
            const palette=findChild(p,"editorPalette");
            palette.searchText="wait";wait(40);
            const entry=all(palette).find(o=>o.spec && o.spec.kind==="instruction" && o.spec.type==="Wait" && o.instruction!==undefined);
            verify(!!entry);
            let edits=[];p.paletteDrags.submit=(edit,reply)=>{edits.push(edit);reply(true);};
            const t=touchEvent(entry);t.press(0,entry,25,25).commit();t.release(0,entry,25,25).commit();wait(40);
            compare(edits.length,1);compare(edits[0].kind,"add_strand");tryCompare(p,"blocksOpen",false);
        }
        function test_320_landscape_bounded_sheets() {
            const p=createTemporaryObject(editorComponent,scene,{width:320,height:300});
            p.openBlocks();tryCompare(p,"blocksOpen",true);
            const sheet=findChild(p,"addBlocksSheet");verify(!!sheet);compare(sheet.width,320);verify(sheet.height<=300);
            p.openActivity();tryCompare(p,"activityOpen",true);
            const log=findChild(p,"activitySheet");verify(log.height<=300);
        }
    }
}
