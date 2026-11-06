import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// The Open app block's launch options: arguments, folder, environment.
AppDialog {
    id: launchDialog; title: qsTr("Launch options"); standardButtons: Dialog.Ok | Dialog.Cancel; acceptText: qsTr("Save")
    // The EditorPage this dialog edits: its macro and invoke().
    required property Item page
    property var request: null
    readonly property bool mac: Qt.platform.os === "osx"
    function openFor(req) {
        request = req;
        const l = (req.instruction && req.instruction.launch) || {};
        launchArgs.text = l.args || "";
        launchDir.text = l.working_dir || "";
        launchEnv.clear();
        for (const v of (l.env || [])) launchEnv.append({ name: v.name || "", value: v.value || "" });
        launchNewCopy.checked = !!l.new_instance;
        open();
        launchArgs.forceActiveFocus();
    }
    onAccepted: {
        const req = request;
        request = null;
        if (!req) return;
        const env = [];
        for (let i = 0; i < launchEnv.count; ++i) {
            const row = launchEnv.get(i);
            if (row.name.trim()) env.push({ name: row.name.trim(), value: row.value });
        }
        const updated = JSON.parse(JSON.stringify(req.instruction || {}));
        updated.launch = { args: launchArgs.text.trim(), working_dir: launchDir.text.trim(), env: env, new_instance: launchNewCopy.checked };
        page.invoke("edit_instruction", { strandId: req.strandId, path: req.path, instruction: updated });
    }
    ListModel { id: launchEnv }
    FolderDialog {
        id: launchFolder; title: qsTr("Choose a working folder")
        currentFolder: StandardPaths.writableLocation(StandardPaths.HomeLocation)
        onAccepted: {
            const text = decodeURIComponent(selectedFolder.toString());
            launchDir.text = Qt.platform.os === "windows" ? text.replace(/^file:\/\/\//, "") : text.replace(/^file:\/\//, "");
        }
    }
    Column { width: 440; spacing: 8
        Text { text: qsTr("Arguments"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
        BwTextField { id: launchArgs; width: parent.width; placeholderText: "--incognito" }
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
            text: qsTr("Added after the app's own command, the way you'd type them in a terminal. Put quotes around an argument with spaces.") }
        Item { width: 1; height: 4 }
        Text { text: qsTr("Working folder"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
        RowLayout { width: parent.width; spacing: 8
            BwTextField { id: launchDir; Layout.fillWidth: true; placeholderText: qsTr("Leave empty to use the default") }
            BwButton { text: qsTr("Browse..."); iconName: "folder-open"; onClicked: launchFolder.open() }
        }
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
            text: launchDialog.mac ? qsTr("macOS always starts apps in their own folder, so this is only used on Windows and Linux.")
                                   : qsTr("The folder the app starts in. ~ means your home folder.") }
        Item { width: 1; height: 4 }
        Text { text: qsTr("Environment variables"); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
        Text { visible: launchEnv.count === 0; width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
            text: qsTr("None. These are set for this app only.") }
        Repeater {
            model: launchEnv
            delegate: RowLayout {
                required property int index
                required property string name
                required property string value
                width: 440; spacing: 6
                BwTextField { Layout.preferredWidth: 150; text: name; placeholderText: qsTr("NAME"); onTextEdited: launchEnv.setProperty(index, "name", text) }
                Text { text: "="; color: Theme.textDim }
                BwTextField { Layout.fillWidth: true; text: value; placeholderText: qsTr("value"); onTextEdited: launchEnv.setProperty(index, "value", text) }
                BwButton { iconName: "x"; implicitWidth: 32; onClicked: launchEnv.remove(index) }
            }
        }
        BwButton { text: qsTr("Add variable"); iconName: "plus"; onClicked: launchEnv.append({ name: "", value: "" }) }
        Item { width: 1; height: 4 }
        BwCheckBox { id: launchNewCopy; text: qsTr("Open a new copy even if the app is already running") }
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
            text: qsTr("Only changes anything on macOS. On Windows and Linux the app decides whether a second launch opens a new window.") }
    }
}
