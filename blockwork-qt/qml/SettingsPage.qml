import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke
    function capturingAction(action){return appState.combo_capture&&appState.combo_capture.kind==="Named"&&JSON.stringify(appState.combo_capture.action)===JSON.stringify(action)}

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 68; color: "#292a2d"; border.color: "#3a3b3f"
            RowLayout { anchors.fill: parent; anchors.margins: 14
                BwButton { iconName: "arrow-left"; text: "Back"; onClicked: root.invoke("close_settings") }
                Text { text: "Settings"; color: "#e7e7e8"; font.pixelSize: 17; font.weight: Font.Bold }
                Item { Layout.fillWidth: true }
            }
        }
        ScrollView {
            Layout.fillWidth: true; Layout.fillHeight: true; clip: true
            Column {
                width: Math.max(760, root.width - 36); x: Math.max(18, (root.width - width) / 2); spacing: 16; padding: 0
                Item { width: 1; height: 4 }
                SectionCard {
                    width: parent.width; title: "Global hotkeys"; iconName: "keyboard"
                    Repeater {
                        model: appState.named_hotkey_defaults || []
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            property var binding: (appState.hotkey_bindings || []).find(b => JSON.stringify(b.action) === JSON.stringify(modelData.action))
                            Text { Layout.fillWidth: true; text: root.actionLabel(modelData.action); color: "#e7e7e8"; font.pixelSize: 14 }
                            Rectangle { width: 138; height: 36; radius: 7; color: "#303136"; border.color: "#4a4b50"
                                Text { anchors.centerIn: parent; text: root.capturingAction(modelData.action)?"Press shortcut…":(parent.parent.binding ? parent.parent.binding.combo_display : (modelData.combo_display || "Not set")); color: root.capturingAction(modelData.action)?Theme.accent:"#a4a5aa"; font.pixelSize: 13 }
                                TapHandler { onTapped: root.invoke("start_combo_capture", { action: modelData.action }) }
                            }
                            BwButton { iconName: "x"; text: ""; danger: true; implicitWidth: 42; onClicked: root.invoke("clear_named_hotkey", { action: modelData.action }) }
                        }
                    }
                }
                SectionCard {
                    width: parent.width; title: "Per-macro hotkeys"; iconName: "keyboard"
                    Repeater {
                        model: (appState.hotkey_bindings || []).filter(b => b.action && b.action.type === "RunSpecificMacro")
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            Text { Layout.fillWidth: true; text: modelData.macro_name || "Macro"; color: "#e7e7e8"; font.pixelSize: 14 }
                            Text { text: modelData.combo_display; color: "#a4a5aa"; font.pixelSize: 13 }
                            BwButton { iconName: "x"; text: ""; danger: true; implicitWidth: 42; onClicked: root.invoke("remove_hotkey_binding", { index: modelData.binding_index }) }
                        }
                    }
                    RowLayout { Layout.fillWidth: true
                        Text { text: "Add hotkey:"; color: "#e7e7e8" }
                        Item { Layout.fillWidth: true }
                        BwComboBox { id: macroBox; model: appState.macro_names || []; implicitWidth: 210; onActivated: index => root.invoke("set_pending_macro_idx", { index: index }) }
                        BwButton { text: appState.combo_capture&&appState.combo_capture.kind==="Pending"?"Press shortcut…":"Set combo"; primary:appState.combo_capture&&appState.combo_capture.kind==="Pending"; onClicked: root.invoke("start_pending_combo_capture") }
                        BwButton { iconName: "plus"; text: "Add"; onClicked: root.invoke("add_macro_hotkey") }
                    }
                }
                SectionCard {
                    width: parent.width; title: "Import and export"; iconName: "layers"
                    RowLayout { Layout.fillWidth: true
                        ColumnLayout { Layout.fillWidth: true
                            Text { text: "Portable macro files"; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                            Text { text: "Import or export .macro files without changing the browser frontend."; color: "#9fa0a6"; font.pixelSize: 12 }
                        }
                        BwButton { text: "Import…"; onClicked: pathDialog.mode = "import", pathDialog.open() }
                        BwButton { text: "Export…"; enabled: appState.macro_selected !== null; onClicked: pathDialog.mode = "export", pathDialog.open() }
                    }
                }
                SectionCard {
                    width: parent.width; title: "TCP server"; iconName: "terminal"
                    RowLayout { Layout.fillWidth: true
                        ColumnLayout { Layout.fillWidth: true
                            Text { text: appState.ipc_active_port === null ? "Server stopped" : "Listening on port " + appState.ipc_active_port; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                            Text { text: "Allow local integrations to run Blockwork macros."; color: "#9fa0a6"; font.pixelSize: 12 }
                        }
                        BwTextField { id: port; text: appState.ipc_port_text || "47821"; validator: IntValidator { bottom: 1; top: 65535 } implicitWidth: 100; onEditingFinished: root.invoke("set_ipc_port_text", { text: text }) }
                        BwButton { text: appState.ipc_active_port === null ? "Start" : "Stop"; primary: appState.ipc_active_port === null; onClicked: root.invoke(appState.ipc_active_port === null ? "start_ipc_server" : "stop_ipc_server") }
                    }
                    BwSwitch { Accessible.name: "Start TCP server automatically"; checked: appState.ipc_auto_start; onToggled: checked => root.invoke("set_ipc_auto_start", { enabled: checked }) }
                }
                SectionCard {
                    width: parent.width; title: "System"; iconName: "settings"
                    RowLayout { Layout.fillWidth: true
                        ColumnLayout { Layout.fillWidth: true
                            Text { text: "Close to tray"; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                            Text { text: "Keep hotkeys and schedules active after closing the editor."; color: "#9fa0a6"; font.pixelSize: 12 }
                        }
                        BwSwitch { checked: appState.close_to_tray; onToggled: checked => root.invoke("set_close_to_tray", { enabled: checked }) }
                    }
                }
                SectionCard {
                    width: parent.width; title: "Updates"; iconName: "refresh-cw"
                    RowLayout { Layout.fillWidth: true
                        ColumnLayout { Layout.fillWidth: true
                            Text { text: root.updateTitle(); color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                            Text { text: root.updateDetail(); color: "#9fa0a6"; font.pixelSize: 12 }
                        }
                        BwButton { text: appState.update_check_state && appState.update_check_state.state === "UpdateAvailable" ? "Install update" : "Check for updates"; primary: appState.update_check_state && appState.update_check_state.state === "UpdateAvailable"; onClicked: root.invoke(appState.update_check_state && appState.update_check_state.state === "UpdateAvailable" ? "apply_update" : "check_for_updates") }
                    }
                }
                Item { width: 1; height: 18 }
            }
        }
    }

    function actionLabel(action) {
        if (!action) return "Hotkey";
        const labels = { RunMacro: "Run Macro", StopLoop: "Stop Loop", NextMacro: "Next Macro", PrevMacro: "Previous Macro", ToggleLoop: "Toggle Loop", StartRecordingImmediate: "Start Recording (immediate)", StopRecording: "Stop Recording", Undo: "Undo", Redo: "Redo" };
        return labels[action.type] || action.type;
    }
    function updateTitle() {
        const u = appState.update_check_state || { state: "Idle" };
        if (u.state === "Checking") return "Checking for updates…";
        if (u.state === "UpdateAvailable") return "Blockwork " + u.version + " is available";
        if (u.state === "UpToDate") return "Blockwork is up to date";
        if (u.state === "Error") return "Update check failed";
        return "Software updates";
    }
    function updateDetail() {
        const u = appState.update_check_state || {};
        return u.error || "Check for a new release of Blockwork.";
    }

    Dialog {
        id: pathDialog
        property string mode: "import"
        anchors.centerIn: parent; modal: true
        title: mode === "import" ? "Import macro" : "Export macro"
        standardButtons: Dialog.Ok | Dialog.Cancel
        background:Rectangle{radius:10;color:Theme.panel;border.color:Theme.border}
        Column { width: 480; spacing: 8
            Text { text: "File path"; color: "#e7e7e8" }
            BwTextField { id: filePath; width: parent.width; placeholderText: "C:/path/to/macro.macro" }
        }
        onAccepted: {
            if (mode === "import") root.invoke("import_macro", { path: filePath.text });
            else if (appState.current_macro) root.invoke("export_macro", { macroId: appState.current_macro.id, path: filePath.text });
        }
    }
}
