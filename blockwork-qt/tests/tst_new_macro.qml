import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 600; height: 700
    Component { id: component; NewMacroDialog {} }
    Component {
        id: libraryComponent
        LibraryPanel { appState: ({}); invoke: () => { throw new Error("New must not invoke a backend command"); } }
    }
    TestCase {
        name: "NewMacro"
        when: windowShown
        function test_library_new_requests_dialog() {
            const library = createTemporaryObject(libraryComponent, scene);
            const dialog = createTemporaryObject(component, scene);
            library.newRequested.connect(() => dialog.open());
            library.open();
            tryCompare(library, "opened", true);
            function findNew(item) {
                if (item.text === "New") return item;
                for (const child of item.children || []) {
                    const found = findNew(child);
                    if (found) return found;
                }
                return null;
            }
            const button = findNew(library.contentItem);
            verify(button);
            mouseClick(button);
            tryCompare(dialog, "opened", true);
            tryCompare(library, "visible", false);
            dialog.reject();
        }
        function test_draft_cancel_create_and_reset_data() {
            return [{tag: "desktop", width: 600, height: 700},
                    {tag: "compact", width: 380, height: 480}];
        }
        function test_draft_cancel_create_and_reset(data) {
            scene.width = data.width; scene.height = data.height;
            scene.Window.window.width = data.width; scene.Window.window.height = data.height;
            const dialog = createTemporaryObject(component, scene);
            let created = [];
            dialog.createRequested.connect(options => created.push(options));
            dialog.open();
            tryCompare(dialog, "opened", true);
            const title = findChild(dialog, "newMacroTitle");
            const advanced = findChild(dialog, "newMacroAdvanced");
            const speed = findChild(dialog, "macroSpeedSlider");
            const limit = findChild(dialog, "runLimit");
            verify(!advanced.checked);
            verify(!speed.visible);
            verify(!dialog.standardButton(Dialog.Ok).enabled);
            title.text = "Draft";
            mouseClick(advanced);
            verify(speed.visible);
            verify(dialog.width <= data.width);
            verify(dialog.height <= data.height);
            speed.value = 2.5; speed.moved();
            limit.text = "30";
            compare(created.length, 0);
            dialog.reject();
            tryCompare(dialog, "visible", false);
            compare(created.length, 0);
            dialog.open();
            tryCompare(dialog, "opened", true);
            compare(title.text, "");
            compare(speed.value, 1);
            verify(!advanced.checked);
            title.text = "  My macro  ";
            mouseClick(advanced);
            speed.forceActiveFocus();
            keyClick(Qt.Key_Right);
            verify(dialog.draft.speed_multiplier > 1);
            speed.value = 2; speed.moved();
            limit.text = "45"; limit.editingFinished();
            verify(dialog.standardButton(Dialog.Ok).enabled);
            mouseClick(dialog.standardButton(Dialog.Ok));
            compare(created.length, 1);
            compare(created[0].title, "My macro");
            compare(created[0].speedMultiplier, 2);
            compare(created[0].maxRunSecs, 45);
        }
        function test_advanced_tabs_save_draft() {
            const dialog = createTemporaryObject(component, scene);
            dialog.guardedAbilities = [{kind: "network", label: "Go online"}];
            dialog.open();
            tryCompare(dialog, "opened", true);
            findChild(dialog, "newMacroTitle").text = "Draft";
            mouseClick(findChild(dialog, "newMacroAdvanced"));
            const tabs = findChild(dialog, "macroSettingsTabs");
            compare(tabs.count, 3);
            tabs.currentIndex = 1;
            findChild(dialog, "alwaysListen").toggle();
            const mode = findChild(dialog, "conditionTimeMode");
            mode.currentIndex = 1; mode.activated(1);
            tabs.currentIndex = 2;
            dialog.edit("set_macro_restricted", {restricted: true});
            dialog.edit("set_macro_permission", {kind: "network", allowed: true});
            let result;
            dialog.createRequested.connect(options => result = options);
            mouseClick(dialog.standardButton(Dialog.Ok));
            verify(result.alwaysListen);
            compare(result.activeFrom, "09:00");
            compare(result.activeUntil, "17:00");
            compare(result.permissions, ["network"]);
        }
    }
}
