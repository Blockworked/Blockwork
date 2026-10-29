import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0
import com.blockworked.Android 1.0

Item {
    id: window
    width: 412; height: 800
    Rectangle { anchors.fill: parent; color: Theme.canvas }
    NativeBridge {
        id: bridge
        onBackRequested: {
            if (editor.dismissSheet()) return;
            if (navigation.visible) { navigation.close(); return; }
            bridge.backgroundEditor();
        }
    }
    readonly property var appState: JSON.parse(bridge.stateJson || "{}")
    readonly property bool compact: width < 720
    function invoke(command, args) {
        if (command === "open_settings") { navigation.open(); return; }
        bridge.invokeCommand(command, JSON.stringify(args || {}));
    }
    Item {
        anchors.fill: parent
        // Qt 6.10's safe-area geometry accounts for Android's system bars.
        anchors.leftMargin: SafeArea.margins.left
        anchors.rightMargin: SafeArea.margins.right
        anchors.topMargin: SafeArea.margins.top
        anchors.bottomMargin: SafeArea.margins.bottom
        ColumnLayout {
            anchors.fill: parent; spacing: 0
            ToolBar {
                Layout.fillWidth: true
                RowLayout {
                    anchors.fill: parent; spacing: 0
                    Button { text: qsTr("Macros"); implicitWidth: 64; implicitHeight: 48; onClicked: navigation.open() }
                    Label { Layout.fillWidth: true; elide: Text.ElideRight; text: window.appState.current_macro ? window.appState.current_macro.name : "Blockwork" }
                    Button { text: qsTr("Blocks"); implicitWidth: 60; implicitHeight: 48; enabled: !!window.appState.current_macro; onClicked: editor.openBlocks() }
                    Button { text: qsTr("Log"); implicitWidth: 48; implicitHeight: 48; onClicked: editor.openActivity() }
                    Button { text: qsTr("Run"); implicitWidth: 48; implicitHeight: 48; enabled: !!window.appState.current_macro; onClicked: window.invoke("run_macro") }
                    Button { text: qsTr("Stop"); implicitWidth: 48; implicitHeight: 48; onClicked: window.invoke("emergency_stop") }
                }
            }
            EditorPage {
                id: editor
                Layout.fillWidth: true; Layout.fillHeight: true
                appState: window.appState
                invoke: window.invoke
                appBridge: bridge
                showToolbar: !window.compact
                sidebarCollapsed: window.compact
                sidebarWidth: Math.min(348, window.width - 48)
            }
            Label {
                Layout.fillWidth: true
                visible: bridge.lastError.length > 0
                text: bridge.lastError; wrapMode: Text.Wrap; color: Theme.danger
                padding: 8
            }
        }
        Keys.onReleased: event => {
            if (event.key !== Qt.Key_Back) return;
            if (Qt.inputMethod.visible) { Qt.inputMethod.hide(); event.accepted = true; }
            else if (editor.dismissSheet()) { event.accepted = true; }
            else if (navigation.opened) { navigation.close(); event.accepted = true; }
        }
        focus: true
    }
    Drawer {
        id: navigation
        width: Math.min(window.width - 48, 360); height: window.height
        ScrollView {
            id: navigationScroll
            anchors.fill: parent; clip: true
            contentWidth: availableWidth
            contentHeight: navigationColumn.implicitHeight + 32
        ColumnLayout {
            id: navigationColumn
            x: 16; y: 16; width: navigationScroll.availableWidth - 32; spacing: 8
            Label { text: qsTr("Workflows"); font.pixelSize: 22 }
            Button { text: qsTr("New workflow"); implicitHeight: 48; Layout.fillWidth: true; onClicked: { window.invoke("new_macro"); navigation.close(); } }
            Button { text: qsTr("Import document"); implicitHeight: 48; Layout.fillWidth: true; onClicked: { bridge.importDocument(); navigation.close(); } }
            Button { text: qsTr("Export workflow"); implicitHeight: 48; Layout.fillWidth: true; enabled: !!window.appState.current_macro; onClicked: bridge.exportDocument(window.appState.current_macro.id) }
            Button { text: qsTr("Pick screen point"); implicitHeight: 48; Layout.fillWidth: true; enabled: !!window.appState.current_macro; onClicked: { navigation.close(); editor.startScreenPick({kind: "tool"}); } }
            Button { text: qsTr("Pick screen region"); implicitHeight: 48; Layout.fillWidth: true; enabled: !!window.appState.current_macro; onClicked: { navigation.close(); editor.startScreenPick({kind: "tool", region: true}); } }
            Button { text: qsTr("Save"); implicitHeight: 48; Layout.fillWidth: true; enabled: !!window.appState.current_macro; onClicked: window.invoke("save_macro") }
            Button { text: qsTr("Enable touch automation"); implicitHeight: 48; Layout.fillWidth: true; onClicked: accessibilityInfo.open() }
            Label {
                Layout.fillWidth: true; wrapMode: Text.Wrap
                text: window.appState.capabilities && window.appState.capabilities.services.touch.ready
                    ? qsTr("Touch automation is ready. Coordinates use physical display pixels.")
                    : qsTr("Touch automation needs the accessibility service and an unlocked device.")
            }
            Button { text: qsTr("Exact alarm access"); implicitHeight: 48; Layout.fillWidth: true; onClicked: bridge.exactAlarmSettings() }
            Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: bridge.scheduleStatus }
            Button { text: qsTr("Resume triggers"); implicitHeight: 48; Layout.fillWidth: true; visible: !!window.appState.triggers_paused; onClicked: window.invoke("resume_triggers") }
            ListView {
                Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                Layout.preferredHeight: Math.min(320, Math.max(144, count * 48))
                model: window.appState.macros_data || []
                delegate: ItemDelegate {
                    required property var modelData
                    width: ListView.view.width; height: 48; text: modelData.name
                    onClicked: { window.invoke("select_macro_by_id", {id: modelData.id}); navigation.close(); }
                }
            }
        }
        }
    }
    readonly property var review: appState.import_review || null
    Dialog {
        id: accessibilityInfo
        anchors.centerIn: parent
        width: Math.min(window.width - 32, 480)
        modal: true
        title: qsTr("Touch automation")
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: bridge.accessibilitySettings()
        contentItem: ScrollView {
            implicitHeight: Math.min(320, window.height / 2)
            Label {
                width: accessibilityInfo.availableWidth; wrapMode: Text.Wrap
                text: qsTr("Blockwork uses accessibility to perform gestures and insert text in other apps and read screen content when you run a workflow or use the picker. It also detects foreground app changes for app triggers and launches apps selected in workflows. Enable ‘Blockwork automation’ in Android Accessibility settings. On some Android versions, an APK install also requires App info → menu → Allow restricted settings. Use the floating Stop button or the notification to stop a run.")
            }
        }
    }
    onReviewChanged: { if (review) importReview.open(); else importReview.close(); }
    Dialog {
        id: importReview
        anchors.centerIn: parent
        width: Math.min(window.width - 32, 480)
        modal: true
        title: qsTr("Import workflow")
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: window.invoke("confirm_import_macro", {keepSettings: {}, allow: []})
        onRejected: window.invoke("cancel_import_macro")
        contentItem: ScrollView {
            implicitHeight: Math.min(320, window.height / 2)
            Label {
                width: importReview.availableWidth; wrapMode: Text.Wrap
                text: window.review ? qsTr("Import “%1”? Imported workflows will ask before using guarded abilities. Android cannot run desktop-only blocks.").arg(window.review.name) : ""
            }
        }
    }
}
