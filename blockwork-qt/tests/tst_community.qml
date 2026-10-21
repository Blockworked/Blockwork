import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    width: 1000; height: 760
    id: scene
    property string invoked: ""
    property var args: ({})
    function invoke(command, args) { invoked = command; scene.args = args; }
    property var definitions: [
        {id:"local",identifier:"Local",shape:"Normal",pieces:[{kind:"Label",text:"local block"}],read_only:false},
        {id:"community",identifier:"Alice_Community",shape:"Normal",pieces:[{kind:"Label",text:"community block"}],read_only:true,pack_name:"Example"},
        {id:"reporter",identifier:"Alice_Reporter",shape:"ReturnsValue",pieces:[{kind:"Label",text:"community reporter"}],read_only:true,pack_name:"Example"}
    ]
    property var pack: ({key:"example#pack",id:"pack",name:"Example pack",version:"1.0",description:"An example pack",author:"Alice",blocks:["community block"],installed:"1.0",available:false,update:false,official:false})
    QtObject {
        id: bridge
        property int replySerial: 0
        property string replyKey: ""
        property string replyError: ""
        property string replyJson: "null"
        function callCommand(key, command, args) { scene.invoked = command; scene.args = JSON.parse(args); }
    }
    PalettePanel {
        id: palette; width: 360; height: 700
        blockDefinitions: scene.definitions
        blockLibraryEnabled: true
    }
    BlockLibrary {
        id: library
        appState: ({current_macro:{id:"macro",block_defs:scene.definitions},block_library:{loaded:true,loading:false,packs:[scene.pack],sources:[],fetching:"",error:""}})
        invoke: scene.invoke
        appBridge: bridge
        macroFiles: null
    }
    TestCase {
        name: "ManagedCommunityPacks"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            if (object.contentItem && (object.children || []).indexOf(object.contentItem) < 0) found = found.concat(all(object.contentItem));
            return found;
        }
        function test_community_palette() {
            verify(palette.categories.some(c => c.id === "community"));
            compare(palette.categoryCount("custom"), 1);
            compare(palette.categoryCount("community"), 2);
            palette.category = "community";
            wait(50);
            for (const id of ["community", "reporter"]) {
                let loaders = all(palette).filter(o => o.modelData && o.modelData.id === id && o.item);
                let community = loaders.find(o => o.visible);
                verify(!!community);
                compare(community.item.extraMenu.length, 1);
                compare(community.item.extraMenu[0].id, "details");
            }
            verify(!all(palette).some(o => o.modelData && o.modelData.id === "local" && o.item && o.visible));
        }
        function test_library_updates_keep_remove_on_the_card() {
            scene.pack = Object.assign({}, scene.pack, { available: true, update: true });
            library.browse();
            wait(100);
            const objects = all(scene).concat(all(Overlay.overlay));
            verify(objects.some(o => o.text === "Update" && o.visible));
            verify(objects.some(o => o.text === "Remove" && o.visible));
        }
        function test_library_one_list_and_hide_installed() {
            library.browse();
            wait(100);
            const objects = all(scene).concat(all(Overlay.overlay));
            const remove = objects.find(o => o.text === "Remove" && o.visible);
            verify(!!remove);
            verify(!objects.some(o => o.text === "Installed in this macro"));
            const hide = objects.find(o => o.text === "Hide installed" && o.visible);
            verify(!!hide);
            mouseClick(hide, hide.width / 2, hide.height / 2);
            wait(50);
            verify(!all(scene).concat(all(Overlay.overlay)).some(o => o.text === "Remove" && o.visible));
            mouseClick(hide, hide.width / 2, hide.height / 2);
            wait(50);
            const restored = all(scene).concat(all(Overlay.overlay)).find(o => o.text === "Remove" && o.visible);
            mouseClick(restored, restored.width / 2, restored.height / 2);
            wait(50);
            const confirm = all(scene).concat(all(Overlay.overlay)).find(o => o.text === "Remove pack" && o.visible);
            verify(!!confirm);
            mouseClick(confirm, confirm.width / 2, confirm.height / 2);
            compare(scene.invoked, "remove_block_pack");
            compare(scene.args.packId, "pack");
        }
    }
}
