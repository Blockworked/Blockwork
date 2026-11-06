import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Makes or renames a variable or a list in the current macro. Not built on
// blockstitch's NameDialog because it needs AppDialog's translated buttons and
// window-shaped dimming.
AppDialog {
    id: dialog
    required property Item page
    // "variable" or "list": picks the commands and the wording.
    required property string kind
    // objectName of the name field, so tests can find it.
    property string fieldObjectName: ""
    property string renameTarget: ""
    property var selectCreated: null
    property string errorText: ""
    readonly property bool isList: kind === "list"
    title: renameTarget.length
        ? (isList ? qsTr("Rename List") : qsTr("Rename Variable"))
        : (isList ? qsTr("Make a List") : qsTr("Make a Variable"))
    standardButtons: Dialog.Ok | Dialog.Cancel
    function openForCreate(select) { renameTarget = ""; selectCreated = select || null; errorText = ""; nameField.text = ""; open(); nameField.forceActiveFocus(); }
    function openForRename(name) { selectCreated = null; errorText = ""; renameTarget = name; nameField.text = name; open(); nameField.forceActiveFocus(); nameField.selectAll(); }
    Column { width: 340; spacing: 8
        Text { text: dialog.isList ? qsTr("List name") : qsTr("Variable name"); color: Theme.text }
        BwTextField {
            id: nameField; objectName: dialog.fieldObjectName; width: parent.width
            placeholderText: dialog.isList ? qsTr("items") : qsTr("score")
            onAccepted: dialog.accept()
        }
        Text { width: parent.width; visible: text.length > 0; text: dialog.errorText; color: Theme.text; wrapMode: Text.WordWrap }
    }
    onRejected: selectCreated = null
    onAccepted: {
        const name = nameField.text.trim();
        if (!name.length) {
            errorText = isList ? qsTr("Enter a list name.") : qsTr("Enter a variable name.");
            Qt.callLater(() => { open(); nameField.forceActiveFocus(); });
            return;
        }
        const select = selectCreated;
        const macroId = page.macro ? page.macro.id : "";
        const command = (renameTarget.length ? "rename_" : "create_") + kind;
        const args = renameTarget.length ? { oldName: renameTarget, newName: name } : { name: name };
        if (renameTarget === name) { selectCreated = null; return; }
        page.daemonCall(command, args, (result, error) => {
            if (!page.macro || page.macro.id !== macroId) return;
            if (error) {
                errorText = error;
                open(); nameField.forceActiveFocus();
                return;
            }
            selectCreated = null;
            renameTarget = ""; nameField.clear();
            if (select) select(name);
        });
    }
}
