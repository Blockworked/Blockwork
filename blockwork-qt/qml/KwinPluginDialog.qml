import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Installs, updates or rebuilds Blockwork's KWin plugin for background input.
AppDialog {
    id: dialog
    objectName: "kwinPluginDialog"
    title: qsTr("Background input plugin")
    standardButtons: busy ? Dialog.NoButton : Dialog.Close
    closePolicy: busy ? Popup.NoAutoClose : Popup.CloseOnEscape
    required property var appBridge
    property var plugin: null
    property bool busy: false
    property string outcome: ""
    property bool outcomeFailed: false
    property bool buildHere: false
    signal installed()

    function openFor(p, build) { plugin = p; buildHere = !!build; busy = false; outcome = ""; outcomeFailed = false; open() }
    // Asks once per start whether this user's copy needs an update.
    function checkAtStartup() {
        if (Qt.platform.os !== "linux") return;
        call("background_input_status", {}, s => {
            if (s && s.plugin && s.plugin.installed_for && (s.plugin.state === "outdated" || s.plugin.state === "stale")) openFor(s.plugin);
        });
    }

    property int callSeq: 0
    property var callbacks: ({})
    function call(command, args, done, failed) {
        if (!appBridge) return;
        const key = "kwinplugin" + (++callSeq);
        callbacks[key] = { done: done, failed: failed };
        appBridge.callCommand(key, command, JSON.stringify(args));
    }
    Connections {
        target: dialog.appBridge
        function onReplySerialChanged() {
            const key = dialog.appBridge.replyKey, call = dialog.callbacks[key];
            if (!call) return;
            delete dialog.callbacks[key];
            const error = dialog.appBridge.replyError;
            if (error) { if (call.failed) call.failed(error); return; }
            call.done(JSON.parse(dialog.appBridge.replyJson || "null"));
        }
    }

    readonly property string pluginState: plugin ? plugin.state : ""
    Column { width: 440; spacing: 10
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
            text: dialog.pluginState === "outdated"
                ? qsTr("The plugin was built for KWin %1, and Plasma now runs KWin %2. Update it to keep using background input.").arg(dialog.plugin.installed_for).arg(dialog.plugin.kwin)
                : dialog.pluginState === "stale" ? qsTr("This version of Blockwork comes with a newer KWin plugin (%1, you have %2). Update it to keep background input working.").arg(dialog.plugin.plugin).arg(dialog.plugin.installed_plugin)
                : dialog.buildHere ? qsTr("If background input still isn't running after you logged out and back in, the downloaded plugin may not suit your distro's Qt. Blockwork can build it on this computer instead.")
                : qsTr("The In window block's background input runs inside KWin, Plasma's window manager, through a small plugin.") }
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Blockwork downloads the build made for your KWin (%1) from its GitHub releases, or builds it on this computer when there isn't one. It's installed for your user only, so no password is needed, and Remove in Settings > Integrations takes it out again.").arg(dialog.plugin ? dialog.plugin.kwin : "") }
        Text { visible: dialog.busy; width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Installing... building it here can take a minute or two.") }
        Text { visible: dialog.outcome !== ""; width: parent.width; wrapMode: Text.WordWrap; font.pixelSize: 12
            color: dialog.outcomeFailed ? Theme.danger : Theme.running
            text: dialog.outcome.charAt(0).toUpperCase() + dialog.outcome.slice(1) + (dialog.outcomeFailed ? "" : ".") }
        StitchButton {
            visible: !dialog.busy && (dialog.outcome === "" || dialog.outcomeFailed)
            iconName: "download"; primary: true
            text: dialog.outcomeFailed ? qsTr("Try again") : dialog.pluginState === "missing" || dialog.pluginState === "" ? qsTr("Install") : qsTr("Update")
            onClicked: {
                dialog.busy = true; dialog.outcome = ""; dialog.outcomeFailed = false;
                dialog.call("kwin_plugin_install", { build: dialog.buildHere }, result => {
                    dialog.busy = false; dialog.outcome = result.message; dialog.installed();
                }, error => {
                    dialog.busy = false; dialog.outcomeFailed = true; dialog.outcome = error;
                });
            }
        }
    }
}
