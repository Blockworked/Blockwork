import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// Importing and exporting single macro files, shared by every page. An
// import waits for the review dialog below before it's added.
Item {
    id: root
    required property var appState
    required property var invoke
    required property var appBridge

    function importMacro() { importDialog.open(); }
    // What a macro can do, by the daemon's key, in the UI language.
    function capabilityLabel(cap) {
        const labels = {
            commands: qsTr("Runs commands and opens or closes apps"), secrets: qsTr("Reads your saved secrets"),
            network: qsTr("Goes online"), browser: qsTr("Controls your web browser (clicks, types and reads pages)"), files_write: qsTr("Writes to files"), files_read: qsTr("Reads files and folders"),
            input: qsTr("Presses keys, clicks and moves the mouse"), clipboard: qsTr("Reads or changes the clipboard"),
            windows: qsTr("Reads or controls other apps and windows"),
            triggers: qsTr("Starts on its own (schedules, keys, typing, apps, idle, windows, files and more)"),
            screen: qsTr("Reads the screen (colors, images and text)"), macros: qsTr("Runs or stops your other macros"),
            globals: qsTr("Reads or changes global variables")
        };
        return labels[cap.kind] || cap.label;
    }
    function settingLabel(setting) {
        const labels = {
            always_listen: qsTr("Always listen for events, even when a different macro is selected"),
            catch_up_missed: qsTr("Run schedules it missed while Blockwork was closed or the computer slept")
        };
        return labels[setting.key] || setting.label;
    }
    function exportMacro(macroId) {
        exportDialog.macroId = macroId;
        daemonCall("export_macro_file_name", { macroId: macroId }, name => {
            const folder = StandardPaths.writableLocation(StandardPaths.DocumentsLocation).toString();
            exportDialog.currentFolder = folder;
            exportDialog.selectedFile = folder + "/" + encodeURIComponent(name || "macro.macro");
            exportDialog.open();
        });
    }
    // Several macros in one .blockwork file; the daemon adds macros they run.
    function exportBundle(macroIds, name) {
        bundleDialog.macroIds = macroIds;
        bundleDialog.bundleName = name;
        const folder = StandardPaths.writableLocation(StandardPaths.DocumentsLocation).toString();
        bundleDialog.currentFolder = folder;
        bundleDialog.selectedFile = folder + "/" + encodeURIComponent((name || "macros").replace(/[\\/:*?"<>|]/g, "_") + ".blockwork");
        bundleDialog.open();
    }
    // file:// URL from a dialog to a local path.
    function localPath(url) {
        const text = decodeURIComponent(url.toString());
        return Qt.platform.os === "windows" ? text.replace(/^file:\/\/\//, "") : text.replace(/^file:\/\//, "");
    }

    property int callSeq: 0
    property var callbacks: ({})
    function daemonCall(command, args, done) {
        const key = "files" + (++callSeq);
        callbacks[key] = done;
        root.appBridge.callCommand(key, command, JSON.stringify(args || {}));
    }
    Connections {
        target: root.appBridge
        function onReplySerialChanged() {
            const key = root.appBridge.replyKey, done = root.callbacks[key];
            if (!done) return;
            delete root.callbacks[key];
            if (root.appBridge.replyError) failed.show(root.appBridge.replyError);
            else done(JSON.parse(root.appBridge.replyJson || "null"));
        }
    }

    FileDialog {
        id: importDialog
        title: qsTr("Import a macro")
        fileMode: FileDialog.OpenFile
        currentFolder: StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        nameFilters: [qsTr("Blockwork macros and bundles (*.macro *.blockwork *.json)"), qsTr("All files (*)")]
        onAccepted: root.daemonCall("import_macro", { path: root.localPath(selectedFile) }, () => {})
    }
    FileDialog {
        id: exportDialog
        property string macroId: ""
        title: qsTr("Export macro")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "macro"
        nameFilters: [qsTr("Blockwork macros (*.macro)")]
        onAccepted: root.daemonCall("export_macro", { macroId: macroId, path: root.localPath(selectedFile) }, () => {})
    }
    FileDialog {
        id: bundleDialog
        property var macroIds: []
        property string bundleName: ""
        title: qsTr("Export bundle")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "blockwork"
        nameFilters: [qsTr("Blockwork bundles (*.blockwork)")]
        onAccepted: root.daemonCall("export_bundle", { macroIds: macroIds, name: bundleName, path: root.localPath(selectedFile) }, count => {
            exported.message = qsTr("Exported %n macro(s), including any macros they run.", "", count);
            exported.open();
        })
    }
    AppDialog {
        id: exported
        title: qsTr("Bundle exported"); standardButtons: Dialog.Ok
        property string message: ""
        Text { width: 380; wrapMode: Text.WordWrap; text: exported.message; color: Theme.text; font.pixelSize: 13 }
    }
    AppDialog {
        id: failed
        title: qsTr("Something went wrong"); standardButtons: Dialog.Ok
        property string message: ""
        function show(text) { message = text; open(); }
        Text { width: 420; wrapMode: Text.WordWrap; text: failed.message; color: Theme.text; font.pixelSize: 13 }
    }

    // An imported macro waits here until the user has seen what it can do.
    readonly property var importReview: appState.import_review || null
    readonly property string importReviewKey: importReview ? JSON.stringify(importReview) : ""
    property var keepSettings: ({})
    // Guarded abilities the user unticked; they'll ask when first used.
    property var askFirst: ({})
    onImportReviewKeyChanged: {
        if (importReview) { keepSettings = ({}); askFirst = ({}); reviewDialog.open(); }
        else reviewDialog.close();
    }
    AppDialog {
        id: reviewDialog
        readonly property var caps: root.importReview ? root.importReview.capabilities : []
        readonly property var customSettings: root.importReview ? root.importReview.custom_settings : []
        readonly property var macroNames: root.importReview ? root.importReview.macros || [] : []
        readonly property var notes: root.importReview ? root.importReview.notes || [] : []
        title: root.importReview ? qsTr("Import \"%1\"?").arg(root.importReview.name) : qsTr("Import macro")
        standardButtons: Dialog.Ok | Dialog.Cancel
        acceptText: qsTr("Import")
        Column { width: 460; spacing: 10
            Text {
                visible: reviewDialog.macroNames.length > 1
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: qsTr("This bundle has %n macro(s): %1.", "", reviewDialog.macroNames.length).arg(reviewDialog.macroNames.join(", "))
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                readonly property bool many: reviewDialog.macroNames.length > 1
                text: reviewDialog.caps.length ? (many ? qsTr("These macros can:") : qsTr("This macro can:"))
                    : many ? qsTr("These macros only use blocks that stay inside Blockwork, like waits and variables.")
                    : qsTr("This macro only uses blocks that stay inside Blockwork, like waits and variables.")
            }
            Repeater {
                model: reviewDialog.caps
                delegate: Row {
                    required property var modelData
                    spacing: 8
                    LucideIcon {
                        anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16
                        name: modelData.risky ? "triangle-alert" : "check"; color: modelData.risky ? Theme.warning : Theme.textDim
                    }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: root.capabilityLabel(modelData); color: modelData.risky ? Theme.warning : Theme.text; font.pixelSize: 13 }
                    BwCheckBox {
                        visible: modelData.guarded
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("allow without asking")
                        checked: !root.askFirst[modelData.kind]
                        onToggled: { const next = Object.assign({}, root.askFirst); next[modelData.kind] = !checked; root.askFirst = next; }
                    }
                }
            }
            Text {
                visible: reviewDialog.caps.some(c => c.risky)
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Only import macros that run commands, go online, write files or read secrets from people you trust. Untick an ability to be asked the first time it's used; you can change this later in Macro settings.")
            }
            Text {
                visible: reviewDialog.notes.length > 0; topPadding: 6
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: qsTr("Made with an older Blockwork, so it was updated:")
            }
            Repeater {
                model: reviewDialog.notes
                delegate: Text {
                    required property string modelData
                    width: 460; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                    text: "• " + modelData
                }
            }
            Text {
                visible: reviewDialog.customSettings.length > 0; topPadding: 6
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: qsTr("It also comes with these settings. Untick any you don't want:")
            }
            Repeater {
                model: reviewDialog.customSettings
                delegate: BwCheckBox {
                    required property var modelData
                    text: root.settingLabel(modelData); checked: modelData.enabled
                    onToggled: root.keepSettings[modelData.key] = checked
                }
            }
        }
        onAccepted: root.invoke("confirm_import_macro", {
            keepSettings: root.keepSettings,
            allow: caps.filter(c => c.guarded && !root.askFirst[c.kind]).map(c => c.kind)
        })
        onRejected: root.invoke("cancel_import_macro")
    }
}
