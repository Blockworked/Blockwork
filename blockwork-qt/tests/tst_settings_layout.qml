import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1700; height: 1000
    property var calls: []
    Component {
        id: settingsComponent
        SettingsPage {
            width: scene.width; height: scene.height
            invoke: (command, args) => scene.calls.push({ command: command, args: args })
            appState: ({
                close_to_tray: true, start_at_startup: false, ipc_active_port: null, ipc_auto_start: false,
                macro_names: ["run OBS", "mouse spin"],
                named_hotkey_defaults: [
                    { action: {type: "RunMacro"}, combo_display: "Ctrl+Alt+M" },
                    { action: {type: "EmergencyStop"}, combo_display: "Ctrl+Shift+Alt+Escape" }
                ],
                hotkey_bindings: [
                    { action: {type: "RunMacro"}, combo_display: "Ctrl+M", binding_index: 0 },
                    { action: {type: "EmergencyStop"}, combo_display: "Ctrl+Shift+Alt+Escape", binding_index: 1 },
                    { action: {type: "RunSpecificMacro"}, macro_name: "run OBS", combo_display: "Ctrl+Alt+O", binding_index: 2 }
                ]
            })
        }
    }
    TestCase {
        name: "SettingsLayout"
        when: windowShown
        function descendants(item) {
            let result = [item];
            for (const child of item.children || []) result = result.concat(descendants(child));
            return result;
        }
        function init() { scene.calls = []; Theme.mode = "dark"; }
        function test_panels_reflow() {
            const page = createTemporaryObject(settingsComponent, scene, { tab: 1 });
            const global = findChild(page, "globalShortcuts");
            const macros = findChild(page, "macroShortcuts");
            tryVerify(() => macros.x > global.x + global.width);
            compare(macros.y, global.y);
            verify(macros.x + macros.width > 1200);
            page.width = 900;
            tryVerify(() => macros.y >= global.y + global.height);
            compare(macros.x, global.x);
            for (const row of descendants(global).filter(o => o.title !== undefined && o.stacked !== undefined)) {
                verify(row.height > 0);
                verify(row.width <= global.width);
            }
        }
        function test_shortcut_actions() {
            const page = createTemporaryObject(settingsComponent, scene, { tab: 1 });
            wait(50);
            const buttons = descendants(page).filter(o => o.clicked !== undefined);
            buttons.find(o => o.text === "Ctrl+M").clicked();
            compare(scene.calls.pop(), { command: "start_combo_capture", args: { action: {type: "RunMacro"} } });
            buttons.find(o => o.iconName === "rotate-ccw" && o.visible).clicked();
            compare(scene.calls.pop().command, "reset_hotkey_to_default");
            buttons.find(o => o.text === "Ctrl+Alt+O").clicked();
            compare(scene.calls.pop(), { command: "start_binding_combo_capture", args: { index: 2 } });
        }
        function test_all_tabs_resize() {
            const page = createTemporaryObject(settingsComponent, scene);
            for (let tab = 0; tab < 4; ++tab) {
                page.tab = tab;
                for (const width of [600, 900, 1700]) {
                    page.width = width;
                    wait(30);
                    let rows = [];
                    tryVerify(() => {
                        rows = descendants(page).filter(o => o.stacked !== undefined && o.visible);
                        return rows.length > 0 && rows.every(row => row.width > 0 && row.height >= 28);
                    }, 5000, "Settings rows must finish layout for tab " + tab + " at width " + width);
                    for (const row of rows) {
                        verify(row.width > 0);
                        verify(row.height >= 28);
                        for (const child of row.children) {
                            verify(child.x >= -1, "Row content extends past the left edge");
                            verify(child.x + child.width <= row.width + 1, "Row content extends past the right edge");
                        }
                    }
                }
            }
        }
    }
}
