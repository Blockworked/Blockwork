import QtQuick
import QtQuick.Controls

// "Make a ..." / "Rename ..." dialog for a named thing (variable, list, dict).
// The host validates: `submitted` fires with the trimmed name, and the host
// calls `fail(message)` to keep the dialog open with an error, or `close()`.
BwDialog {
    id: root
    property string noun: "list"
    property string renameTarget: ""
    property string placeholder: ""
    property string error: ""
    // Optional extra choice shown under the name (e.g. "this actor only").
    default property alias extra: extraColumn.data
    signal submitted(string name, string renameTarget)

    // Whole phrases per noun, so translations don't have to glue words.
    readonly property var phrases: ({
        variable: { make: qsTr("Make a Variable"), rename: qsTr("Rename Variable"), label: qsTr("Variable name"), empty: qsTr("Variable name can't be empty") },
        list: { make: qsTr("Make a List"), rename: qsTr("Rename List"), label: qsTr("List name"), empty: qsTr("List name can't be empty") },
        dict: { make: qsTr("Make a Dict"), rename: qsTr("Rename Dict"), label: qsTr("Dict name"), empty: qsTr("Dict name can't be empty") }
    })
    readonly property var phrase: phrases[noun] || { make: qsTr("Make a %1").arg(noun), rename: qsTr("Rename %1").arg(noun), label: qsTr("Name"), empty: qsTr("The name can't be empty") }
    title: renameTarget.length ? phrase.rename : phrase.make
    standardButtons: Dialog.Ok | Dialog.Cancel
    closePolicy: Popup.CloseOnEscape

    function openForCreate() { renameTarget = ""; nameField.text = ""; error = ""; open(); nameField.forceActiveFocus(); }
    function openForRename(name) { renameTarget = name; nameField.text = name; error = ""; open(); nameField.forceActiveFocus(); nameField.selectAll(); }
    function fail(message) { error = message; if (!visible) open(); nameField.forceActiveFocus(); }

    onAccepted: {
        const name = nameField.text.trim();
        if (!name.length) { fail(phrase.empty); return; }
        submitted(name, renameTarget);
    }

    Column {
        width: 340; spacing: 8
        Text { text: root.phrase.label; color: Theme.text }
        BwTextField {
            id: nameField; width: parent.width; placeholderText: root.placeholder
            onAccepted: root.accept()
            onTextChanged: root.error = ""
        }
        Text { visible: root.error.length > 0; text: root.error; width: parent.width; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 12 }
        Column { id: extraColumn; width: parent.width; spacing: 6 }
    }
}
