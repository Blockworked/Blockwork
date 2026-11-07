import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Renames one of the current macro's named signals.
AppDialog {
    id: signalDialog
    required property Item page
    property string renameTarget: ""
    property string macroId: ""
    property bool busy: false
    title: qsTr("Rename signal")
    standardButtons: Dialog.NoButton
    function openForRename(name) {
        renameTarget = name; signalName.text = name;
        macroId = page.macro ? page.macro.id : "";
        signalError.text = ""; busy = false;
        open(); signalName.forceActiveFocus(); signalName.selectAll();
    }
    function submit() {
        const name = signalName.text.trim();
        if (busy || !name.length || !page.macro || page.macro.id !== macroId) return;
        busy = true;
        page.daemonCall("rename_signal", { oldName: renameTarget, newName: name }, (result, error) => {
            busy = false;
            if (error) signalError.text = error;
            else close();
        });
    }
    Column {
        width: 400; spacing: 12
        Text { text: qsTr("Name"); color: Theme.text; font.pixelSize: 13 }
        StitchTextField { id: signalName; width: parent.width; enabled: !signalDialog.busy; onAccepted: signalDialog.submit() }
        Text { width: parent.width; wrapMode: Text.WordWrap; text: qsTr("Updates named blocks in this macro. Expressions keep their contents."); color: Theme.textDim; font.pixelSize: 13 }
        Text { id: signalError; width: parent.width; visible: text.length > 0; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 13 }
    }
    footer: DialogButtonBox {
        alignment: Qt.AlignRight; spacing: 8; padding: 18; topPadding: 8
        background: null
        StitchButton { text: qsTr("Cancel"); enabled: !signalDialog.busy; onClicked: signalDialog.reject() }
        StitchButton { text: qsTr("Rename"); primary: true; enabled: !signalDialog.busy && signalName.text.trim().length > 0; onClicked: signalDialog.submit() }
    }
}
