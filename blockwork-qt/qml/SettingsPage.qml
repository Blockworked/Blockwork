import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke
    property var appBridge: null
    property var translator: null
    function capturingBinding(index){return !!appState.combo_capture&&appState.combo_capture.kind==="Binding"&&appState.combo_capture.binding_index===index}
    function capturingAction(action){return !!appState.combo_capture&&appState.combo_capture.kind==="Named"&&JSON.stringify(appState.combo_capture.action)===JSON.stringify(action)}

    Rectangle { anchors.fill: parent; color: Theme.window }

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
        readonly property bool stacked: width < ctl.implicitWidth + 240
        implicitHeight: (stacked ? textCol.implicitHeight + (ctl.implicitHeight > 0 ? ctl.implicitHeight + 12 : 0)
                                : Math.max(textCol.implicitHeight, ctl.implicitHeight)) + 28
        data: [
            Rectangle { anchors.top: parent.top; anchors.left: parent.left; anchors.right: parent.right; height: 1; color: Theme.borderSoft; visible: !row.first },
            Column {
                id: textCol
                x: 0; y: row.stacked ? 14 : (row.height - height) / 2
                width: row.stacked ? row.width : Math.max(0, row.width - ctl.width - 16); spacing: 4
                Text { width: parent.width; text: row.title; color: Theme.text; font.pixelSize: 14; font.weight: Font.DemiBold; wrapMode: Text.WordWrap }
                Text { width: parent.width; visible: row.detail !== ""; text: row.detail; textFormat: Text.PlainText; color: Theme.textDim; font.pixelSize: 12; wrapMode: Text.WordWrap }
            },
            Row {
                id: ctl
                x: row.stacked ? 0 : row.width - width
                y: row.stacked ? textCol.y + textCol.height + 12 : (row.height - height) / 2
                spacing: 8
            }
        ]
    }

    component SettingsSection: Rectangle {
        id: section
        property string title: ""
        default property alias rows: sectionContent.data
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        Layout.minimumWidth: 0
        Layout.alignment: Qt.AlignTop
        implicitHeight: sectionContent.implicitHeight + 32
        radius: 10; color: Theme.panel; border.color: Theme.borderSoft
        ColumnLayout {
            id: sectionContent
            x: 16; y: 16; width: parent.width - 32; spacing: 0
            Text {
                visible: section.title !== ""
                Layout.fillWidth: true; Layout.bottomMargin: 8
                text: section.title; color: Theme.text; font.pixelSize: 16; font.weight: Font.Bold
                wrapMode: Text.WordWrap
            }
        }
    }

    component SettingsGrid: GridLayout {
        Layout.fillWidth: true
        columns: width >= 1120 ? 2 : 1
        columnSpacing: 20; rowSpacing: 20
    }

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 52; color: Theme.field; border.color: Theme.borderSoft
            TitleBarDrag { anchors.fill: parent; visible: Qt.platform.os !== "osx" }
            RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                Image {
                    visible: Qt.platform.os !== "osx"
                    source: "qrc:/icons/blockwork.png"; sourceSize: Qt.size(40, 40)
                    Layout.preferredWidth: 20; Layout.preferredHeight: 20; Layout.rightMargin: 2
                    fillMode: Image.PreserveAspectFit; smooth: true; mipmap: true
                }
                BwButton { iconName: "arrow-left"; text: qsTr("Back"); implicitHeight: 34; onClicked: root.invoke("close_settings") }
                Text { text: qsTr("Settings"); color: Theme.text; font.pixelSize: 17; font.weight: Font.Bold }
                Item { Layout.fillWidth: true }
                WindowControls { visible: Qt.platform.os !== "osx" }
            }
        }

        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 0

            Rectangle {
                Layout.preferredWidth: 190; Layout.fillHeight: true
                color: Theme.panel
                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.borderSoft }
                Column {
                    anchors.fill: parent; anchors.margins: 10; spacing: 2
                    Repeater {
                        model: root.tabs
                        delegate: Rectangle {
                            id: navItem
                            required property var modelData
                            required property int index
                            width: parent.width; height: 38; radius: 8
                            color: root.tab === index ? Theme.selection : (navMouse.containsMouse ? Theme.hover : "transparent")
                            Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                            Row { anchors.left: parent.left; anchors.leftMargin: 12; anchors.verticalCenter: parent.verticalCenter; spacing: 10
                                LucideIcon { name: navItem.modelData.icon; width: 16; height: 16; anchors.verticalCenter: parent.verticalCenter
                                    color: root.tab === navItem.index ? Theme.accent : Theme.textDim }
                                Text { text: navItem.modelData.name; font.pixelSize: 14; anchors.verticalCenter: parent.verticalCenter
                                    font.weight: root.tab === navItem.index ? Font.DemiBold : Font.Normal
                                    color: root.tab === navItem.index ? Theme.accent : Theme.text }
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
                        x: 28; y: 24; width: Math.max(0, parent.width - 56); spacing: 16
                        Text { text: root.tabs[root.tab].name; color: Theme.text; font.pixelSize: 22; font.weight: Font.Bold }

                        // General
                        SettingsGrid { visible: root.tab === 0
                            SettingsSection { title: qsTr("Preferences")
                                SettingsRow { first: true; title: qsTr("Appearance"); detail: qsTr("Choose a light or dark theme, or follow your system setting.")
                                    BwComboBox {
                                        readonly property var schemes: [{ id: "", text: qsTr("System default") }, { id: "light", text: qsTr("Light") }, { id: "dark", text: qsTr("Dark") }]
                                        model: schemes; textRole: "text"; implicitWidth: 210
                                        currentIndex: Math.max(0, schemes.findIndex(s => s.id === (appState.color_scheme || "")))
                                        onActivated: index => root.invoke("set_color_scheme", { scheme: schemes[index].id })
                                    }
                                }
                                SettingsRow { title: qsTr("Close to tray"); detail: qsTr("Keep hotkeys and schedules active after closing the editor.")
                                    BwSwitch { checked: appState.close_to_tray; onToggled: checked => root.invoke("set_close_to_tray", { enabled: checked }) }
                                }
                                SettingsRow { title: qsTr("Start at system startup"); detail: qsTr("Launch Blockwork in the background when you log in so hotkeys work right away.")
                                    BwSwitch { checked: appState.start_at_startup === true; onToggled: checked => root.invoke("set_start_at_startup", { enabled: checked }) }
                                }
                                SettingsRow { title: qsTr("Language"); detail: qsTr("The language of the editor and macro dialogs. Run log messages stay in English.")
                                    BwButton {
                                        objectName: "chooseLanguage"
                                        implicitWidth: 210
                                        text: appState.language ? root.languageName(appState.language) : qsTr("System default")
                                        Accessible.name: qsTr("Choose language")
                                        onClicked: languageDialog.open()
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
                                            Text { x: 0; text: qsTr("Instant", "animation speed"); color: Theme.textDim; font.pixelSize: 11 }
                                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: qsTr("Normal", "animation speed"); color: Theme.textDim; font.pixelSize: 11 }
                                            Text { anchors.right: parent.right; text: qsTr("Slow", "animation speed"); color: Theme.textDim; font.pixelSize: 11 }
                                        }
                                    }
                                }
                            }
                            SettingsSection { title: qsTr("App and data")
                                SettingsRow { first: true; title: root.updateTitle(); detail: root.updateDetail()
                                    BwButton {
                                        readonly property bool available: !!appState.update_check_state && appState.update_check_state.state === "UpdateAvailable"
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
                                        Text { text: modelData.name; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.maximumWidth: 220 }
                                        Text { text: modelData.value; color: Theme.textDim; font.pixelSize: 13; elide: Text.ElideRight; Layout.fillWidth: true }
                                        BwButton { iconName: "trash-2"; text: qsTr("Delete"); onClicked: root.invoke("delete_global", { name: modelData.name }) }
                                    }
                                }
                                Rectangle { Layout.fillWidth: true; Layout.topMargin: 8; height: 1; color: Theme.borderSoft }
                                RowLayout { Layout.fillWidth: true; Layout.topMargin: 16; spacing: 8
                                    BwButton { iconName: "play"; text: qsTr("Replay welcome tour"); onClicked: root.invoke("set_onboarding_done", { done: false }) }
                                    Item { Layout.fillWidth: true }
                                }
                            }
                        }

                        // Hotkeys
                        SettingsGrid { visible: root.tab === 1
                            SettingsSection { title: qsTr("Global shortcuts"); objectName: "globalShortcuts"
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
                                            primary: hotkeyRow.capturing; implicitWidth: 190; implicitHeight: 36
                                            onClicked: if (!hotkeyRow.capturing) root.invoke("start_combo_capture", { action: hotkeyRow.modelData.action })
                                        }
                                        Item { width: 36; height: 36
                                            BwButton { anchors.fill: parent; visible: !hotkeyRow.capturing && !hotkeyRow.atDefault; iconName: "rotate-ccw"; implicitHeight: 36
                                                Accessible.name: qsTr("Reset %1 to default").arg(hotkeyRow.title)
                                                ToolTip.visible: hovered; ToolTip.text: qsTr("Reset to default")
                                                onClicked: root.invoke("reset_hotkey_to_default", { action: hotkeyRow.modelData.action }) }
                                        }
                                        Item { width: 36; height: 36
                                            BwButton { anchors.fill: parent; visible: !hotkeyRow.capturing && !!hotkeyRow.binding; iconName: "x"; text: ""; flat: true; implicitHeight: 36
                                                Accessible.name: qsTr("Clear %1 shortcut").arg(hotkeyRow.title)
                                                ToolTip.visible: hovered; ToolTip.text: qsTr("Clear shortcut")
                                                onClicked: root.invoke("clear_named_hotkey", { action: hotkeyRow.modelData.action }) }
                                        }
                                    }
                                }
                            }
                            SettingsSection { title: qsTr("Macro shortcuts"); objectName: "macroShortcuts"
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
                                            primary: macroRow.capturing; implicitWidth: 190; implicitHeight: 36
                                            onClicked: if (!macroRow.capturing) root.invoke("start_binding_combo_capture", { index: macroRow.modelData.binding_index })
                                        }
                                        BwButton { iconName: "x"; text: ""; flat: true; implicitWidth: 36; implicitHeight: 36
                                            Accessible.name: qsTr("Remove shortcut for %1").arg(macroRow.title)
                                            ToolTip.visible: hovered; ToolTip.text: qsTr("Remove shortcut")
                                            onClicked: root.invoke("remove_hotkey_binding", { index: macroRow.modelData.binding_index }) }
                                    }
                                }
                                Text { text: qsTr("Add a macro shortcut"); color: Theme.textDim; font.pixelSize: 12; Layout.topMargin: 18; Layout.bottomMargin: 8 }
                                Flow { Layout.fillWidth: true; spacing: 8
                                    BwComboBox { id: macroBox; model: appState.macro_names || []; width: Math.min(210, parent.width); onActivated: index => root.invoke("set_pending_macro_idx", { index: index }) }
                                    BwButton { text: appState.combo_capture&&appState.combo_capture.kind==="Pending"?qsTr("Press shortcut..."):qsTr("Set combo"); primary:!!appState.combo_capture&&appState.combo_capture.kind==="Pending"; onClicked: root.invoke("start_pending_combo_capture") }
                                    BwButton { iconName: "plus"; text: qsTr("Add"); onClicked: root.invoke("add_macro_hotkey") }
                                }
                            }
                        }

                        // Editor
                        SettingsGrid { visible: root.tab === 2
                            SettingsSection {
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
                            }
                            SettingsSection {
                                SettingsRow { first: true; title: qsTr("Block library")
                                    detail: qsTr("Where My Blocks > Browse finds shared blocks: the address of each library's index.json. Each pack is checked against the checksum its library lists before it's used.")
                                }
                                Repeater {
                                    model: appState.block_library ? appState.block_library.sources : []
                                    delegate: RowLayout {
                                        required property var modelData
                                        Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 6; spacing: 10
                                        LucideIcon { name: modelData.error ? "triangle-alert" : "book-open"; width: 14; height: 14; color: modelData.error ? Theme.warning : Theme.textDim }
                                        ColumnLayout { Layout.fillWidth: true; spacing: 1
                                            Text { Layout.fillWidth: true; text: modelData.name || modelData.url; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                            Text {
                                                Layout.fillWidth: true; color: modelData.error ? Theme.warning : Theme.textDim; font.pixelSize: 11; elide: Text.ElideMiddle
                                                text: modelData.error ? modelData.error : modelData.name ? qsTr("%n pack(s)", "", modelData.packs) + " - " + modelData.url : modelData.url
                                            }
                                        }
                                        Text { visible: modelData.builtin; text: qsTr("Built-in"); color: Theme.textDim; font.pixelSize: 12; Layout.rightMargin: 4 }
                                        BwButton { visible: !modelData.builtin; iconName: "trash-2"; text: qsTr("Remove"); onClicked: root.invoke("remove_block_library_source", { url: modelData.url }) }
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
                        }

                        // Integrations
                        SettingsGrid { visible: root.tab === 3
                            SettingsSection {
                                SettingsRow { first: true
                                    title: appState.ipc_active_port === null ? qsTr("TCP server stopped") : qsTr("TCP server listening on port %1").arg(appState.ipc_active_port)
                                    detail: qsTr("Allow local integrations to run Blockwork macros.")
                                    BwTextField { id: port; text: appState.ipc_port_text || "47821"; validator: IntValidator { bottom: 1; top: 65535 } implicitWidth: 100; onEditingFinished: root.invoke("set_ipc_port_text", { text: text }) }
                                    BwButton { text: appState.ipc_active_port === null ? qsTr("Start") : qsTr("Stop"); primary: appState.ipc_active_port === null; onClicked: root.invoke(appState.ipc_active_port === null ? "start_ipc_server" : "stop_ipc_server") }
                                }
                                SettingsRow { title: qsTr("Start automatically"); detail: qsTr("Start the TCP server whenever Blockwork launches.")
                                    BwSwitch { Accessible.name: qsTr("Start TCP server automatically"); checked: appState.ipc_auto_start; onToggled: checked => root.invoke("set_ipc_auto_start", { enabled: checked }) }
                                }
                            }
                            SettingsSection {
                                SettingsRow { first: true; title: qsTr("Secrets")
                                    detail: qsTr("Passwords and API keys macros can use with the \"secret\" block. They're kept in your system keyring, never in macro files, and show as dots in the run log.")
                                }
                                Repeater {
                                    model: appState.secret_names || []
                                    delegate: RowLayout {
                                        required property string modelData
                                        Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 6; spacing: 10
                                        LucideIcon { name: "lock"; width: 14; height: 14; color: Theme.textDim }
                                        Text { text: modelData; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight; Layout.fillWidth: true }
                                        BwButton { iconName: "pencil"; text: qsTr("Change"); onClicked: secretDialog.openFor(modelData) }
                                        BwButton { iconName: "trash-2"; text: qsTr("Delete"); onClicked: root.invoke("delete_secret", { name: modelData }) }
                                    }
                                }
                                RowLayout { Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 14; spacing: 8
                                    BwButton { iconName: "plus"; text: qsTr("Add secret..."); onClicked: secretDialog.openFor("") }
                                    Item { Layout.fillWidth: true }
                                }
                            }
                            SettingsSection {
                                SettingsRow {
                                    first: true
                                    id: hookRow
                                    readonly property var hook: appState.webhook || ({ enabled: false, port: 8787, error: "" })
                                    title: qsTr("Webhook server")
                                    detail: hook.error || qsTr("Lets other programs start macros with a web request to a \"when webhook is called\" block. Only reachable from this computer.")
                                    Row {
                                        anchors.verticalCenter: parent.verticalCenter; spacing: 6
                                        readonly property color tint: !hookRow.hook.enabled ? Theme.textDim : hookRow.hook.error ? Theme.warning : Theme.running
                                        Rectangle { width: 8; height: 8; radius: 4; color: parent.tint; anchors.verticalCenter: parent.verticalCenter }
                                        Text {
                                            anchors.verticalCenter: parent.verticalCenter; color: parent.tint; font.pixelSize: 13
                                            text: !hookRow.hook.enabled ? qsTr("Off") : hookRow.hook.error ? qsTr("Not running") : qsTr("Listening")
                                        }
                                    }
                                    BwTextField {
                                        id: hookPort; implicitWidth: 80; Accessible.name: qsTr("Webhook port")
                                        text: String(hookRow.hook.port); validator: IntValidator { bottom: 1; top: 65535 }
                                        onEditingFinished: root.invoke("set_webhook", { enabled: !!hookRow.hook.enabled, port: Number(text) || 8787 })
                                    }
                                    // The row's title names the switch, so its own label is clipped off.
                                    Item {
                                        anchors.verticalCenter: parent.verticalCenter; implicitWidth: 38; implicitHeight: 28; clip: true
                                        BwSwitch {
                                            Accessible.name: qsTr("Webhook server"); checked: !!hookRow.hook.enabled
                                            onToggled: checked => root.invoke("set_webhook", { enabled: checked, port: Number(hookPort.text) || 8787 })
                                        }
                                    }
                                }
                                // The token stays hidden: copying fetches it without showing it, and
                                // Show reveals it until Hide, 30 seconds pass or the window loses focus.
                                ColumnLayout {
                                    id: hookInfo
                                    visible: !!hookRow.hook.enabled
                                    Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 14; spacing: 8
                                    property string token: ""
                                    readonly property bool revealed: token !== ""
                                    property bool showExample: false
                                    property string copied: ""
                                    function example(token) { return "curl -X POST -H 'X-Blockwork-Token: " + token + "' -d 'hello' http://127.0.0.1:" + hookRow.hook.port + "/hello" }
                                    function hide() { token = ""; hideTimer.stop() }
                                    function copy(what, text) { root.copyText(text); copied = what; copiedTimer.restart() }
                                    onVisibleChanged: if (!visible) hide()
                                    Timer { id: hideTimer; interval: 30000; onTriggered: hookInfo.hide() }
                                    Timer { id: copiedTimer; interval: 1500; onTriggered: hookInfo.copied = "" }
                                    Connections {
                                        target: Qt.application
                                        function onStateChanged() { if (Qt.application.state !== Qt.ApplicationActive) hookInfo.hide() }
                                    }
                                    Connections { target: root; function onTabChanged() { hookInfo.hide() } }
                                    RowLayout { Layout.fillWidth: true; spacing: 8
                                        LucideIcon { name: "lock"; width: 14; height: 14; color: Theme.textDim }
                                        Text { text: qsTr("Token"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
                                        Text {
                                            Layout.fillWidth: true; Layout.leftMargin: 4; elide: Text.ElideRight
                                            text: hookInfo.revealed ? hookInfo.token : "\u2022".repeat(24)
                                            color: hookInfo.revealed ? Theme.text : Theme.textDim; font.family: "monospace"; font.pixelSize: 12
                                            Accessible.name: hookInfo.revealed ? hookInfo.token : qsTr("Hidden token")
                                        }
                                        BwButton {
                                            iconName: hookInfo.revealed ? "lock" : "eye"; text: hookInfo.revealed ? qsTr("Hide") : qsTr("Show")
                                            onClicked: hookInfo.revealed ? hookInfo.hide() : root.withWebhookToken(token => { hookInfo.token = token; hideTimer.restart() })
                                        }
                                        BwButton {
                                            iconName: hookInfo.copied === "token" ? "check" : "copy"; text: hookInfo.copied === "token" ? qsTr("Copied") : qsTr("Copy")
                                            onClicked: root.withWebhookToken(token => hookInfo.copy("token", token))
                                        }
                                        BwButton { iconName: "refresh-cw"; text: qsTr("New token"); onClicked: { hookInfo.hide(); root.invoke("new_webhook_token") } }
                                    }
                                    Item {
                                        implicitWidth: expander.implicitWidth; implicitHeight: expander.implicitHeight
                                        Row { id: expander; spacing: 6
                                            LucideIcon { name: hookInfo.showExample ? "chevron-down" : "chevron-right"; width: 14; height: 14; color: Theme.textDim; anchors.verticalCenter: parent.verticalCenter }
                                            Text { text: qsTr("How to call it"); color: Theme.textDim; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                                        }
                                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: hookInfo.showExample = !hookInfo.showExample }
                                    }
                                    ColumnLayout {
                                        visible: hookInfo.showExample; Layout.fillWidth: true; spacing: 8
                                        Text {
                                            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                                            text: qsTr("Send the token in an X-Blockwork-Token header (or ?token= in the address). For a block listening on \"hello\":")
                                        }
                                        RowLayout { Layout.fillWidth: true; spacing: 8
                                            Rectangle {
                                                Layout.fillWidth: true; implicitHeight: exampleText.implicitHeight + 16; radius: 6; color: Theme.field; border.color: Theme.borderSoft
                                                Text {
                                                    id: exampleText; anchors.fill: parent; anchors.margins: 8; wrapMode: Text.WrapAnywhere; color: Theme.text; font.family: "monospace"; font.pixelSize: 12
                                                    text: hookInfo.example(hookInfo.revealed ? hookInfo.token : "<token>")
                                                }
                                            }
                                            BwButton {
                                                iconName: hookInfo.copied === "example" ? "check" : "copy"; text: hookInfo.copied === "example" ? qsTr("Copied") : qsTr("Copy")
                                                onClicked: root.withWebhookToken(token => hookInfo.copy("example", hookInfo.example(token)))
                                            }
                                        }
                                    }
                                }
                            }
                            SettingsSection {
                                SettingsRow {
                                    first: true
                                    id: browserRow
                                    readonly property var bridge: appState.browser || ({ enabled: false, port: 8788, error: "", browsers: [] })
                                    readonly property int connections: (bridge.browsers || []).length
                                    title: qsTr("Browser extension")
                                    detail: bridge.error || (connections ? qsTr("Connected: %1").arg(bridge.browsers.join(", "))
                                        : qsTr("Lets macros click, type and read on web pages with the Browser blocks, by element instead of by picture. Install the Blockwork extension in Chrome, Edge or Firefox and paste the pairing key into it."))
                                    Row {
                                        anchors.verticalCenter: parent.verticalCenter; spacing: 6
                                        readonly property color tint: !browserRow.bridge.enabled ? Theme.textDim : browserRow.bridge.error ? Theme.warning
                                            : browserRow.connections ? Theme.running : Theme.textDim
                                        Rectangle { width: 8; height: 8; radius: 4; color: parent.tint; anchors.verticalCenter: parent.verticalCenter }
                                        Text {
                                            anchors.verticalCenter: parent.verticalCenter; color: parent.tint; font.pixelSize: 13
                                            text: !browserRow.bridge.enabled ? qsTr("Off") : browserRow.bridge.error ? qsTr("Not running")
                                                : browserRow.connections ? qsTr("Connected") : qsTr("Waiting for a browser")
                                        }
                                    }
                                    BwTextField {
                                        id: browserPort; implicitWidth: 80; Accessible.name: qsTr("Browser extension port")
                                        text: String(browserRow.bridge.port); validator: IntValidator { bottom: 1; top: 65535 }
                                        onEditingFinished: root.invoke("set_browser_bridge", { enabled: !!browserRow.bridge.enabled, port: Number(text) || 8788 })
                                    }
                                    Item {
                                        anchors.verticalCenter: parent.verticalCenter; implicitWidth: 38; implicitHeight: 28; clip: true
                                        BwSwitch {
                                            Accessible.name: qsTr("Browser extension"); checked: !!browserRow.bridge.enabled
                                            onToggled: checked => root.invoke("set_browser_bridge", { enabled: checked, port: Number(browserPort.text) || 8788 })
                                        }
                                    }
                                }
                                // The pairing key stays hidden like the webhook token.
                                ColumnLayout {
                                    id: browserInfo
                                    visible: !!browserRow.bridge.enabled
                                    Layout.fillWidth: true; Layout.leftMargin: 12; Layout.bottomMargin: 14; spacing: 8
                                    property string key: ""
                                    readonly property bool revealed: key !== ""
                                    property bool copied: false
                                    function hide() { key = ""; browserHideTimer.stop() }
                                    onVisibleChanged: if (!visible) hide()
                                    Timer { id: browserHideTimer; interval: 30000; onTriggered: browserInfo.hide() }
                                    Timer { id: browserCopiedTimer; interval: 1500; onTriggered: browserInfo.copied = false }
                                    Connections {
                                        target: Qt.application
                                        function onStateChanged() { if (Qt.application.state !== Qt.ApplicationActive) browserInfo.hide() }
                                    }
                                    Connections { target: root; function onTabChanged() { browserInfo.hide() } }
                                    RowLayout { Layout.fillWidth: true; spacing: 8
                                        LucideIcon { name: "lock"; width: 14; height: 14; color: Theme.textDim }
                                        Text { text: qsTr("Pairing key"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
                                        Text {
                                            Layout.fillWidth: true; Layout.leftMargin: 4; elide: Text.ElideRight
                                            text: browserInfo.revealed ? browserInfo.key : "\u2022".repeat(24)
                                            color: browserInfo.revealed ? Theme.text : Theme.textDim; font.family: "monospace"; font.pixelSize: 12
                                            Accessible.name: browserInfo.revealed ? browserInfo.key : qsTr("Hidden pairing key")
                                        }
                                        BwButton {
                                            iconName: browserInfo.revealed ? "lock" : "eye"; text: browserInfo.revealed ? qsTr("Hide") : qsTr("Show")
                                            onClicked: browserInfo.revealed ? browserInfo.hide() : root.withReply("browser_key", key => { browserInfo.key = key; browserHideTimer.restart() })
                                        }
                                        BwButton {
                                            iconName: browserInfo.copied ? "check" : "copy"; text: browserInfo.copied ? qsTr("Copied") : qsTr("Copy")
                                            onClicked: root.withReply("browser_key", key => { root.copyText(key); browserInfo.copied = true; browserCopiedTimer.restart() })
                                        }
                                        BwButton { iconName: "refresh-cw"; text: qsTr("New key"); onClicked: { browserInfo.hide(); root.invoke("new_browser_key") } }
                                    }
                                    Text {
                                        Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                                        text: qsTr("Open the extension's menu in your browser, choose Pairing and paste the key. A new key disconnects every browser until it gets the new one.")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    readonly property var languageDetails: root.translator ? JSON.parse(root.translator.languageDetails()) : []
    readonly property var availableLanguages: [{ id: "", text: qsTr("System default"), percent: -1 }].concat(
        languageDetails.map(entry => ({ id: entry.id, text: root.languageName(entry.id),
            percent: entry.total > 0 ? 100 * entry.translated / entry.total : 0 }))
        .sort((a, b) => a.text.localeCompare(b.text)))

    AppDialog {
        id: languageDialog
        objectName: "languageDialog"
        title: qsTr("Choose language")
        width: Math.min(560, root.width - 32)
        height: Math.min(620, root.height - 32)
        standardButtons: Dialog.Ok | Dialog.Cancel
        property string selectedLanguage: ""
        readonly property var filteredLanguages: root.availableLanguages.filter(entry =>
            entry.id === "" || entry.text.toLocaleLowerCase().includes(languageSearch.text.trim().toLocaleLowerCase())
            || entry.id.toLowerCase().includes(languageSearch.text.trim().toLowerCase()))
        onOpened: {
            selectedLanguage = root.appState.language || "";
            languageSearch.text = "";
            const index = filteredLanguages.findIndex(entry => entry.id === selectedLanguage);
            languageList.currentIndex = Math.max(0, index);
            languageList.positionViewAtIndex(languageList.currentIndex, ListView.Contain);
            languageList.forceActiveFocus();
        }
        onAccepted: root.invoke("set_language", { language: selectedLanguage })
        contentItem: ColumnLayout {
            spacing: 12
            Text {
                Layout.fillWidth: true
                text: qsTr("Untranslated text appears in English.")
                color: Theme.textDim; font.pixelSize: 12; wrapMode: Text.WordWrap
            }
            BwTextField {
                id: languageSearch
                objectName: "languageSearch"
                Layout.fillWidth: true
                placeholderText: qsTr("Search languages")
                Accessible.name: placeholderText
            }
            Rectangle {
                Layout.fillWidth: true; Layout.fillHeight: true
                color: Theme.field; radius: 6; border.color: Theme.borderSoft
                ColumnLayout {
                    anchors.fill: parent; anchors.margins: 1; spacing: 0
                    RowLayout {
                        Layout.fillWidth: true; Layout.margins: 12; spacing: 12
                        Text { Layout.fillWidth: true; text: qsTr("Language"); color: Theme.text; font.weight: Font.DemiBold }
                        Text { text: qsTr("Completeness"); color: Theme.textDim; font.pixelSize: 12 }
                    }
                    Rectangle { Layout.fillWidth: true; height: 1; color: Theme.borderSoft }
                    ListView {
                        id: languageList
                        objectName: "languageList"
                        Layout.fillWidth: true; Layout.fillHeight: true
                        clip: true; boundsBehavior: Flickable.StopAtBounds
                        model: languageDialog.filteredLanguages
                        ScrollBar.vertical: ScrollBar {}
                        keyNavigationEnabled: false
                        function moveSelection(delta) {
                            currentIndex = Math.max(0, Math.min(count - 1, currentIndex + delta));
                            languageDialog.selectedLanguage = model[currentIndex].id;
                            positionViewAtIndex(currentIndex, ListView.Contain);
                        }
                        Keys.onUpPressed: moveSelection(-1)
                        Keys.onDownPressed: moveSelection(1)
                        Keys.onReturnPressed: languageDialog.accept()
                        Keys.onEnterPressed: languageDialog.accept()
                        delegate: ItemDelegate {
                            id: languageRow
                            required property var modelData
                            required property int index
                            objectName: "language_" + modelData.id
                            width: languageList.width; height: 42
                            highlighted: languageDialog.selectedLanguage === modelData.id
                            Accessible.name: modelData.text + (modelData.percent >= 0 ? ", " + modelData.percent.toFixed(1) + "%" : "")
                            onClicked: { languageList.currentIndex = index; languageDialog.selectedLanguage = modelData.id; }
                            HoverHandler { cursorShape: Qt.PointingHandCursor }
                            background: Rectangle {
                                color: languageRow.highlighted ? Theme.selection : languageRow.hovered ? Theme.hover
                                    : languageRow.index % 2 ? Theme.panel : "transparent"
                            }
                            contentItem: RowLayout {
                                spacing: 12
                                Text {
                                    Layout.fillWidth: true; Layout.minimumWidth: 0
                                    text: languageRow.modelData.text; color: Theme.text
                                    horizontalAlignment: Text.AlignLeft; elide: Text.ElideRight
                                    font.weight: languageRow.highlighted ? Font.DemiBold : Font.Normal
                                }
                                Rectangle {
                                    visible: languageRow.modelData.percent >= 0 && languageList.width >= 360
                                    Layout.preferredWidth: 68; height: 4; radius: 2; color: Theme.borderSoft
                                    Rectangle {
                                        width: parent.width * Math.max(0, languageRow.modelData.percent) / 100
                                        height: parent.height; radius: 2; color: Theme.accent
                                    }
                                }
                                Text {
                                    Layout.preferredWidth: 54
                                    text: languageRow.modelData.percent < 0 ? "" : languageRow.modelData.percent.toFixed(1) + "%"
                                    color: Theme.textDim; horizontalAlignment: Text.AlignRight; font.pixelSize: 12
                                }
                            }
                        }
                    }
                }
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("Completeness counts finished translations. It does not measure translation quality.")
                color: Theme.textDim; font.pixelSize: 12; wrapMode: Text.WordWrap
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
    function copyText(text) { clipboardHelper.text = text; clipboardHelper.selectAll(); clipboardHelper.copy(); clipboardHelper.text = ""; }

    // The webhook token and browser pairing key aren't in appState; they're
    // fetched when copied or shown.
    property int callSeq: 0
    property var callbacks: ({})
    function withReply(command, done) {
        if (!appBridge) return;
        const key = "settings" + (++callSeq);
        callbacks[key] = done;
        appBridge.callCommand(key, command, "{}");
    }
    function withWebhookToken(done) { withReply("webhook_token", done) }
    Connections {
        target: root.appBridge
        function onReplySerialChanged() {
            const key = root.appBridge.replyKey, done = root.callbacks[key];
            if (!done) return;
            delete root.callbacks[key];
            const token = root.appBridge.replyError ? "" : JSON.parse(root.appBridge.replyJson || "null");
            if (token) done(token);
        }
    }

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
            Text { text: qsTr("Name"); color: Theme.text }
            BwTextField { id: secretName; width: parent.width; readOnly: !!secretDialog.editing; placeholderText: qsTr("api token") }
            Text { text: qsTr("Value"); color: Theme.text }
            BwTextField { id: secretValue; width: parent.width; echoMode: TextInput.Password; placeholderText: qsTr("Paste it here"); onAccepted: secretDialog.accept() }
        }
        onAccepted: {
            if (secretName.text.trim() && secretValue.text) root.invoke("set_secret", { name: secretName.text.trim(), value: secretValue.text });
            secretValue.text = "";
        }
        onRejected: secretValue.text = ""
    }
}
