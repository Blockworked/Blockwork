import QtQuick
import QWindowKit 1.0

Item {
    id: root
    property bool ready: false
    property bool dark: false
    onDarkChanged: applyAppearance()
    WindowAgent { id: agent; objectName: "windowAgent" }

    function attach(window) {
        ready = agent.setup(window);
        if (!ready) {
            console.error("QWindowKit could not attach to the editor window");
            Qt.quit();
        }
        applyAppearance();
    }

    function applyAppearance() {
        if (ready && Qt.platform.os === "windows")
            agent.setWindowAttribute("dark-mode", dark);
    }

    function registerTitleBar(bar, controls, buttons, nativeButtons) {
        agent.setTitleBar(bar);
        for (const control of controls) agent.setHitTestVisible(control, true);
        if (Qt.platform.os === "osx") {
            agent.setSystemButtonArea(nativeButtons);
        } else {
            agent.setSystemButton(WindowAgent.Minimize, buttons.minimizeButton);
            agent.setSystemButton(WindowAgent.Maximize, buttons.maximizeButton);
            agent.setSystemButton(WindowAgent.Close, buttons.closeButton);
            agent.setHitTestVisible(buttons, true);
        }
    }
}
