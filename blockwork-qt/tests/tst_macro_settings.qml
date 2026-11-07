import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 900; height: 700
    property var calls: []
    Component {
        id: settingsComponent
        MacroSettingsDialog {
            macro: ({name: "Aim to nearest Minecraft player", speed_multiplier: 1,
                settings: {max_run_secs: 12, max_loop_iterations: 50, always_listen: true, power: "Any"}})
            permissions: null
            guardedAbilities: [{kind: "RunCommand", label: "Run commands"}, {kind: "Network", label: "Go online"}]
            invoke: (command, args) => { scene.calls.push({command: command, args: args}); }
        }
    }
    TestCase {
        name: "MacroSettings"
        when: windowShown
        function openDialog() {
            const dialog = createTemporaryObject(settingsComponent, scene);
            verify(dialog);
            dialog.open();
            tryCompare(dialog, "opened", true);
            scene.calls = [];
            return dialog;
        }
        function descendants(item) {
            let result = [item];
            for (const child of item.children || []) result = result.concat(descendants(child));
            return result;
        }
        function control(dialog, name) {
            const found = descendants(dialog.contentItem).find(o => o.Accessible && o.Accessible.name === name);
            verify(!!found, name);
            return found;
        }
        function test_tabs_fit_data() {
            return [{tag: "desktop", width: 900, height: 700},
                    {tag: "compact", width: 500, height: 540},
                    {tag: "narrow", width: 380, height: 480}];
        }
        function test_tabs_fit(data) {
            scene.width = data.width; scene.height = data.height;
            scene.Window.window.width = data.width; scene.Window.window.height = data.height;
            const dialog = openDialog();
            const tabs = findChild(dialog.contentItem, "macroSettingsTabs");
            const flick = findChild(dialog.contentItem, "macroSettingsFlick");
            dialog.permissions = [];
            dialog.macro = Object.assign({}, dialog.macro, {settings: {active_from: "09:00", active_until: "17:00"}});
            verify(dialog.width <= scene.width);
            verify(dialog.height <= scene.height);
            for (let i = 0; i < 3; ++i) {
                tabs.currentIndex = i;
                wait(30);
                compare(flick.contentY, 0);
                verify(flick.height > 150);
                for (const item of descendants(flick.contentItem)) {
                    if (!item.visible || !item.width || !item.height || !item.Accessible || !item.Accessible.name) continue;
                    const pos = item.mapToItem(flick.contentItem, 0, 0);
                    verify(pos.x >= -1 && pos.x + item.width <= flick.width + 1,
                           item.Accessible.name + " overflows: " + pos.x + " + " + item.width + " > " + flick.width);
                }
            }
            compare(scene.calls.length, 0);
            dialog.close();
        }
        function test_time_range_uses_the_shared_picker_data() {
            return [{tag: "12-hour", format: "12h", label: "1:45 " + Schedules.pmText},
                    {tag: "24-hour", format: "24h", label: "13:45"}];
        }
        function test_time_range_uses_the_shared_picker(data) {
            scene.width = 900; scene.height = 700;
            scene.Window.window.width = 900; scene.Window.window.height = 700;
            const previousFormat = Schedules.clockFormat;
            Schedules.clockFormat = data.format;
            const dialog = openDialog();
            findChild(dialog.contentItem, "macroSettingsTabs").currentIndex = 1;
            wait(30);
            const start = findChild(dialog.contentItem, "conditionStart");
            const end = findChild(dialog.contentItem, "conditionEnd");
            const mode = findChild(dialog.contentItem, "conditionTimeMode");
            compare(mode.currentIndex, 0);
            verify(!start.visible);
            mode.currentIndex = 1; mode.activated(1);
            compare(scene.calls[0].args.activeFrom, "09:00");
            compare(scene.calls[0].args.activeUntil, "17:00");
            dialog.macro = {name: "Test", speed_multiplier: 1,
                settings: {active_from: "09:00", active_until: "17:00", power: "Any"}};
            wait(30);
            const startPicker = findChild(start, "conditionStartPicker");
            const endPicker = findChild(end, "conditionEndPicker");
            mouseClick(start);
            tryCompare(startPicker, "opened", true);
            compare(startPicker.use12h, data.format === "12h");
            compare(startPicker.hour, 9);
            startPicker.hour = 13; startPicker.minute = 45;
            startPicker.accept();
            tryCompare(startPicker, "visible", false);
            compare(scene.calls.length, 2);
            compare(scene.calls[1].args.activeFrom, "13:45");
            compare(scene.calls[1].args.activeUntil, "17:00");
            dialog.macro = {name: "Test", speed_multiplier: 1,
                settings: {active_from: "13:45", active_until: "17:00", power: "Any"}};
            compare(start.text, data.label);
            wait(30);
            mouseClick(end);
            tryCompare(endPicker, "opened", true);
            compare(endPicker.hour, 17);
            endPicker.close();
            tryCompare(endPicker, "visible", false);
            compare(scene.calls.length, 2);
            mouseClick(end);
            tryCompare(endPicker, "opened", true);
            endPicker.hour = 18; endPicker.minute = 20;
            endPicker.accept();
            tryCompare(endPicker, "visible", false);
            compare(scene.calls[2].args.activeFrom, "13:45");
            compare(scene.calls[2].args.activeUntil, "18:20");
            mode.currentIndex = 0; mode.activated(0);
            dialog.macro = {name: "Test", speed_multiplier: 1, settings: {}};
            compare(mode.currentIndex, 0);
            verify(!start.visible);
            compare(scene.calls[3].args.activeFrom, "");
            compare(scene.calls[3].args.activeUntil, "");
            Schedules.clockFormat = previousFormat;
            dialog.close();
        }
        function test_edits_and_conditions_keep_their_commands() {
            scene.width = 900; scene.height = 700;
            scene.Window.window.width = 900; scene.Window.window.height = 700;
            const dialog = openDialog();
            const title = findChild(dialog.contentItem, "macroTitle");
            title.text = "Renamed"; title.editingFinished();
            const limit = findChild(dialog.contentItem, "runLimit");
            compare(limit.text, "12"); limit.text = "20"; limit.editingFinished();
            const loop = findChild(dialog.contentItem, "loopLimit");
            compare(loop.text, "50"); loop.text = "100"; loop.editingFinished();
            const tabs = findChild(dialog.contentItem, "macroSettingsTabs");
            tabs.currentIndex = 1;
            control(dialog, "Listen for triggers").toggle();
            control(dialog, "Catch up on missed schedules").toggle();
            control(dialog, "Focused window").text = "Minecraft";
            control(dialog, "Focused window").editingFinished();
            const monday = control(dialog, "Monday");
            monday.forceActiveFocus(); keyClick(Qt.Key_Space);
            tabs.currentIndex = 2;
            control(dialog, "Ask before risky actions").toggle();
            dialog.permissions = [];
            wait(30);
            control(dialog, "Run commands").forceActiveFocus(); keyClick(Qt.Key_Space);
            compare(scene.calls.map(c => c.command), ["set_title", "set_macro_max_run_secs", "set_macro_max_loop_iterations",
                "set_macro_always_listen", "set_macro_catch_up", "set_macro_conditions", "set_macro_conditions",
                "set_macro_restricted", "set_macro_permission"]);
            compare(scene.calls[5].args.onlyInWindow, "Minecraft");
            compare(scene.calls[6].args.activeDays, 125);
            compare(scene.calls[8].args, {kind: "RunCommand", allowed: true});
            dialog.close();
        }
    }
}
