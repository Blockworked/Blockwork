import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

ApplicationWindow {
    id: root
    width: 1220
    height: 850
    minimumWidth: 900
    minimumHeight: 640
    visible: true
    title: "Blockwork"
    color: "#202124"
    property real sidebarWidth: 348

    // Keep every control on the app theme instead of the system light/dark palette.
    palette {
        window: Theme.panel; windowText: Theme.text; base: Theme.field; text: Theme.text
        button: Theme.panelRaised; buttonText: Theme.text; highlight: Theme.accent; highlightedText: "white"
        placeholderText: Theme.textDim; toolTipBase: Theme.panelRaised; toolTipText: Theme.text
        mid: Theme.border; dark: Theme.borderSoft; light: Theme.panelRaised; shadow: "#000000"
    }

    property var appState: ({
        macro_names: [], macro_selected: null, current_macro: null, macros_data: [],
        loop_mode_enabled: false, global_speed_multiplier: 1, is_looping: false,
        recording_phase: { phase: "Idle", countdown: null }, page: "Main",
        can_undo: false, can_redo: false, hotkey_bindings: [], named_hotkey_defaults: [],
        ipc_active_port: null, ipc_auto_start: false, ipc_port_text: "47821",
        close_to_tray: false, record_mouse_relative: true, record_mouse_movement: false,
        key_capture: null, standalone_key: null, combo_capture: null,
        absolute_mouse_position_available: true, update_check_state: { state: "Idle" }
    })

    function invoke(command, args) {
        bridge.invokeCommand(command, JSON.stringify(args || {}));
    }

    function webCode(event) {
        if (event.key >= Qt.Key_A && event.key <= Qt.Key_Z)
            return "Key" + String.fromCharCode(65 + event.key - Qt.Key_A);
        if (event.key >= Qt.Key_0 && event.key <= Qt.Key_9)
            return "Digit" + String.fromCharCode(48 + event.key - Qt.Key_0);
        if (event.key >= Qt.Key_F1 && event.key <= Qt.Key_F24)
            return "F" + String(event.key - Qt.Key_F1 + 1);
        const codes = ({});
        codes[Qt.Key_Space]="Space"; codes[Qt.Key_Return]="Enter"; codes[Qt.Key_Enter]="NumpadEnter";
        codes[Qt.Key_Escape]="Escape"; codes[Qt.Key_Tab]="Tab"; codes[Qt.Key_Backspace]="Backspace";
        codes[Qt.Key_Delete]="Delete"; codes[Qt.Key_Insert]="Insert"; codes[Qt.Key_Home]="Home"; codes[Qt.Key_End]="End";
        codes[Qt.Key_PageUp]="PageUp"; codes[Qt.Key_PageDown]="PageDown"; codes[Qt.Key_Left]="ArrowLeft";
        codes[Qt.Key_Right]="ArrowRight"; codes[Qt.Key_Up]="ArrowUp"; codes[Qt.Key_Down]="ArrowDown";
        codes[Qt.Key_Minus]="Minus"; codes[Qt.Key_Equal]="Equal"; codes[Qt.Key_BracketLeft]="BracketLeft";
        codes[Qt.Key_BracketRight]="BracketRight"; codes[Qt.Key_Backslash]="Backslash"; codes[Qt.Key_Semicolon]="Semicolon";
        codes[Qt.Key_Apostrophe]="Quote"; codes[Qt.Key_Comma]="Comma"; codes[Qt.Key_Period]="Period"; codes[Qt.Key_Slash]="Slash";
        return codes[event.key] || "";
    }
    function captureKey(event) {
        if (root.appState.key_capture !== null && root.appState.key_capture !== undefined) {
            const code = root.webCode(event);
            if (code.length) root.invoke("key_capture_event", { code: code, key: event.text || code });
            event.accepted = true;
            return;
        }
        if (root.appState.combo_capture !== null && root.appState.combo_capture !== undefined) {
            if (event.key === Qt.Key_Escape) root.invoke("cancel_combo_capture");
            else if ([Qt.Key_Control,Qt.Key_Shift,Qt.Key_Alt,Qt.Key_Meta].indexOf(event.key)<0) {
                const code = root.webCode(event);
                if (code.length) {
                    const modifiers = ((event.modifiers & Qt.ControlModifier)?1:0) | ((event.modifiers & Qt.ShiftModifier)?2:0)
                            | ((event.modifiers & Qt.AltModifier)?4:0) | ((event.modifiers & Qt.MetaModifier)?8:0);
                    root.invoke("combo_capture_event", { code: code, modifiers: modifiers });
                }
            }
            event.accepted = true;
        }
    }

    AppBridge {
        id: bridge
        onStateJsonChanged: {
            try { root.appState = JSON.parse(stateJson); }
            catch (error) { console.warn("Invalid Blockwork state", error); }
            if (root.appState.key_capture !== null || root.appState.combo_capture !== null) contentRoot.forceActiveFocus();
        }
        onShouldQuitChanged: if (shouldQuit) Qt.quit()
        onFocusSerialChanged: { root.show(); root.raise(); root.requestActivate(); }
    }
    Component.onCompleted: bridge.start()
    Timer { interval: 16; running: true; repeat: true; onTriggered: bridge.poll() }

    Item {
        id:contentRoot;anchors.fill:parent;focus:true
        Keys.priority:Keys.BeforeItem
        Keys.onPressed:event=>root.captureKey(event)
        Loader {
            anchors.fill: parent
            sourceComponent: root.appState.page === "Settings" ? settingsComponent : editorComponent
        }
    }
    Component { id: editorComponent; EditorPage { appState: root.appState; invoke: root.invoke; appBridge: bridge; sidebarWidth: root.sidebarWidth; onSidebarWidthChanged: root.sidebarWidth = sidebarWidth } }
    Component { id: settingsComponent; SettingsPage { appState: root.appState; invoke: root.invoke } }

    Rectangle {
        visible: !bridge.connected
        anchors.fill: parent
        color: "#dd202124"
        z: 100
        Column {
            anchors.centerIn: parent; spacing: 12
            BusyIndicator { anchors.horizontalCenter: parent.horizontalCenter; running: bridge.lastError.length === 0 }
            Text { text: bridge.lastError.length ? bridge.lastError : "Connecting to Blockwork…"; color: bridge.lastError.length ? "#ff7373" : "#e7e7e8"; font.pixelSize: 16 }
        }
    }

    Rectangle {
        visible: bridge.lastError.length > 0 && bridge.connected
        anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
        height: 38; color: "#4b2225"; z: 110
        Text { anchors.centerIn: parent; text: bridge.lastError; color: "#ffb5b5"; font.pixelSize: 13 }
    }
}
