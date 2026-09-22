import QtQuick
import QtQuick.Controls
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

    property var appState: ({
        macro_names: [], macro_selected: null, current_macro: null, macros_data: [],
        loop_mode_enabled: false, global_speed_multiplier: 1, is_looping: false,
        recording_phase: { phase: "Idle", countdown: null }, page: "Main",
        can_undo: false, can_redo: false, hotkey_bindings: [], named_hotkey_defaults: [],
        ipc_active_port: null, ipc_auto_start: false, ipc_port_text: "47821",
        close_to_tray: false, record_mouse_relative: true, record_mouse_movement: false,
        absolute_mouse_position_available: true, update_check_state: { state: "Idle" }
    })

    function invoke(command, args) {
        bridge.invokeCommand(command, JSON.stringify(args || {}));
    }

    AppBridge {
        id: bridge
        onStateJsonChanged: {
            try { root.appState = JSON.parse(stateJson); }
            catch (error) { console.warn("Invalid Blockwork state", error); }
        }
        onShouldQuitChanged: if (shouldQuit) Qt.quit()
        onFocusSerialChanged: { root.show(); root.raise(); root.requestActivate(); }
    }
    Component.onCompleted: bridge.start()
    Timer { interval: 16; running: true; repeat: true; onTriggered: bridge.poll() }

    Loader {
        anchors.fill: parent
        sourceComponent: root.appState.page === "Settings" ? settingsComponent : editorComponent
    }
    Component { id: editorComponent; EditorPage { appState: root.appState; invoke: root.invoke } }
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
