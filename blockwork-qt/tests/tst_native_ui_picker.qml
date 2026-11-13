import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1000; height: 800
    property var calls: []
    property var invoked: []
    property var pickAnswer: null
    property string pickError: ""
    property var failures: []
    readonly property var hit: ({app: "Calc", window: "Calculator", name: "Plus", role: "push button", enabled: true,
        x: 100, y: 120, width: 40, height: 30, window_only: false})
    Item {
        id: page
        property var macro: ({id: "macro"})
        property bool recordingBusy: false
        property var appState: ({ui_automation_enabled: true})
        function daemonCall(command, args, done) {
            scene.calls = scene.calls.concat([{command: command, args: args}]);
            if (command === "native_ui_element_at") done(scene.hit, "");
            else if (command === "native_ui_pick") done(scene.pickAnswer, scene.pickError);
            else if (command === "native_ui_applications") done(["Calc", "Editor"], "");
            else if (command === "native_ui_inspect") done([
                {app: "Calc", window: "Calculator", name: "Plus", role: "push button", id: "", enabled: true, text: "app=Calc; name=Plus; role=push button", matches: 1},
                {app: "Calc", window: "Calculator", name: "", role: "push button", id: "", enabled: true, text: "app=Calc; role=push button", matches: 9}], "");
            else done(null, "unexpected " + command);
        }
        function invoke(command, args) { scene.invoked = scene.invoked.concat([{command: command, args: args}]); }
        function writeNativeUi(request, text, macroId) { return list.write(request, text, macroId); }
    }
    BlockCatalog { id: catalog; page: page }
    ElementPickerOverlay {
        id: overlay
        call: (command, args, done) => page.daemonCall(command, args, done)
    }
    NativeUiPickerDialog { id: list; page: page }
    ScreenPicker {
        id: picker; page: page
        onFailed: message => scene.failures = scene.failures.concat([message])
        onElementList: (request, hint) => list.openFor(request, hint)
    }
    SignalSpy { id: pickedSpy; target: overlay; signalName: "picked" }
    SignalSpy { id: canceledSpy; target: overlay; signalName: "canceled" }

    TestCase {
        name: "NativeUiPicker"
        when: windowShown
        function init() {
            scene.calls = []; scene.invoked = []; scene.failures = []; scene.pickError = "";
            scene.pickAnswer = null; pickedSpy.clear(); canceledSpy.clear(); page.macro = {id: "macro"};
        }
        function cleanup() { overlay.end(); list.close(); wait(150); }

        function test_labels_read_both_selector_spellings() {
            compare(catalog.nativeUiLabel("app=Calc; name=Plus; role=push button"), "Plus · Calc");
            compare(catalog.nativeUiLabel('{"scope":"App","app":"Calc","name":"Plus"}'), "Plus · Calc");
            compare(catalog.nativeUiLabel("scope=Focused;name=Save;role=push button"), "Save · focused window");
            compare(catalog.nativeUiLabel("window=Prefs; role=check box"), "check box · Prefs");
            compare(catalog.nativeUiLabel(""), "Choose element");
        }
        function test_hover_outlines_and_names_the_element() {
            overlay.begin();
            tryVerify(() => overlay.overlayAt(0) && overlay.overlayAt(0).visible);
            const win = overlay.overlayAt(0);
            compare(win.title, overlay.overlayTitle);
            overlay.lookAt({x: 110, y: 130});
            tryVerify(() => overlay.hover && overlay.hover.name === "Plus");
            compare(scene.calls[0].command, "native_ui_element_at");
            compare(scene.calls[0].args.ignore, "Blockwork element picker");
            verify(win.boxHere);
            verify(win.tagItem.visible);
            // Moves while a lookup is out are coalesced to the latest point.
            overlay.asking = true;
            overlay.lookAt({x: 1, y: 1}); overlay.lookAt({x: 2, y: 2});
            compare(overlay.pending.x, 2);
            overlay.asking = false;
            compare(overlay.describe(scene.hit), "Plus");
            compare(overlay.detail(scene.hit), "push button · Calc · Calculator");
            compare(overlay.describe({window_only: true, window: "Game", app: "game"}), "Game");
        }
        function test_click_picks_and_errors_stay_on_screen() {
            overlay.begin();
            scene.pickError = "Native UI automation is disabled in Settings";
            overlay.pickAt({x: 110, y: 130});
            tryCompare(overlay, "message", "Native UI automation is disabled in Settings");
            verify(overlay.active);
            scene.pickError = "";
            overlay.pickAt({x: 5, y: 5});
            tryVerify(() => overlay.message.indexOf("no app window") >= 0);
            scene.pickAnswer = {hit: scene.hit, node: {app: "Calc", name: "Plus", text: "app=Calc; name=Plus; role=push button", matches: 1}};
            overlay.pickAt({x: 110, y: 130});
            tryCompare(pickedSpy, "count", 1);
            verify(!overlay.active);
            overlay.begin();
            overlay.cancel();
            compare(canceledSpy.count, 1);
        }
        function test_a_unique_pick_is_written_into_the_block() {
            scene.pickAnswer = {hit: scene.hit, node: {app: "Calc", name: "Plus", text: "app=Calc; name=Plus; role=push button", matches: 1}};
            picker.startElementPick({location: {strand_id: "s"}});
            tryVerify(() => picker.elementOverlay.active, 2000);
            picker.elementOverlay.pickAt({x: 110, y: 130});
            tryCompare(scene.invoked, "length", 1);
            compare(scene.invoked[0].command, "put_value");
            compare(scene.invoked[0].args.value.value, "app=Calc; name=Plus; role=push button");
            compare(scene.failures.length, 0);
        }
        function test_an_ambiguous_pick_opens_the_list_with_a_notice() {
            scene.pickAnswer = {hit: scene.hit, node: {app: "Calc", name: "", role: "push button", text: "app=Calc; role=push button", matches: 9}};
            picker.startElementPick({location: {strand_id: "s"}});
            tryVerify(() => picker.elementOverlay.active, 2000);
            picker.elementOverlay.pickAt({x: 110, y: 130});
            tryCompare(list, "opened", true);
            verify(list.notice.indexOf("9 elements") >= 0);
            tryVerify(() => list.nodes.length === 2);
            compare(list.wanted, null);
            compare(scene.calls.filter(c => c.command === "native_ui_inspect")[0].args.selector, JSON.stringify({scope: "App", app: "Calc", name: ""}));
            compare(scene.invoked.length, 0);
            list.choose(list.nodes[0]);
            compare(scene.invoked.length, 1);
            compare(scene.invoked[0].args.value.value, "app=Calc; name=Plus; role=push button");
        }
        function test_automation_off_offers_to_turn_it_on() {
            page.appState = {ui_automation_enabled: false};
            picker.startElementPick({location: {}});
            tryVerify(() => picker.elementOverlay.active && picker.elementOverlay.automationOff, 2000);
            picker.elementOverlay.lookAt({x: 1, y: 1});
            compare(scene.calls.length, 0);
            picker.elementOverlay.turnOnAutomation();
            compare(scene.invoked[0].command, "set_ui_automation_enabled");
            picker.elementOverlay.cancel();
            page.appState = {ui_automation_enabled: true};
        }
        function test_the_list_refuses_a_stale_macro() {
            list.openFor({location: {}}, null);
            tryCompare(list, "opened", true);
            page.macro = {id: "other"};
            list.choose({text: "app=Calc; name=Plus"});
            verify(list.errorText.length > 0);
            compare(scene.invoked.length, 0);
        }
    }
}
