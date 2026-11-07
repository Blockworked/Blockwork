import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 800; height: 600
    Component {
        id: panelComponent
        ActivityPanel { width: 700; height: 220; expanded: true; macroId: "macro" }
    }
    TestCase {
        name: "ActivityScroll"
        when: windowShown
        function test_wheel_controls_following_data() {
            return [{ tag: "instant", scale: 0 }, { tag: "animated", scale: 1 }];
        }
        function cleanup() { Motion.durationScale = 1; }
        function test_wheel_controls_following(data) {
            Motion.durationScale = data.scale;
            const panel = createTemporaryObject(panelComponent, scene);
            const entries = [];
            for (let i = 0; i < 100; ++i)
                entries.push({ seq: i, macro_id: "macro", level: "Info", message: "Entry " + i,
                               source: "Macro", event: "Log", time_ms: Date.now() });
            panel.entries = entries;
            const list = findChild(panel, "activityList");
            tryCompare(list, "count", 100);
            tryVerify(() => list.atYEnd);
            const end = list.contentY;
            mouseWheel(list, 100, 60, 0, 120);
            tryVerify(() => list.contentY < end - 20);
            tryCompare(list, "atEnd", false);
            wait(300);
            panel.entries = entries.concat([{ seq: 100, macro_id: "macro", level: "Info",
                message: "New entry", source: "Macro", event: "Log", time_ms: Date.now() }]);
            tryCompare(list, "count", 101);
            wait(50);
            verify(!list.atYEnd);
            compare(list.atEnd, false);
            mouseWheel(list, 100, 60, 0, -12000);
            tryVerify(() => list.atYEnd);
            tryCompare(list, "atEnd", true);
        }
    }
}
