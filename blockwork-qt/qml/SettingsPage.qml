import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke
    property var translator: null
    function capturingBinding(index){return appState.combo_capture&&appState.combo_capture.kind==="Binding"&&appState.combo_capture.binding_index===index}
    function capturingAction(action){return appState.combo_capture&&appState.combo_capture.kind==="Named"&&JSON.stringify(appState.combo_capture.action)===JSON.stringify(action)}

    property int tab: 0
    // The newly picked tab's page fades in.
    onTabChanged: if (Motion.enabled) tabFade.restart()
    NumberAnimation { id: tabFade; target: pane; property: "opacity"; from: 0; to: 1; duration: Motion.normal; easing.type: Easing.OutCubic }
    readonly property var tabs: [
        { name: qsTr("General"), icon: "settings" }, { name: qsTr("Hotkeys"), icon: "keyboard" },
        { name: qsTr("Editor"), icon: "blocks" }, { name: qsTr("Integrations"), icon: "terminal" }
    ]

    component SettingsRow: Item {
        id: row
        property string title: ""
        property string detail: ""
        property bool first: false
        default property alias controls: ctl.data
        Layout.fillWidth: true
        implicitHeight: Math.max(textCol.implicitHeight, ctl.implicitHeight) + 26
        data: [
            Rectangle { anchors.top: parent.top; anchors.left: parent.left; anchors.right: parent.right; height: 1; color: "#3a3b3f"; visible: !row.first },
            Column {
                id: textCol
                anchors.left: parent.left; anchors.right: ctl.left; anchors.rightMargin: 16; anchors.verticalCenter: parent.verticalCenter; spacing: 2
                Text { width: parent.width; text: row.title; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold; elide: Text.ElideRight }
                Text { width: parent.width; visible: row.detail !== ""; text: row.detail; textFormat: Text.PlainText; color: "#9fa0a6"; font.pixelSize: 12; wrapMode: Text.WordWrap }
            },
            Row { id: ctl; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; spacing: 8 }
        ]
    }

    component SubHeading: Text {
        Layout.topMargin: 6; Layout.bottomMargin: 2
        color: "#a7a8ae"; font.pixelSize: 12; font.weight: Font.Bold; font.letterSpacing: 1.2
    }

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 52; color: "#292a2d"; border.color: "#38393d"
            TitleBarDrag { anchors.fill: parent; visible: Qt.platform.os !== "osx" }
            RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                Image {
                    visible: Qt.platform.os !== "osx"
                    source: "qrc:/icons/blockwork.png"; sourceSize: Qt.size(40, 40)
                    Layout.preferredWidth: 20; Layout.preferredHeight: 20; Layout.rightMargin: 2
                    fillMode: Image.PreserveAspectFit; smooth: true; mipmap: true
                }
                BwButton { iconName: "arrow-left"; text: qsTr("Back"); implicitHeight: 34; onClicked: root.invoke("close_settings") }
                Text { text: qsTr("Settings"); color: "#e7e7e8"; font.pixelSize: 17; font.weight: Font.Bold }
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
                            Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
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
                            Text { text: qsTr("General"); color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SettingsRow { first: true; title: qsTr("Close to tray"); detail: qsTr("Keep hotkeys and schedules active after closing the editor.")
                                BwSwitch { checked: appState.close_to_tray; onToggled: checked => root.invoke("set_close_to_tray", { enabled: checked }) }
                            }
                            SettingsRow { title: qsTr("Start at system startup"); detail: qsTr("Launch Blockwork in the background when you log in so hotkeys work right away.")
                                BwSwitch { checked: appState.start_at_startup === true; onToggled: checked => root.invoke("set_start_at_startup", { enabled: checked }) }
                            }
                            SettingsRow { title: qsTr("Language"); detail: qsTr("The language of the editor and macro dialogs. Run log messages stay in English.")
                                BwComboBox {
                                    id: languageBox
                                    // Every built-in translation, in its own name so it's findable in any language.
                                    readonly property var languages: [{ id: "", text: qsTr("System default") }, { id: "en", text: "English" }]
                                        .concat((root.translator ? root.translator.languages() : []).filter(c => c !== "en")
                                            .map(c => ({ id: c, text: root.languageName(c) })))
                                    model: languages; textRole: "text"; implicitWidth: 210
                                    currentIndex: Math.max(0, languages.findIndex(l => l.id === (appState.language || "")))
                                    onActivated: index => root.invoke("set_language", { language: languages[index].id })
                                    // Text follows its own direction by default, which right-aligns
                                    // Arabic and Hebrew names; keep every name lined up on the left.
                                    contentItem: Text { text: languageBox.displayText; color: languageBox.enabled ? Theme.text : Theme.textDim; font: languageBox.font; verticalAlignment: Text.AlignVCenter; horizontalAlignment: Text.AlignLeft; elide: Text.ElideRight }
                                    delegate: ItemDelegate {
                                        required property var modelData
                                        required property int index
                                        width: ListView.view.width; implicitHeight: 32; highlighted: languageBox.highlightedIndex === index
                                        HoverHandler { cursorShape: Qt.PointingHandCursor }
                                        contentItem: Text { text: modelData.text; color: Theme.text; font: languageBox.font; verticalAlignment: Text.AlignVCenter; horizontalAlignment: Text.AlignLeft }
                                        background: Rectangle { radius: 4; color: parent.highlighted ? "#424348" : "transparent"; Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } } }
                                    }
                                }
                            }
                            SettingsRow { title: qsTr("Time format"); detail: qsTr("How times show in blocks and pickers.")
                                BwComboBox {
                                    readonly property var formats: [{ id: "", text: qsTr("System default") }, { id: "12h", text: qsTr("12-hour (9:30 PM)") }, { id: "24h", text: qsTr("24-hour (21:30)") }]
                                    model: formats; textRole: "text"; implicitWidth: 210
                                    currentIndex: Math.max(0, formats.findIndex(f => f.id === (appState.clock_format || "")))
                                    onActivated: index => root.invoke("set_clock_format", { format: formats[index].id })
                                }
                            }
                            SettingsRow { title: qsTr("Animation speed"); detail: qsTr("How long menus, dialogs, scrolling and other transitions take. Instant turns animations off.")
                                Column {
                                    spacing: 2
                                    BwSlider {
                                        id: animationSlider
                                        // Duration multipliers, Instant to Slow; Normal sits in the middle.
                                        readonly property var speeds: [0, 0.25, 0.5, 1, 1.5, 2, 3]
                                        width: 230; from: 0; to: speeds.length - 1; stepSize: 1; ticks: true
                                        Accessible.name: qsTr("Animation speed")
                                        value: {
                                            const s = appState.animation_speed ?? 1;
                                            let best = 0;
                                            speeds.forEach((v, i) => { if (Math.abs(v - s) < Math.abs(speeds[best] - s)) best = i; });
                                            return best;
                                        }
                                        onMoved: root.invoke("set_animation_speed", { speed: speeds[Math.round(value)] })
                                    }
                                    Item {
                                        width: animationSlider.width; height: 16
                                        Text { x: 0; text: qsTr("Instant", "animation speed"); color: "#9fa0a6"; font.pixelSize: 11 }
                                        Text { anchors.horizontalCenter: parent.horizontalCenter; text: qsTr("Normal", "animation speed"); color: "#9fa0a6"; font.pixelSize: 11 }
                                        Text { anchors.right: parent.right; text: qsTr("Slow", "animation speed"); color: "#9fa0a6"; font.pixelSize: 11 }
                                    }
                                }
                            }
                            SettingsRow { title: root.updateTitle(); detail: root.updateDetail()
                                BwButton {
                                    readonly property bool available: appState.update_check_state && appState.update_check_state.state === "UpdateAvailable"
                                    text: available ? qsTr("Install update") : qsTr("Check for updates"); primary: available
                                    onClicked: root.invoke(available ? "apply_update" : "check_for_updates")
                                }
                            }
                            SettingsRow { title: qsTr("Global variables")
                                detail: (appState.globals || []).length ? qsTr("Shared by every macro and kept after restarts. Set them with the \"set global\" block.")
                                    : qsTr("None yet. The \"set global\" block makes one: a value every macro can read, kept after restarts.")
                            }
                            Repeater {
                                model: appState.globals || []
                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 6; spacing: 10
                                    Text { text: modelData.name; color: "#e7e7e8"; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.maximumWidth: 220 }
                                    Text { text: modelData.value; color: "#9fa0a6"; font.pixelSize: 13; elide: Text.ElideRight; Layout.fillWidth: true }
                                    BwButton { iconName: "trash-2"; text: qsTr("Delete"); onClicked: root.invoke("delete_global", { name: modelData.name }) }
                                }
                            }
                            Rectangle { Layout.fillWidth: true; Layout.topMargin: 8; height: 1; color: "#3a3b3f" }
                            RowLayout { Layout.fillWidth: true; Layout.topMargin: 16; spacing: 8
                                BwButton { iconName: "play"; text: qsTr("Replay welcome tour"); onClicked: root.invoke("set_onboarding_done", { done: false }) }
                                Item { Layout.fillWidth: true }
                            }
                        }

                        // Hotkeys
                        ColumnLayout { Layout.fillWidth: true; spacing: 0; visible: root.tab === 1
                            Text { text: qsTr("Hotkeys"); color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SubHeading { text: qsTr("GLOBAL") }
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
                                        text: hotkeyRow.capturing ? qsTr("Press shortcut...") : (hotkeyRow.binding ? hotkeyRow.binding.combo_display : qsTr("Not set"))
                                        primary: hotkeyRow.capturing; implicitWidth: 150; implicitHeight: 36
                                        onClicked: if (!hotkeyRow.capturing) root.invoke("start_combo_capture", { action: hotkeyRow.modelData.action })
                                    }
                                    Item { width: 84; height: 36
                                        BwButton { anchors.fill: parent; visible: !hotkeyRow.capturing && !hotkeyRow.atDefault; text: qsTr("Default"); implicitHeight: 36
                                            onClicked: root.invoke("reset_hotkey_to_default", { action: hotkeyRow.modelData.action }) }
                                    }
                                    Item { width: 42; height: 36
                                        BwButton { anchors.fill: parent; visible: !hotkeyRow.capturing && !!hotkeyRow.binding; iconName: "x"; text: ""; danger: true; implicitHeight: 36
                                            onClicked: root.invoke("clear_named_hotkey", { action: hotkeyRow.modelData.action }) }
                                    }
                                }
                            }
                            SubHeading { text: qsTr("PER MACRO"); Layout.topMargin: 24 }
                            Repeater {
                                model: (appState.hotkey_bindings || []).filter(b => b.action && b.action.type === "RunSpecificMacro")
                                delegate: SettingsRow {
                                    id: macroRow
                                    required property var modelData
                                    required property int index
                                    first: index === 0
                                    title: modelData.macro_name || qsTr("Macro")
                                    readonly property bool capturing: root.capturingBinding(modelData.binding_index)
                                    BwButton {
                                        text: macroRow.capturing ? qsTr("Press shortcut...") : macroRow.modelData.combo_display
                                        primary: macroRow.capturing; implicitWidth: 150; implicitHeight: 36
                                        onClicked: if (!macroRow.capturing) root.invoke("start_binding_combo_capture", { index: macroRow.modelData.binding_index })
                                    }
                                    Item { width: 84; height: 36 }
                                    BwButton { iconName: "x"; text: ""; danger: true; implicitWidth: 42; implicitHeight: 36
                                        onClicked: root.invoke("remove_hotkey_binding", { index: macroRow.modelData.binding_index }) }
                                }
                            }
                            RowLayout { Layout.fillWidth: true; Layout.topMargin: 14; spacing: 8
                                Text { text: qsTr("Add hotkey"); color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                                Item { Layout.fillWidth: true }
                                BwComboBox { id: macroBox; model: appState.macro_names || []; implicitWidth: 210; onActivated: index => root.invoke("set_pending_macro_idx", { index: index }) }
                                BwButton { text: appState.combo_capture&&appState.combo_capture.kind==="Pending"?qsTr("Press shortcut..."):qsTr("Set combo"); primary:appState.combo_capture&&appState.combo_capture.kind==="Pending"; onClicked: root.invoke("start_pending_combo_capture") }
                                BwButton { iconName: "plus"; text: qsTr("Add"); onClicked: root.invoke("add_macro_hotkey") }
                            }
                        }

                        // Editor
                        ColumnLayout { Layout.fillWidth: true; spacing: 0; visible: root.tab === 2
                            Text { text: qsTr("Editor"); color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SettingsRow { first: true; title: qsTr("Block palette layout"); detail: qsTr("How the block sidebar is organized in the editor.")
                                BwComboBox {
                                    id: paletteLayoutBox
                                    readonly property var layouts: [
                                        { id: "rail", text: qsTr("Category rail") }, { id: "chips", text: qsTr("Chip strip") }, { id: "accordion", text: qsTr("Accordion") },
                                        { id: "flyout", text: qsTr("Rail with flyout") }, { id: "tiles", text: qsTr("Tile grid") }
                                    ]
                                    model: layouts; textRole: "text"; implicitWidth: 210
                                    currentIndex: Math.max(0, layouts.findIndex(l => l.id === (appState.palette_layout || "rail")))
                                    onActivated: index => root.invoke("set_palette_layout", { layout: layouts[index].id })
                                }
                            }
                            SettingsRow { title: qsTr("Block library")
                                detail: qsTr("Where My Blocks > Browse finds shared blocks: the address of each library's index.json. Each pack is checked against the checksum its library lists before it's used.")
                            }
                            Repeater {
                                model: appState.block_library ? appState.block_library.sources : []
                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 6; spacing: 10
                                    LucideIcon { name: modelData.error ? "triangle-alert" : "book-open"; width: 14; height: 14; color: modelData.error ? Theme.warning : "#9fa0a6" }
                                    ColumnLayout { Layout.fillWidth: true; spacing: 1
                                        Text { Layout.fillWidth: true; text: modelData.name || modelData.url; color: "#e7e7e8"; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                        Text {
                                            Layout.fillWidth: true; color: modelData.error ? Theme.warning : "#9fa0a6"; font.pixelSize: 11; elide: Text.ElideMiddle
                                            text: modelData.error ? modelData.error : modelData.name ? qsTr("%n pack(s)", "", modelData.packs) + " - " + modelData.url : modelData.url
                                        }
                                    }
                                    BwButton { iconName: "trash-2"; text: qsTr("Remove"); onClicked: root.invoke("remove_block_library_source", { url: modelData.url }) }
                                }
                            }
                            RowLayout { Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 14; spacing: 8
                                BwTextField { id: librarySource; Layout.fillWidth: true; placeholderText: "https://example.com/blocks/index.json" }
                                BwButton {
                                    iconName: "plus"; text: qsTr("Add library"); enabled: librarySource.text.trim() !== ""
                                    onClicked: { root.invoke("add_block_library_source", { url: librarySource.text.trim() }); librarySource.text = ""; }
                                }
                                BwButton { iconName: "refresh-cw"; text: qsTr("Refresh"); onClicked: root.invoke("refresh_block_library") }
                            }
                        }

                        // Integrations
                        ColumnLayout { Layout.fillWidth: true; spacing: 0; visible: root.tab === 3
                            Text { text: qsTr("Integrations"); color: "#e7e7e8"; font.pixelSize: 20; font.weight: Font.Bold; Layout.bottomMargin: 10 }
                            SettingsRow { first: true
                                title: appState.ipc_active_port === null ? qsTr("TCP server stopped") : qsTr("TCP server listening on port %1").arg(appState.ipc_active_port)
                                detail: qsTr("Allow local integrations to run Blockwork macros.")
                                BwTextField { id: port; text: appState.ipc_port_text || "47821"; validator: IntValidator { bottom: 1; top: 65535 } implicitWidth: 100; onEditingFinished: root.invoke("set_ipc_port_text", { text: text }) }
                                BwButton { text: appState.ipc_active_port === null ? qsTr("Start") : qsTr("Stop"); primary: appState.ipc_active_port === null; onClicked: root.invoke(appState.ipc_active_port === null ? "start_ipc_server" : "stop_ipc_server") }
                            }
                            SettingsRow { title: qsTr("Start automatically"); detail: qsTr("Start the TCP server whenever Blockwork launches.")
                                BwSwitch { Accessible.name: qsTr("Start TCP server automatically"); checked: appState.ipc_auto_start; onToggled: checked => root.invoke("set_ipc_auto_start", { enabled: checked }) }
                            }
                            SettingsRow { title: qsTr("Secrets")
                                detail: qsTr("Passwords and API keys macros can use with the \"secret\" block. They're kept in your system keyring, never in macro files, and show as dots in the run log.")
                            }
                            Repeater {
                                model: appState.secret_names || []
                                delegate: RowLayout {
                                    required property string modelData
                                    Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 6; spacing: 10
                                    LucideIcon { name: "lock"; width: 14; height: 14; color: "#9fa0a6" }
                                    Text { text: modelData; color: "#e7e7e8"; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.fillWidth: true }
                                    BwButton { iconName: "pencil"; text: qsTr("Change"); onClicked: secretDialog.openFor(modelData) }
                                    BwButton { iconName: "trash-2"; text: qsTr("Delete"); onClicked: root.invoke("delete_secret", { name: modelData }) }
                                }
                            }
                            RowLayout { Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 14; spacing: 8
                                BwButton { iconName: "plus"; text: qsTr("Add secret..."); onClicked: secretDialog.openFor("") }
                                Item { Layout.fillWidth: true }
                            }
                            SettingsRow {
                                readonly property var hook: appState.webhook || ({ enabled: false, port: 8787, token: "", error: "" })
                                title: !hook.enabled ? qsTr("Webhook server off") : hook.error ? qsTr("Webhook server not running") : qsTr("Webhook server listening on port %1").arg(hook.port)
                                detail: hook.error || qsTr("Lets other programs and services start macros with a web request to a \"when webhook is called\" block. Only reachable from this computer.")
                                BwTextField {
                                    id: hookPort; implicitWidth: 100
                                    text: String(appState.webhook ? appState.webhook.port : 8787); validator: IntValidator { bottom: 1; top: 65535 }
                                    onEditingFinished: root.invoke("set_webhook", { enabled: !!appState.webhook && appState.webhook.enabled, port: Number(text) || 8787 })
                                }
                                BwSwitch {
                                    Accessible.name: qsTr("On"); checked: !!appState.webhook && appState.webhook.enabled
                                    onToggled: checked => root.invoke("set_webhook", { enabled: checked, port: Number(hookPort.text) || 8787 })
                                }
                            }
                            ColumnLayout {
                                id: hookInfo
                                readonly property var hook: appState.webhook || ({})
                                visible: !!hook.enabled && !!hook.token
                                Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 14; spacing: 8
                                readonly property string example: "curl -X POST -H 'X-Blockwork-Token: " + hook.token + "' -d 'hello' http://127.0.0.1:" + hook.port + "/hello"
                                Text {
                                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: "#9fa0a6"; font.pixelSize: 12
                                    text: qsTr("Send the token in an X-Blockwork-Token header (or ?token= in the address). For a block listening on \"hello\":")
                                }
                                Rectangle {
                                    Layout.fillWidth: true; implicitHeight: exampleText.implicitHeight + 16; radius: 6; color: Theme.field; border.color: Theme.borderSoft
                                    Text { id: exampleText; anchors.fill: parent; anchors.margins: 8; text: hookInfo.example; wrapMode: Text.WrapAnywhere; color: "#e7e7e8"; font.family: "monospace"; font.pixelSize: 12 }
                                }
                                RowLayout { spacing: 8
                                    BwButton { iconName: "copy"; text: qsTr("Copy example"); onClicked: root.copyText(hookInfo.example) }
                                    BwButton { iconName: "copy"; text: qsTr("Copy token"); onClicked: root.copyText(hookInfo.hook.token) }
                                    BwButton { iconName: "refresh-cw"; text: qsTr("New token"); onClicked: root.invoke("new_webhook_token") }
                                }
                            }
                            SettingsRow { title: qsTr("Command line")
                                detail: qsTr("Scripts and launchers can run macros by name: \"blockwork run <name>\" (add --wait to return when it ends), \"blockwork stop <name>\" and \"blockwork list\".")
                            }
                        }
                    }
                }
            }
        }
    }

    TextEdit { id: clipboardHelper; visible: false }
    // "pt_BR" -> "Português (Brasil)". Codes Qt doesn't know (like "pirate") get a name here.
    function languageName(code) {
        if (code === "pirate") return "English (Pirate)"
        const locale = Qt.locale(code)
        if (locale.name === "C") return code
        let name = locale.nativeLanguageName
        if (code.indexOf("_") > 0) name += " (" + locale.nativeTerritoryName + ")"
        return name.charAt(0).toUpperCase() + name.slice(1)
    }
    function copyText(text) { clipboardHelper.text = text; clipboardHelper.selectAll(); clipboardHelper.copy(); }

    function actionLabel(action) {
        if (!action) return qsTr("Hotkey");
        const labels = { RunMacro: qsTr("Run Macro"), StopLoop: qsTr("Stop Macro"), EmergencyStop: qsTr("Emergency Stop (pauses triggers)"), NextMacro: qsTr("Next Macro"), PrevMacro: qsTr("Previous Macro"), StartRecordingImmediate: qsTr("Start Recording (immediate)"), StopRecording: qsTr("Stop Recording"), Undo: qsTr("Undo"), Redo: qsTr("Redo") };
        return labels[action.type] || action.type;
    }
    function updateTitle() {
        const u = appState.update_check_state || { state: "Idle" };
        if (u.state === "Checking") return qsTr("Checking for updates...");
        if (u.state === "UpdateAvailable") return qsTr("Blockwork %1 is available").arg(u.version);
        if (u.state === "UpToDate") return qsTr("Blockwork is up to date");
        if (u.state === "Error") return qsTr("Update check failed");
        return qsTr("Software updates");
    }
    function updateDetail() {
        const u = appState.update_check_state || {};
        return u.error || qsTr("Check for a new release of Blockwork.");
    }

    AppDialog {
        id: secretDialog
        property string editing: ""
        title: editing ? qsTr("Change \"%1\"").arg(editing) : qsTr("Add a secret")
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openFor(name) {
            editing = name; secretName.text = name; secretValue.text = ""; open();
            if (name) secretValue.forceActiveFocus(); else secretName.forceActiveFocus();
        }
        Column { width: 380; spacing: 8
            Text { text: qsTr("Name"); color: "#e7e7e8" }
            BwTextField { id: secretName; width: parent.width; readOnly: !!secretDialog.editing; placeholderText: qsTr("api token") }
            Text { text: qsTr("Value"); color: "#e7e7e8" }
            BwTextField { id: secretValue; width: parent.width; echoMode: TextInput.Password; placeholderText: qsTr("Paste it here"); onAccepted: secretDialog.accept() }
        }
        onAccepted: {
            if (secretName.text.trim() && secretValue.text) root.invoke("set_secret", { name: secretName.text.trim(), value: secretValue.text });
            secretValue.text = "";
        }
        onRejected: secretValue.text = ""
    }
}
