import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

ApplicationWindow {
    id: root
    // Room around the frameless window for the drop shadow (KWin only shadows decorated windows).
    readonly property real shadowMargin: customChrome && visibility === Window.Windowed ? 18 : 0
    width: 1220 + 2 * 18
    height: 850 + 2 * 18
    minimumWidth: 600 + 2 * 18
    minimumHeight: 640 + 2 * 18
    visible: true
    title: qsTr("Blockwork")
    // macOS keeps its native title bar.
    readonly property bool customChrome: Qt.platform.os !== "osx"
    readonly property real cornerRadius: customChrome && visibility === Window.Windowed ? 10 : 0
    flags: customChrome ? (Qt.Window | Qt.FramelessWindowHint) : Qt.Window
    color: customChrome ? "transparent" : "#202124"
    property real sidebarWidth: 348
    // True while the window is being resized: the rounded-corner mask layer is
    // skipped then, since rendering the whole UI offscreen every frame is slow.
    property bool liveResizing: false
    onWidthChanged: { liveResizing = true; resizeIdle.restart(); }
    onHeightChanged: { liveResizing = true; resizeIdle.restart(); }
    Timer { id: resizeIdle; interval: 140; onTriggered: root.liveResizing = false }
    property bool sidebarCollapsed: false

    // Keep every control on the app theme instead of the system light/dark palette.
    palette {
        window: Theme.panel; windowText: Theme.text; base: Theme.field; text: Theme.text
        button: Theme.panelRaised; buttonText: Theme.text; highlight: Theme.accent; highlightedText: "white"
        placeholderText: Theme.textDim; toolTipBase: Theme.panelRaised; toolTipText: Theme.text
        mid: Theme.border; dark: Theme.borderSoft; light: Theme.panelRaised; shadow: "#000000"
    }

    property var appState: ({
        macro_names: [], macro_selected: null, current_macro: null, macros_data: [],
        global_speed_multiplier: 1, is_looping: false,
        recording_phase: { phase: "Idle", countdown: null }, page: "Main",
        can_undo: false, can_redo: false, hotkey_bindings: [], named_hotkey_defaults: [],
        ipc_active_port: null, ipc_auto_start: false, ipc_port_text: "47821",
        close_to_tray: false, start_at_startup: false, onboarding_done: true, record_mouse_relative: true, record_mouse_movement: false,
        key_capture: null, standalone_key: null, combo_capture: null,
        absolute_mouse_position_available: true, update_check_state: { state: "Idle" }
    })

    // The UI language from Settings; empty follows the system. The saved one
    // is already applied at startup, so this only handles changes.
    readonly property string uiLanguage: appState.language || ""
    onUiLanguageChanged: translator.setLanguage(uiLanguage)
    Translator { id: translator }

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
    // Keys held during a key capture, in press order; sent together when one is released.
    property var heldCaptureKeys: []
    function captureCode(event) {
        if (event.key === Qt.Key_Control) return "Control";
        if (event.key === Qt.Key_Shift) return "Shift";
        if (event.key === Qt.Key_Alt) return "Alt";
        if (event.key === Qt.Key_Meta) return "Meta";
        return root.webCode(event);
    }
    // Ctrl turns event.text into a control character, so letters come from the key code.
    function captureChar(event) {
        if (event.key >= Qt.Key_A && event.key <= Qt.Key_Z) return String.fromCharCode(97 + event.key - Qt.Key_A);
        return event.text && event.text.length === 1 && event.text >= " " ? event.text : "";
    }
    function captureKey(event) {
        if (root.appState.key_capture !== null && root.appState.key_capture !== undefined) {
            event.accepted = true;
            if (event.isAutoRepeat) return;
            const code = root.captureCode(event), key = root.captureChar(event) || code;
            if (!code.length && !key.length) return;
            if (!root.heldCaptureKeys.some(k => k[0] === code && k[1] === key))
                root.heldCaptureKeys = root.heldCaptureKeys.concat([[code, key]]);
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
            if (root.appState.key_capture === null || root.appState.key_capture === undefined) root.heldCaptureKeys = [];
            if (root.appState.key_capture !== null || root.appState.combo_capture !== null) contentRoot.forceActiveFocus();
        }
        onShouldQuitChanged: if (shouldQuit) Qt.quit()
        onFocusSerialChanged: { root.show(); root.raise(); root.requestActivate(); }
    }
    Component.onCompleted: { bridge.start(); bridge.setWindowFrameMargin(shadowMargin); }
    // Compositor treats the shadow as outside the window, so it can reach the screen edges.
    onShadowMarginChanged: bridge.setWindowFrameMargin(shadowMargin)
    onActiveChanged: if (!active && appState.combo_capture) invoke("cancel_combo_capture")
    Timer { interval: 16; running: true; repeat: true; onTriggered: bridge.poll() }

    // Pre-baked 9-slice shadow: scaling a bitmap is far cheaper per resize frame than a live blur.
    BorderImage {
        visible: root.shadowMargin > 0
        anchors.fill: parent
        source: "qrc:/icons/window-shadow.png"
        border { left: 40; right: 40; top: 40; bottom: 40 }
    }
    // Clipped to the rounded window shape on the frameless build.
    Item {
        id: frame
        anchors.fill: parent; anchors.margins: root.shadowMargin
        layer.enabled: root.cornerRadius > 0 && !root.liveResizing
        layer.smooth: true
        layer.effect: MultiEffect { maskEnabled: true; maskSource: frameMask; maskThresholdMin: 0.5; maskSpreadAtMin: 1.0 }
        Rectangle { anchors.fill: parent; color: "#202124" }
    Item {
        id:contentRoot;anchors.fill:parent;focus:true
        Keys.priority:Keys.BeforeItem
        Keys.onPressed:event=>root.captureKey(event)
        Keys.onReleased:event=>{
            if (event.isAutoRepeat || root.heldCaptureKeys.length === 0) return;
            if (root.appState.key_capture !== null && root.appState.key_capture !== undefined) {
                const keys = root.heldCaptureKeys;
                root.heldCaptureKeys = [];
                root.invoke("key_capture_event", { keys: keys });
                event.accepted = true;
            }
        }
        Loader {
            anchors.fill: parent
            sourceComponent: root.appState.onboarding_done === false ? onboardingComponent
                : root.appState.page === "Settings" ? settingsComponent : editorComponent
        }
    }
    Rectangle {
        visible: !bridge.connected
        anchors.fill: parent
        color: "#dd202124"
        z: 100
        Column {
            anchors.centerIn: parent; spacing: 12
            BusyIndicator { anchors.horizontalCenter: parent.horizontalCenter; running: bridge.lastError.length === 0 }
            Text { text: bridge.lastError.length ? bridge.lastError : qsTr("Connecting to Blockwork..."); color: bridge.lastError.length ? "#ff7373" : "#e7e7e8"; font.pixelSize: 16 }
        }
    }

    Rectangle {
        visible: bridge.lastError.length > 0 && bridge.connected
        anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
        height: 38; color: "#4b2225"; z: 110
        Text { anchors.centerIn: parent; text: bridge.lastError; color: "#ffb5b5"; font.pixelSize: 13 }
    }
    }
    Item {
        id: frameMask
        anchors.fill: frame; visible: false
        layer.enabled: root.cornerRadius > 0 && !root.liveResizing; layer.smooth: true
        Rectangle { anchors.fill: parent; radius: root.cornerRadius; color: "black"; antialiasing: true }
    }
    Component {
        id: editorComponent
        EditorPage {
            appState: root.appState; invoke: root.invoke; appBridge: bridge; macroFiles: macroFiles; blockLibrary: blockLibrary
            sidebarWidth: root.sidebarWidth
            onSidebarWidthChanged: root.sidebarWidth = sidebarWidth
            sidebarCollapsed: root.sidebarCollapsed
            onSidebarCollapsedChanged: root.sidebarCollapsed = sidebarCollapsed
        }
    }
    Component { id: onboardingComponent; OnboardingPage { appState: root.appState; invoke: root.invoke } }
    Component { id: settingsComponent; SettingsPage { appState: root.appState; invoke: root.invoke; macroFiles: macroFiles; translator: translator } }
    MacroFiles { id: macroFiles; appState: root.appState; invoke: root.invoke; appBridge: bridge }
    BlockLibrary { id: blockLibrary; appState: root.appState; invoke: root.invoke; appBridge: bridge; macroFiles: macroFiles }

    // Frameless window: hairline outline and edge/corner resize grips.
    Rectangle {
        visible: root.customChrome && root.visibility !== Window.Maximized
        anchors.fill: parent; anchors.margins: root.shadowMargin
        color: "transparent"; radius: root.cornerRadius; border.color: "#45474d"; z: 150
    }
    Repeater {
        model: root.customChrome && root.visibility === Window.Windowed ? [
            { e: Qt.TopEdge, c: Qt.SizeVerCursor }, { e: Qt.BottomEdge, c: Qt.SizeVerCursor },
            { e: Qt.LeftEdge, c: Qt.SizeHorCursor }, { e: Qt.RightEdge, c: Qt.SizeHorCursor },
            { e: Qt.TopEdge | Qt.LeftEdge, c: Qt.SizeFDiagCursor }, { e: Qt.BottomEdge | Qt.RightEdge, c: Qt.SizeFDiagCursor },
            { e: Qt.TopEdge | Qt.RightEdge, c: Qt.SizeBDiagCursor }, { e: Qt.BottomEdge | Qt.LeftEdge, c: Qt.SizeBDiagCursor }
        ] : []
        delegate: MouseArea {
            required property var modelData
            readonly property int grip: Math.max(5, root.shadowMargin)
            readonly property bool edgeL: (modelData.e & Qt.LeftEdge) !== 0
            readonly property bool edgeR: (modelData.e & Qt.RightEdge) !== 0
            readonly property bool edgeT: (modelData.e & Qt.TopEdge) !== 0
            readonly property bool edgeB: (modelData.e & Qt.BottomEdge) !== 0
            readonly property bool corner: (edgeL || edgeR) && (edgeT || edgeB)
            z: 200; cursorShape: modelData.c
            x: edgeR ? root.width - (corner ? grip * 2 : grip) : 0
            y: edgeB ? root.height - (corner ? grip * 2 : grip) : 0
            width: (edgeL || edgeR) ? (corner ? grip * 2 : grip) : root.width
            height: (edgeT || edgeB) ? (corner ? grip * 2 : grip) : root.height
            onPressed: root.startSystemResize(modelData.e)
        }
    }

}
