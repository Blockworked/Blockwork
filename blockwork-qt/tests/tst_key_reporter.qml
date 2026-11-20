import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// The "key held?" reporter records its key like the Key block does.
Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    function invoke(command, args) { calls.push({command: command, args: args}); }
    MockBridge { id: bridge; calls: scene.calls }
    function macro() {
        return {id: "m", name: "m", speed_multiplier: 1, settings: {always_listen: false}, signals: [], variables: [], lists: [], comments: [],
            floating_values: [], block_defs: [], strands: [{id: "main", x: 40, y: 40, instructions: [
                {id: "wait", type: "WaitUntil", condition: {kind: "Op", op: "KeyHeld", args: [{kind: "Text", value: "Control+a"}]}}]}]};
    }
    function state(capture) {
        return {current_macro: macro(), recording_phase: {phase: "Idle", countdown: null}, key_capture: capture || null, can_undo: false, can_redo: false,
            record_mouse_movement: false, record_mouse_relative: false, absolute_mouse_position_available: true, browser: {unlocked: true}};
    }
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: scene.state()
        }
    }
    TestCase {
        name: "KeyReporter"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function test_key_slot_records_a_key() {
            BlockRegistry.rows = ({});
            BlockRegistry.resetOperators();
            const page = createTemporaryObject(editorComponent, scene);
            tryVerify(() => !all(page).some(o => o.pendingEntries > 0), 10000, "palette finished building");
            let chip = null;
            tryVerify(() => {
                chip = all(page).find(o => o.keyMode === true && o.location && o.location.strand_id === "main" && o.width > 0);
                return !!chip;
            }, 5000, "the reporter's key slot draws as keycaps");
            verify(!chip.keyCapturing);
            scene.calls = [];
            mouseClick(chip, chip.width / 2, chip.height / 2);
            const call = scene.calls.find(c => c.command === "start_value_key_capture");
            verify(!!call, "a click starts a key capture");
            compare(BlockRegistry.locationKey(call.args.location), BlockRegistry.locationKey(chip.location));
            page.appState = scene.state({kind: "Value", location: call.args.location});
            tryVerify(() => chip.keyCapturing, 2000, "the slot shows it is waiting for a key");
        }
    }
}
