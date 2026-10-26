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
    readonly property var openApp: ({ id: "open", type: "OpenApp", command: "brave", name: "Brave", icon: null,
        launch: { args: "--incognito", working_dir: "", env: [{ name: "LANG", value: "C" }], new_instance: false } })
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: ({
                current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:[], lists:[], comments:[],
                    floating_values:[], block_defs:[],
                    strands:[{id:"strand",x:40,y:40,instructions:[{id:"run",type:"WhenRan"}, scene.openApp]}]},
                recording_phase:{phase:"Idle"}, key_capture:null, can_undo:false, can_redo:false,
                record_mouse_movement:false, record_mouse_relative:false, absolute_mouse_position_available:true
            })
        }
    }
    TestCase {
        name: "LaunchOptions"
        when: windowShown
        function all(object) {
            let found = [object];
            const list = object.data || object.children || [];
            const kids = [];
            for (let k = 0; k < list.length; ++k) kids.push(list[k]);
            if (object.contentItem && kids.indexOf(object.contentItem) < 0) kids.push(object.contentItem);
            for (const child of kids) found = found.concat(all(child));
            return found;
        }
        function test_the_gear_shows_the_arguments() {
            createTemporaryObject(editorComponent, scene);
            const summary = BlockRegistry.row("OpenApp").head.find(p => p.action === "launchOptions").text;
            compare(summary(scene.openApp), "--incognito");
            compare(summary({ type: "OpenApp" }), "");
            compare(summary({ launch: { args: "", working_dir: "~/x", env: [], new_instance: false } }), "custom");
            compare(summary({ launch: { args: "", working_dir: "", env: [], new_instance: false } }), "");
            verify(summary({ launch: { args: "x".repeat(40) } }).length <= 28);
        }
        function test_the_panel_saves_the_launch_options() {
            const page = createTemporaryObject(editorComponent, scene);
            wait(50);
            scene.calls = [];
            const path = [{ index: 1 }];
            page.onPieceAction("launchOptions", "strand", path, scene.openApp);
            const dialog = all(page).find(o => o.mac !== undefined && typeof o.openFor === "function");
            verify(!!dialog);
            tryVerify(() => dialog.visible);
            const fields = all(dialog).filter(o => o.placeholderText !== undefined);
            const args = fields.find(f => f.placeholderText === "--incognito");
            compare(args.text, "--incognito");
            args.text = "--incognito --new-window";
            fields.find(f => f.placeholderText === "Leave empty to use the default").text = "~/Downloads";
            dialog.accept();
            const edit = scene.calls.find(c => c.command === "edit_instruction");
            verify(!!edit);
            compare(edit.args.strandId, "strand");
            compare(edit.args.path, path);
            compare(edit.args.instruction.command, "brave");
            compare(edit.args.instruction.launch, { args: "--incognito --new-window", working_dir: "~/Downloads",
                env: [{ name: "LANG", value: "C" }], new_instance: false });
        }
        function test_the_row_has_an_app_button_and_a_gear() {
            createTemporaryObject(editorComponent, scene);
            const row = BlockRegistry.row("OpenApp");
            verify(!!row);
            const buttons = row.head.filter(p => p.kind === "button");
            compare(buttons.map(b => b.action), ["pickApp", "launchOptions"]);
            verify(buttons[0].palette);
            compare(buttons[0].text(scene.openApp), "Brave");
            compare(buttons[1].text(scene.openApp), "--incognito");
        }
    }
}
