import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    function invoke(command, args) { calls.push({command: command, args: args}); }
    MockBridge {
        id: bridge
        silent: true
        property string installedAppsJson: JSON.stringify([
            { name: "Kate", command: "kate -b", icon: null, window: "org.kde.kate", process: "kate" },
            { name: "Firefox", command: "firefox", icon: null, window: "firefox", process: "firefox" }])
        property bool installedAppsLoading: false
        property int refreshes: 0
        function refreshInstalledApps() { refreshes++; }
    }
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: ({
                current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:[], lists:[], comments:[],
                    floating_values:[], block_defs:[], strands:[{id:"strand",x:40,y:40,instructions:[{id:"run",type:"WhenRan"}]}]},
                recording_phase:{phase:"Idle"}, key_capture:null, can_undo:false, can_redo:false,
                record_mouse_movement:false, record_mouse_relative:false, absolute_mouse_position_available:true
            })
        }
    }
    TestCase {
        name: "AppSlots"
        when: windowShown
        function patternPieces(type) {
            return BlockRegistry.row(type).head.filter(p => p.kind === "value" && p.key === "pattern");
        }
        function test_window_blocks_pick_an_app_while_matching_by_app() {
            createTemporaryObject(editorComponent, scene);
            for (const type of ["WindowAction", "SetWindow", "UseCoordinates", "InWindow", "MoveWindow", "DetectObjectsInWindow", "WaitForDetection", "AimStep"]) {
                const pieces = patternPieces(type);
                compare(pieces.length, 2, type);
                const byApp = { type: type, by: "App", window: true };
                const byTitle = { type: type, by: "Title", window: true };
                compare(pieces.filter(p => p.when(byApp)).map(p => !!p.pick), [true], type);
                compare(pieces.filter(p => p.when(byTitle)).map(p => !!p.pick), [false], type);
            }
            compare(patternPieces("UseCoordinates").filter(p => p.when({ by: "App", window: false })).length, 0);
        }
        function test_the_slot_shows_the_apps_name() {
            const page = createTemporaryObject(editorComponent, scene);
            const pick = patternPieces("InWindow").find(p => p.pick).pick;
            compare(pick.action, "pickWindowApp");
            compare(pick.display("org.kde.kate"), "Kate");
            compare(pick.display("kat"), "kat");
            compare(pick.display(""), "Choose app...");
            const appPick = BlockRegistry.row("WaitForApp").head.find(p => p.key === "pattern").pick;
            compare(appPick.action, "pickRunningApp");
            compare(appPick.display("kate"), "Kate");
            compare(BlockRegistry.row("WhenApp").head.find(p => p.key === "pattern").pick.action, "pickRunningApp");
        }
        function test_picking_writes_the_matching_name() {
            const page = createTemporaryObject(editorComponent, scene);
            wait(50);
            const location = { strand_id: "strand", index: [{ index: 1 }], field: "WindowPattern" };
            for (const [action, expected] of [["pickWindowApp", "org.kde.kate"], ["pickRunningApp", "kate"]]) {
                scene.calls = [];
                page.blockScreens[action]({ kind: "value", location: location, value: { kind: "Text", value: "" } });
                page.applyPickedApp(JSON.parse(bridge.installedAppsJson)[0]);
                const put = scene.calls.find(c => c.command === "put_value");
                verify(!!put, action);
                compare(put.args.location, location);
                compare(put.args.value, { kind: "Text", value: expected });
            }
            scene.calls = [];
            page.blockScreens.pickWindowApp({ kind: "value", location: location, value: null });
            page.applyPickedApp({ typed: "kat" });
            compare(scene.calls.find(c => c.command === "put_value").args.value, { kind: "Text", value: "kat" });
        }
    }
}
