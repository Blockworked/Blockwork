import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 900; height: 800
    property var calls: []
    QtObject {
        id: translations
        function languageDetails() {
            return JSON.stringify([
                { id: "en", translated: 20, total: 20 },
                { id: "de", translated: 5, total: 20 },
                { id: "fr", translated: 0, total: 20 },
                { id: "pirate", translated: 20, total: 20 }
            ]);
        }
    }
    Component {
        id: settingsComponent
        SettingsPage {
            width: scene.width; height: scene.height
            appState: ({ language: "de", close_to_tray: false, ipc_auto_start: false })
            translator: translations
            invoke: (command, args) => scene.calls.push({command: command, args: args})
        }
    }
    TestCase {
        name: "Languages"
        when: windowShown
        function init() { scene.calls = []; Motion.durationScale = 0; }
        function open(page) {
            const dialog = findChild(page, "languageDialog");
            findChild(page, "chooseLanguage").clicked();
            tryCompare(dialog, "opened", true);
            return dialog;
        }
        function test_completeness() {
            const page = createTemporaryObject(settingsComponent, scene);
            compare(page.availableLanguages.length, 5);
            compare(page.availableLanguages.find(l => l.id === "en").percent, 100);
            compare(page.availableLanguages.find(l => l.id === "de").percent, 25);
            compare(page.availableLanguages.find(l => l.id === "fr").percent, 0);
            compare(page.availableLanguages[0].percent, -1);
            const dialog = open(page);
            compare(dialog.selectedLanguage, "de");
            dialog.reject();
        }
        function test_search_apply_and_cancel() {
            const page = createTemporaryObject(settingsComponent, scene);
            const dialog = open(page);
            const search = findChild(page, "languageSearch");
            search.text = "FR";
            tryCompare(findChild(page, "languageList"), "count", 2);
            const filteredList = findChild(page, "languageList");
            tryVerify(() => filteredList.itemAtIndex(1) !== null);
            mouseClick(filteredList.itemAtIndex(1));
            compare(dialog.selectedLanguage, "fr");
            compare(scene.calls.length, 0);
            dialog.reject();
            compare(scene.calls.length, 0);
            open(page);
            compare(search.text, "");
            compare(dialog.selectedLanguage, "de");
            const list = findChild(page, "languageList");
            list.forceActiveFocus();
            const previousLanguage = list.model[list.currentIndex - 1].id;
            keyClick(Qt.Key_Up);
            compare(dialog.selectedLanguage, previousLanguage);
            while (list.currentIndex > 0) keyClick(Qt.Key_Up);
            keyClick(Qt.Key_Return);
            compare(JSON.stringify(scene.calls.pop()), JSON.stringify({ command: "set_language", args: {language: ""} }));
        }
        function test_small_window() {
            const page = createTemporaryObject(settingsComponent, scene, {width: 420, height: 500});
            const dialog = open(page);
            verify(dialog.width <= page.width);
            verify(dialog.height <= page.height);
            verify(findChild(page, "languageList").height > 80);
            dialog.reject();
        }
    }
}
