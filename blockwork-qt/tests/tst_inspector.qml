import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 400; height: 300
    property var calls: []
    property var answer: null
    readonly property var full: ({x: 640, y: 360, color: "#336699", notes: [],
        window: {title: "Notes", app: "editor", pid: 42, x: 600, y: 300, width: 800, height: 600, mouse_x: 40, mouse_y: 60},
        element: {app: "editor", window: "Notes", name: "Save", role: "button", enabled: true, x: 630, y: 350, width: 40, height: 20, window_only: false}})
    InspectorWindow {
        id: inspector
        // The test pointer may rest on the window.
        pauseOnHover: false
        call: (command, args, done) => {
            scene.calls = scene.calls.concat([{command: command, args: args}]);
            done(scene.answer, "");
        }
    }

    TestCase {
        name: "Inspector"
        when: windowShown
        function init() {
            scene.answer = scene.full;
            scene.calls = [];
            inspector.reading = null;
        }
        function value(label) {
            const row = inspector.rows.find(r => r[0] === label);
            return row ? row[1] : null;
        }
        function test_reads_what_is_under_the_mouse() {
            inspector.show();
            tryVerify(() => inspector.reading !== null);
            compare(scene.calls[0].command, "inspect_mouse");
            compare(scene.calls[0].args.ignore, inspector.title);
            verify(scene.calls[0].args.color);
            compare(value("Mouse on screen"), "640, 360");
            compare(value("Mouse in window"), "40, 60");
            compare(value("Process ID"), "42");
            compare(value("Color"), "#336699");
            compare(value("Element"), "Save");
            // Later readings skip the screenshot but keep the last color.
            scene.answer = Object.assign({}, scene.answer, {color: null, x: 641});
            tryVerify(() => value("Mouse on screen") === "641, 360");
            compare(value("Color"), "#336699");
            inspector.hide();
        }
        function test_missing_parts_say_why() {
            scene.answer = {x: 1, y: 2, color: null, window: null, element: null, notes: [["window", "listing windows isn't supported"]]};
            inspector.show();
            tryVerify(() => value("Window") === "listing windows isn't supported");
            compare(value("App"), null);
            inspector.hide();
        }
    }
}
