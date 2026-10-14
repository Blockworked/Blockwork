import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke
    function capturingBinding(index){return appState.combo_capture&&appState.combo_capture.kind==="Binding"&&appState.combo_capture.binding_index===index}
    function capturingAction(action){return appState.combo_capture&&appState.combo_capture.kind==="Named"&&JSON.stringify(appState.combo_capture.action)===JSON.stringify(action)}

    property int tab: 0
    readonly property var tabs: [
        { name: "General", icon: "settings" }, { name: "Hotkeys", icon: "keyboard" },
        { name: "Editor", icon: "blocks" }, { name: "Integrations", icon: "terminal" }
    ]

    component SettingsRow: ColumnLayout {
        id: row
        property string title: ""
        property string detail: ""
        property bool first: false
        default property alias controls: ctl.data
        Layout.fillWidth: true; spacing: 0
        Rectangle { Layout.fillWidth: true; height: 1; color: "#3a3b3f"; visible: !row.first }
        RowLayout {
            Layout.fillWidth: true; Layout.topMargin: 14; Layout.bottomMargin: 14; spacing: 16
            ColumnLayout { Layout.fillWidth: true; spacing: 2
                Text { text: row.title; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                Text { visible: row.detail !== ""; text: row.detail; color: "#9fa0a6"; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            }
            RowLayout { id: ctl; spacing: 8 }
        }
    }

    component SubHeading: Text {
        Layout.topMargin: 6; Layout.bottomMargin: 2
        color: "#a7a8ae"; font.pixelSize: 12; font.weight: Font.Bold; font.letterSpacing: 1.2
    }

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 68; color: "#292a2d"; border.color: "#3a3b3f"
            TitleBarDrag { anchors.fill: parent; visible: Qt.platform.os !== "osx" }
            RowLayout { anchors.fill: parent; anchors.margins: 14
                Image {
                    visible: Qt.platform.os !== "osx"
                    source: "qrc:/icons/blockwork.png"; sourceSize: Qt.size(40, 40)
                    Layout.preferredWidth: 20; Layout.preferredHeight: 20; Layout.rightMargin: 2
                    fillMode: Image.PreserveAspectFit; smooth: true; mipmap: true
                }
                BwButton { iconName: "arrow-left"; text: "Back"; onClicked: root.invoke("close_settings") }
                Text { text: "Settings"; color: "#e7e7e8"; font.pixelSize: 17; font.weight: Font.Bold }
                Item { Layout.fillWidth: true }
                WindowControls { visible: Qt.platform.os !== "osx" }
            }
        }

        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 0

            Rectangle {
                Layout.preferredWidth: 190; Layout.fillHeight: true
                color: "#252629"
                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: "#3a3b3f" }
                Column {
                    anchors.fill: parent; anchors.margins: 10; spacing: 2
                    Repeater {
                        model: root.tabs
                        delegate: Rectangle {
                            id: navItem
                            required property var modelData
                            required property int index
                            width: parent.width; height: 38; radius: 8
                            color: root.tab === index ? "#2f3b4d" : (navMouse.containsMouse ? "#2e2f33" : "transparent")
                            Row { anchors.left: parent.left; anchors.leftMargin: 12; anchors.verticalCenter: parent.verticalCenter; spacing: 10
                                LucideIcon { name: navItem.modelData.icon; width: 16; height: 16; anchors.verticalCenter: parent.verticalCenter
                                    color: root.tab === navItem.index ? "#6cb2ff" : "#9da0a8" }
                                Text { text: navItem.modelData.name; font.pixelSize: 14; anchors.verticalCenter: parent.verticalCenter
                                    font.weight: root.tab === navItem.index ? Font.DemiBold : Font.Normal
                                    color: root.tab === navItem.index ? "#6cb2ff" : "#c4c5ca" }
                            }
                            MouseArea { id: navMouse; anchors.fill: parent; hoverEnabled: true; onClicked: root.tab = navItem.index }
                        }
                    }
                }
            }

            ScrollView {
                id: settingsScroll
                Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                contentWidth: availableWidth
                WheelPan { flickable: settingsScroll.contentItem }
                Item {
                    width: settingsScroll.availableWidth
                    implicitHeight: pane.implicitHeight + 48
                    ColumnLayout {
                        id: pane
                        x: 28; y: 24; width: Math.min(720, parent.width - 56); spacing: 0

                        // General
                        ColumnLayout { Layout.fillWidth: true; spacing: 0; visible: root.tab === 0
                            Text { text: "General"; color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SettingsRow { first: true; title: "Close to tray"; detail: "Keep hotkeys and schedules active after closing the editor."
                                BwSwitch { checked: appState.close_to_tray; onToggled: checked => root.invoke("set_close_to_tray", { enabled: checked }) }
                            }
                            SettingsRow { title: "Start at system startup"; detail: "Launch Blockwork in the background when you log in so hotkeys work right away."
                                BwSwitch { checked: appState.start_at_startup === true; onToggled: checked => root.invoke("set_start_at_startup", { enabled: checked }) }
                            }
                            SettingsRow { title: root.updateTitle(); detail: root.updateDetail()
                                BwButton {
                                    readonly property bool available: appState.update_check_state && appState.update_check_state.state === "UpdateAvailable"
                                    text: available ? "Install update" : "Check for updates"; primary: available
                                    onClicked: root.invoke(available ? "apply_update" : "check_for_updates")
                                }
                            }
                            SettingsRow { title: "Global variables"
                                detail: (appState.globals || []).length ? "Shared by every macro and kept after restarts. Set them with the \u201cset global\u201d block."
                                    : "None yet. The \u201cset global\u201d block makes one: a value every macro can read, kept after restarts."
                            }
                            Repeater {
                                model: appState.globals || []
                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 6; spacing: 10
                                    Text { text: modelData.name; color: "#e7e7e8"; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.maximumWidth: 220 }
                                    Text { text: modelData.value; color: "#9fa0a6"; font.pixelSize: 13; elide: Text.ElideRight; Layout.fillWidth: true }
                                    BwButton { iconName: "trash-2"; text: "Delete"; onClicked: root.invoke("delete_global", { name: modelData.name }) }
                                }
                            }
                            Rectangle { Layout.fillWidth: true; Layout.topMargin: 8; height: 1; color: "#3a3b3f" }
                            RowLayout { Layout.fillWidth: true; Layout.topMargin: 16; spacing: 8
                                BwButton { iconName: "download"; text: "Import macro…"; onClicked: pathDialog.mode = "import", pathDialog.open() }
                                BwButton { iconName: "upload"; text: "Export macro…"; enabled: appState.macro_selected !== null; onClicked: pathDialog.mode = "export", pathDialog.open() }
                                BwButton { iconName: "play"; text: "Replay welcome tour"; onClicked: root.invoke("set_onboarding_done", { done: false }) }
                                Item { Layout.fillWidth: true }
                            }
                        }

                        // Hotkeys
                        ColumnLayout { Layout.fillWidth: true; spacing: 0; visible: root.tab === 1
                            Text { text: "Hotkeys"; color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SubHeading { text: "GLOBAL" }
                            Repeater {
                                model: appState.named_hotkey_defaults || []
                                delegate: SettingsRow {
                                    id: hotkeyRow
                                    required property var modelData
                                    required property int index
                                    first: index === 0
                                    title: root.actionLabel(modelData.action)
                                    readonly property var binding: (appState.hotkey_bindings || []).find(b => JSON.stringify(b.action) === JSON.stringify(modelData.action))
                                    readonly property bool capturing: root.capturingAction(modelData.action)
                                    readonly property bool atDefault: modelData.combo_display == null || (binding ? binding.combo_display : null) === modelData.combo_display
                                    BwButton {
                                        text: hotkeyRow.capturing ? "Press shortcut…" : (hotkeyRow.binding ? hotkeyRow.binding.combo_display : "Not set")
                                        primary: hotkeyRow.capturing; implicitWidth: 150; implicitHeight: 36
                                        onClicked: if (!hotkeyRow.capturing) root.invoke("start_combo_capture", { action: hotkeyRow.modelData.action })
                                    }
                                    Item { Layout.preferredWidth: 84; Layout.preferredHeight: 36
                                        BwButton { anchors.fill: parent; visible: !hotkeyRow.capturing && !hotkeyRow.atDefault; text: "Default"; implicitHeight: 36
                                            onClicked: root.invoke("reset_hotkey_to_default", { action: hotkeyRow.modelData.action }) }
                                    }
                                    Item { Layout.preferredWidth: 42; Layout.preferredHeight: 36
                                        BwButton { anchors.fill: parent; visible: !hotkeyRow.capturing && !!hotkeyRow.binding; iconName: "x"; text: ""; danger: true; implicitHeight: 36
                                            onClicked: root.invoke("clear_named_hotkey", { action: hotkeyRow.modelData.action }) }
                                    }
                                }
                            }
                            SubHeading { text: "PER MACRO"; Layout.topMargin: 24 }
                            Repeater {
                                model: (appState.hotkey_bindings || []).filter(b => b.action && b.action.type === "RunSpecificMacro")
                                delegate: SettingsRow {
                                    id: macroRow
                                    required property var modelData
                                    required property int index
                                    first: index === 0
                                    title: modelData.macro_name || "Macro"
                                    readonly property bool capturing: root.capturingBinding(modelData.binding_index)
                                    BwButton {
                                        text: macroRow.capturing ? "Press shortcut…" : macroRow.modelData.combo_display
                                        primary: macroRow.capturing; implicitWidth: 150; implicitHeight: 36
                                        onClicked: if (!macroRow.capturing) root.invoke("start_binding_combo_capture", { index: macroRow.modelData.binding_index })
                                    }
                                    BwButton { iconName: "x"; text: ""; danger: true; implicitWidth: 42; implicitHeight: 36
                                        onClicked: root.invoke("remove_hotkey_binding", { index: macroRow.modelData.binding_index }) }
                                }
                            }
                            RowLayout { Layout.fillWidth: true; Layout.topMargin: 14; spacing: 8
                                Text { text: "Add hotkey"; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                                Item { Layout.fillWidth: true }
                                BwComboBox { id: macroBox; model: appState.macro_names || []; implicitWidth: 210; onActivated: index => root.invoke("set_pending_macro_idx", { index: index }) }
                                BwButton { text: appState.combo_capture&&appState.combo_capture.kind==="Pending"?"Press shortcut…":"Set combo"; primary:appState.combo_capture&&appState.combo_capture.kind==="Pending"; onClicked: root.invoke("start_pending_combo_capture") }
                                BwButton { iconName: "plus"; text: "Add"; onClicked: root.invoke("add_macro_hotkey") }
                            }
                        }

                        // Editor
                        ColumnLayout { Layout.fillWidth: true; spacing: 0; visible: root.tab === 2
                            Text { text: "Editor"; color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SettingsRow { first: true; title: "Block palette layout"; detail: "How the block sidebar is organized in the editor."
                                BwComboBox {
                                    id: paletteLayoutBox
                                    readonly property var layouts: [
                                        { id: "rail", text: "Category rail" }, { id: "chips", text: "Chip strip" }, { id: "accordion", text: "Accordion" },
                                        { id: "flyout", text: "Rail with flyout" }, { id: "tiles", text: "Tile grid" }
                                    ]
                                    model: layouts; textRole: "text"; implicitWidth: 210
                                    currentIndex: Math.max(0, layouts.findIndex(l => l.id === (appState.palette_layout || "rail")))
                                    onActivated: index => root.invoke("set_palette_layout", { layout: layouts[index].id })
                                }
                            }
                        }

                        // Integrations
                        ColumnLayout { Layout.fillWidth: true; spacing: 0; visible: root.tab === 3
                            Text { text: "Integrations"; color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SettingsRow { first: true
                                title: appState.ipc_active_port === null ? "TCP server stopped" : "TCP server listening on port " + appState.ipc_active_port
                                detail: "Allow local integrations to run Blockwork macros."
                                BwTextField { id: port; text: appState.ipc_port_text || "47821"; validator: IntValidator { bottom: 1; top: 65535 } implicitWidth: 100; onEditingFinished: root.invoke("set_ipc_port_text", { text: text }) }
                                BwButton { text: appState.ipc_active_port === null ? "Start" : "Stop"; primary: appState.ipc_active_port === null; onClicked: root.invoke(appState.ipc_active_port === null ? "start_ipc_server" : "stop_ipc_server") }
                            }
                            SettingsRow { title: "Start automatically"; detail: "Start the TCP server whenever Blockwork launches."
                                BwSwitch { Accessible.name: "Start TCP server automatically"; checked: appState.ipc_auto_start; onToggled: checked => root.invoke("set_ipc_auto_start", { enabled: checked }) }
                            }
                            SettingsRow { title: "Command line"
                                detail: "Scripts and launchers can run macros by name: \u201cblockwork run <name>\u201d (add --wait to return when it ends), \u201cblockwork stop <name>\u201d and \u201cblockwork list\u201d."
                            }
                        }
                    }
                }
            }
        }
    }

    function actionLabel(action) {
        if (!action) return "Hotkey";
        const labels = { RunMacro: "Run Macro", StopLoop: "Stop Macro", EmergencyStop: "Emergency Stop (pauses triggers)", NextMacro: "Next Macro", PrevMacro: "Previous Macro", StartRecordingImmediate: "Start Recording (immediate)", StopRecording: "Stop Recording", Undo: "Undo", Redo: "Redo" };
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

    // An imported macro waits here until the user has seen what it can do.
    readonly property var importReview: appState.import_review || null
    readonly property string importReviewKey: importReview ? JSON.stringify(importReview) : ""
    property var keepSettings: ({})
    onImportReviewKeyChanged: {
        if (importReview) { keepSettings = ({}); importDialog.open(); }
        else importDialog.close();
    }
    BwDialog {
        id: importDialog
        readonly property var caps: root.importReview ? root.importReview.capabilities : []
        readonly property var customSettings: root.importReview ? root.importReview.custom_settings : []
        title: root.importReview ? "Import \u201c" + root.importReview.name + "\u201d?" : "Import macro"
        standardButtons: Dialog.Ok | Dialog.Cancel
        Component.onCompleted: standardButton(Dialog.Ok).text = "Import"
        Column { width: 460; spacing: 10
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: importDialog.caps.length ? "This macro can:" : "This macro only uses blocks that stay inside Blockwork, like waits and variables."
            }
            Repeater {
                model: importDialog.caps
                delegate: Row {
                    required property var modelData
                    spacing: 8
                    LucideIcon {
                        anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16
                        name: modelData.risky ? "triangle-alert" : "check"; color: modelData.risky ? Theme.warning : Theme.textDim
                    }
                    Text { text: modelData.label; color: modelData.risky ? Theme.warning : Theme.text; font.pixelSize: 13 }
                }
            }
            Text {
                visible: importDialog.caps.some(c => c.risky)
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: "Only import macros that run commands, go online or write files from people you trust."
            }
            Text {
                visible: importDialog.customSettings.length > 0; topPadding: 6
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: "It also comes with these settings. Untick any you don't want:"
            }
            Repeater {
                model: importDialog.customSettings
                delegate: BwCheckBox {
                    required property var modelData
                    text: modelData.label; checked: modelData.enabled
                    onToggled: root.keepSettings[modelData.key] = checked
                }
            }
        }
        onAccepted: root.invoke("confirm_import_macro", { keepSettings: root.keepSettings })
        onRejected: root.invoke("cancel_import_macro")
    }

    BwDialog {
        id: pathDialog
        property string mode: "import"
        title: mode === "import" ? "Import macro" : "Export macro"
        standardButtons: Dialog.Ok | Dialog.Cancel
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
