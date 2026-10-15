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
    function exportMacro(macroId) {
        exportDialog.macroId = macroId;
        daemonCall("export_macro_file_name", { macroId: macroId }, name => {
            const folder = StandardPaths.writableLocation(StandardPaths.DocumentsLocation).toString();
            exportDialog.currentFolder = folder;
            exportDialog.selectedFile = folder + "/" + encodeURIComponent(name || "macro.macro");
            exportDialog.open();
        });
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
        title: "Import a macro"
        fileMode: FileDialog.OpenFile
        currentFolder: StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        nameFilters: ["Blockwork macros (*.macro *.json)", "All files (*)"]
        onAccepted: root.daemonCall("import_macro", { path: root.localPath(selectedFile) }, () => {})
    }
    FileDialog {
        id: exportDialog
        property string macroId: ""
        title: "Export macro"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "macro"
        nameFilters: ["Blockwork macros (*.macro)"]
        onAccepted: root.daemonCall("export_macro", { macroId: macroId, path: root.localPath(selectedFile) }, () => {})
    }
    BwDialog {
        id: failed
        title: "Something went wrong"; standardButtons: Dialog.Ok
        property string message: ""
        function show(text) { message = text; open(); }
        Text { width: 420; wrapMode: Text.WordWrap; text: failed.message; color: Theme.text; font.pixelSize: 13 }
    }

    // An imported macro waits here until the user has seen what it can do.
    readonly property var importReview: appState.import_review || null
    readonly property string importReviewKey: importReview ? JSON.stringify(importReview) : ""
    property var keepSettings: ({})
    onImportReviewKeyChanged: {
        if (importReview) { keepSettings = ({}); reviewDialog.open(); }
        else reviewDialog.close();
    }
    BwDialog {
        id: reviewDialog
        readonly property var caps: root.importReview ? root.importReview.capabilities : []
        readonly property var customSettings: root.importReview ? root.importReview.custom_settings : []
        title: root.importReview ? "Import “" + root.importReview.name + "”?" : "Import macro"
        standardButtons: Dialog.Ok | Dialog.Cancel
        Component.onCompleted: standardButton(Dialog.Ok).text = "Import"
        Column { width: 460; spacing: 10
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: reviewDialog.caps.length ? "This macro can:" : "This macro only uses blocks that stay inside Blockwork, like waits and variables."
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
                    Text { text: modelData.label; color: modelData.risky ? Theme.warning : Theme.text; font.pixelSize: 13 }
                }
            }
            Text {
                visible: reviewDialog.caps.some(c => c.risky)
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: "Only import macros that run commands, go online, write files or read secrets from people you trust."
            }
            Text {
                visible: reviewDialog.customSettings.length > 0; topPadding: 6
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: "It also comes with these settings. Untick any you don't want:"
            }
            Repeater {
                model: reviewDialog.customSettings
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
}
