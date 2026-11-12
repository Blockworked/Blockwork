import QtQuick
import QtQuick.Controls
import QtTest
import QWindowKit 1.0
import com.blockworked.Blockwork 1.0
import "../../qml/window" as Chrome

Item {
    Component {
        id: windowComponent
        ApplicationWindow {
            id: window
            width: 800; height: 500
            flags: Qt.Window | Qt.WindowMinMaxButtonsHint | Qt.WindowCloseButtonHint | Qt.WindowSystemMenuHint
            visible: chrome.ready
            property var windowChrome: chrome
            property alias toolbar: toolbarLoader.item
            property alias toolbarSource: toolbarLoader.sourceComponent
            property int clicks: 0
            Chrome.WindowChrome { id: chrome; Component.onCompleted: attach(window) }
            Loader { id: toolbarLoader; width: parent.width; height: 52; sourceComponent: toolbarComponent }
        }
    }
    Component {
        id: toolbarComponent
        Item {
            property alias action: action
            property alias buttons: buttons
            property alias nativeButtons: nativeButtons
            TitleBarDrag {
                anchors.fill: parent
                interactiveItems: [action]; windowButtons: buttons; nativeButtons: parent.nativeButtons
            }
            Item { id: nativeButtons; x: 12; y: 9; width: 72; height: 34 }
            Button { id: action; x: 100; y: 9; width: 100; height: 34; text: "Action"; onClicked: Window.window.clicks++ }
            WindowControls { id: buttons; x: 660; y: 9; visible: Qt.platform.os !== "osx" }
        }
    }
    Component { id: blankComponent; Item {} }
    TestCase {
        name: "NativeWindowChrome"
        when: windowShown

        function editorWindow() {
            const window = createTemporaryObject(windowComponent, null);
            verify(window);
            tryCompare(window, "visible", true);
            waitForRendering(window.contentItem);
            return window;
        }
        function test_toolbar_control_stays_interactive() {
            const window = editorWindow();
            mouseClick(window.toolbar.action);
            compare(window.clicks, 1);
            window.toolbar.action.forceActiveFocus();
            keyClick(Qt.Key_Space);
            compare(window.clicks, 2);
            window.close();
        }
        function test_titlebar_double_click() {
            if (Qt.platform.os === "osx") skip("Native macOS title-bar behavior needs a desktop session");
            const window = editorWindow();
            mouseDoubleClickSequence(window.toolbar, 400, 26);
            tryCompare(window, "visibility", Window.Maximized);
            mouseDoubleClickSequence(window.toolbar, 400, 26);
            tryCompare(window, "visibility", Window.Windowed);
            window.close();
        }
        function test_system_buttons_and_page_replacement() {
            const window = editorWindow();
            const agent = findChild(window.windowChrome, "windowAgent");
            verify(agent);
            tryVerify(() => agent.titleBar() === findChild(window.toolbar, "windowDragArea"));
            const first = window.toolbar;
            window.toolbarSource = blankComponent;
            window.toolbarSource = toolbarComponent;
            tryVerify(() => window.toolbar && window.toolbar !== first);
            tryVerify(() => agent.titleBar() === findChild(window.toolbar, "windowDragArea"));
            if (Qt.platform.os === "osx") {
                compare(agent.systemButtonArea(), window.toolbar.nativeButtons);
            } else {
                compare(agent.systemButton(WindowAgent.Minimize), window.toolbar.buttons.minimizeButton);
                compare(agent.systemButton(WindowAgent.Maximize), window.toolbar.buttons.maximizeButton);
                compare(agent.systemButton(WindowAgent.Close), window.toolbar.buttons.closeButton);
                mouseClick(window.toolbar.buttons.maximizeButton);
                tryCompare(window, "visibility", Window.Maximized);
                window.toolbar.buttons.maximizeButton.forceActiveFocus();
                keyClick(Qt.Key_Space);
                tryCompare(window, "visibility", Window.Windowed);
                mouseClick(window.toolbar.buttons.minimizeButton);
                tryCompare(window, "visibility", Window.Minimized);
                window.showNormal();
                tryCompare(window, "visibility", Window.Windowed);
                mouseClick(window.toolbar.buttons.closeButton);
                tryCompare(window, "visible", false);
            }
            window.close();
        }
    }
}
