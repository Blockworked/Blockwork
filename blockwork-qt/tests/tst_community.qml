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
        {id:"community",identifier:"Alice_Community",shape:"Normal",pieces:[{kind:"Label",text:"community block"}],read_only:true,pack_name:"Command helpers",packs:[{id:"commands",name:"Command helpers"},{id:"text",name:"Text helpers"}]},
        {id:"reporter",identifier:"Alice_Reporter",shape:"ReturnsValue",pieces:[{kind:"Label",text:"community reporter"}],read_only:true,pack_name:"Text helpers",packs:[{id:"text",name:"Text helpers"}]}
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
    Component { id: blockDialogComponent; MakeBlockDialog {} }
    TestCase {
        name: "ManagedCommunityPacks"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            if (object.contentItem && (object.children || []).indexOf(object.contentItem) < 0) found = found.concat(all(object.contentItem));
            return found;
        }
        function test_editing_preserves_pack_defaults() {
            const dialog = createTemporaryObject(blockDialogComponent, scene);
            verify(!!dialog);
            const definition = {id:"defaults",shape:"Normal",pieces:[
                {kind:"Label",id:"label",text:"example"},
                {kind:"Input",id:"text",name:"text",default_value:{kind:"Text",value:""}},
                {kind:"Input",id:"number",name:"count",value_type:"Any",default_value:{kind:"Number",value:1}}
            ]};
            dialog.pendingDef = definition;
            dialog.loadDef(definition);
            const pieces = dialog.resultPieces();
            compare(pieces[1].default_value.kind, "Text");
            compare(pieces[1].default_value.value, "");
            compare(pieces[2].default_value.value, 1);
        }
        function named(object, name) {
            return findChild(object, name);
        }
        function test_default_parameter_editor() {
            const dialog = createTemporaryObject(blockDialogComponent, scene);
            verify(!!dialog);
            dialog.openForEdit({id:"defaults", pieces:[
                {kind:"Label",id:"label",text:"example"},
                {kind:"Input",id:"count",name:"count",value_type:"Any",default_value:{kind:"Number",value:4}},
                {kind:"Input",id:"text",name:"text",value_type:"Any",default_value:{kind:"Text",value:""}},
                {kind:"Input",id:"flag",name:"enabled",value_type:"Bool",default_value:{kind:"Bool",value:true}}
            ]});
            tryCompare(dialog, "opened", true);
            compare(dialog.resultPieces().length, 4);
            const defaults = named(dialog, "defaultParametersPopup");
            const publishing = named(dialog, "publishingPopup");
            verify(!defaults.visible);
            verify(!publishing.visible);
            mouseClick(named(dialog, "defaultParametersButton"));
            tryCompare(defaults, "visible", true);
            verify(!publishing.visible);
            const repeater = named(defaults, "defaultParameterRows");
            verify(!!repeater);
            compare(repeater.count, 4);
            const rows = [1, 2, 3].map(i => repeater.itemAt(i));
            compare(rows.length, 3);
            const count = rows.find(o => o.pieceName === "count");
            const number = named(count, "defaultNumberValue");
            compare(number.text, "4");
            number.forceActiveFocus();
            number.selectAll();
            keyClick(Qt.Key_9);
            compare(dialog.resultPieces()[1].default_value.value, 9);
            const textRow = rows.find(o => o.pieceName === "text");
            const text = named(textRow, "defaultTextValue");
            text.forceActiveFocus();
            keyClick(Qt.Key_A);
            compare(dialog.resultPieces()[2].default_value.value, "a");
            mouseClick(named(rows.find(o => o.pieceName === "enabled"), "defaultBooleanValue"));
            compare(dialog.resultPieces()[3].default_value.value, false);
            dialog.setInputDefault(1, "Number", "bad");
            verify(!dialog.defaultsValid());
            dialog.setInputDefault(1, "Number", "-2.5");
            verify(dialog.defaultsValid());
            dialog.selectedPiece = 1;
            dialog.moveSelected(1);
            compare(dialog.resultPieces()[2].id, "count");
            compare(dialog.resultPieces()[2].default_value.value, -2.5);
            defaults.close();
            mouseClick(named(dialog, "publishingButton"));
            tryCompare(publishing, "visible", true);
            verify(!defaults.visible);
            const identifier = named(publishing, "publishingIdentifier");
            identifier.forceActiveFocus();
            keyClick(Qt.Key_X);
            compare(identifier.text, "x");
            let saved = null;
            dialog.editRequested.connect((id, pieces) => { saved = pieces; });
            dialog.submit();
            verify(!!saved);
            compare(saved[2].default_value.value, -2.5);
            compare(saved[1].default_value.value, "a");
            compare(saved[3].default_value.value, false);
            tryCompare(dialog, "visible", false);
            dialog.openForCreate();
            tryCompare(dialog, "opened", true);
            compare(identifier.text, "");
            dialog.addPiece("Input", "Any");
            dialog.commitPiece();
            dialog.setInputDefault(1, "Text", "");
            compare(dialog.resultPieces()[1].default_value.kind, "Text");
            compare(dialog.resultPieces()[1].default_value.value, "");
            dialog.close();
        }
        function test_pack_input_defaults() {
            const definition = {id:"defaults",shape:"Normal",pieces:[
                {kind:"Label",text:"example"},
                {kind:"Input",name:"window",default_value:{kind:"Text",value:""}},
                {kind:"Input",name:"monitor",default_value:{kind:"Number",value:1}},
                {kind:"Input",name:"enabled",value_type:"Bool",default_value:{kind:"Bool",value:true}},
                {kind:"Input",name:"legacy"},
                {kind:"Input",name:"legacy bool",value_type:"Bool"}
            ]};
            const command = palette.customInstruction(definition);
            compare(command.args[0].kind, "Text");
            compare(command.args[0].value, "");
            compare(command.args[1].value, 1);
            compare(command.args[2].op, "True");
            compare(command.args[3].value, 0);
            compare(command.args[4].kind, "Bool");
            const reporter = palette.customValue(definition);
            compare(JSON.stringify(command.args), JSON.stringify(reporter.args));
            command.args[1].value = 7;
            compare(palette.customInstruction(definition).args[1].value, 1);
            compare(definition.pieces[2].default_value.value, 1);
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
        function test_community_pack_sections_and_search() {
            palette.category = "community";
            palette.searchText = "";
            wait(50);
            compare(palette.communityPacks.length, 2);
            compare(palette.communityPacks[0].blocks.length, 1);
            compare(palette.communityPacks[1].blocks.length, 2);
            let headings = all(palette).filter(o => o.objectName === "communityPackHeading" && o.visible);
            compare(headings.map(o => o.text).join(","), "Command helpers,Text helpers");
            palette.searchText = "reporter";
            wait(50);
            headings = all(palette).filter(o => o.objectName === "communityPackHeading" && o.visible);
            compare(headings.length, 1);
            compare(headings[0].text, "Text helpers");
            palette.searchText = "Command helpers";
            wait(50);
            verify(!palette.noResults);
            headings = all(palette).filter(o => o.objectName === "communityPackHeading" && o.visible);
            compare(headings.length, 1);
            compare(headings[0].text, "Command helpers");
            verify(all(palette).some(o => o.modelData && o.modelData.id === "community" && o.item && o.visible));
            palette.searchText = "";
        }
        function test_rail_labels_stay_inside_the_rail() {
            let label = all(palette).find(o => o.objectName === "paletteCategoryLabel" && o.text === "Community");
            verify(!!label);
            verify(!label.truncated);
            verify(label.mapToItem(palette, 0, 0).x >= 0);
            verify(label.mapToItem(palette, label.width, 0).x <= palette.railWidth);
            const longName = "An extraordinarily long translated category";
            palette.appCategories = [{id:"long", name:longName, icon:"info"}];
            wait(50);
            label = all(palette).find(o => o.objectName === "paletteCategoryLabel" && o.text === longName);
            verify(!!label);
            verify(label.truncated);
            compare(label.elide, Text.ElideRight);
            verify(label.mapToItem(palette, 0, 0).x >= 0);
            verify(label.mapToItem(palette, label.width, 0).x <= palette.railWidth);
            palette.appCategories = [];
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
