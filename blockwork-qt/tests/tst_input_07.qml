import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1000; height: 800
    property var calls: []
    property var current: null
    Item {
        id: page
        property var macro: ({id: "macro"})
        function daemonCall(command, args, done) { compareCommand(command); done(JSON.parse(args.sequence), ""); }
        function compareCommand(command) { if (command !== "validate_touch_paths") throw new Error(command); }
        function instructionForRequest(request) { return scene.current; }
        function invoke(command, args) { scene.calls = scene.calls.concat([{command: command, args: args}]); }
    }
    TouchPathsDialog { id: paths; page: page }
    TestCase {
        name: "Input07"
        when: windowShown
        function instruction() { return {id: "touch", type: "TouchSequence", normalized: true, monitor: 0, sequence: {kind: "Text", value: JSON.stringify({version: 1, frames: [{at_ms:0, contacts:[{id:0,x:200000,y:300000}]},{at_ms:100,contacts:[{id:0,x:700000,y:300000}]},{at_ms:101,contacts:[]}]})}}; }
        function init() { scene.calls = []; scene.current = instruction(); page.macro = {id: "macro"}; }
        function cleanup() { paths.close(); wait(250); }
        function openPaths() { paths.openFor({strandId: "s", path:[{index:0}], instruction:scene.current}); tryCompare(paths, "opened", true); }
        function test_paths_edit_multiple_fingers_with_one_apply() {
            openPaths();
            paths.addContact(0);
            paths.point(0, 1, "x", "60");
            paths.addFrame();
            compare(paths.frames.length, 4);
            compare(paths.frames[0].contacts[1].x, 600000);
            compare(paths.frames[3].contacts.length, 0);
            paths.apply();
            tryCompare(scene, "calls", [{command:"edit_instruction", args:{strandId:"s",path:[{index:0}],instruction:Object.assign({}, scene.current, {sequence:{kind:"Text",value:JSON.stringify({version:1,frames:paths.frames})}})}}]);
        }
        function test_cancel_and_stale_macro_preserve_the_original_paths() {
            openPaths(); paths.point(0, 0, "x", "55"); paths.reject(); compare(scene.calls.length, 0);
            openPaths(); paths.point(0, 0, "x", "55"); page.macro = {id:"other"}; paths.apply();
            tryVerify(() => paths.errorText.length > 0);
            compare(scene.calls.length, 0);
        }
        function test_non_numeric_drafts_keep_apply_disabled() {
            openPaths(); paths.check("example", "", 1, true); verify(!paths.valid);
            paths.addContact(1); verify(!paths.valid);
            paths.check("example", "5", 1, true); verify(paths.valid);
        }
    }
}
