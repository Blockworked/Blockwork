import QtQuick
import QtQuick.Controls
import BlockworkStyle as Editing
import com.blockworked.Blockstitch 1.0

// Blockwork's "Make a Block" dialog: blockstitch's shared definition editor
// with Blockwork's wording for the publishing details.
AppDialog {
    id: root
    title: editBlockId.length ? qsTr("Edit Block") : qsTr("Make a Block")
    width: 720
    // Follow the window only while open; closed, a resize would relayout all of it.
    height: visible ? Math.min(760, parent ? parent.height - 32 : 760) : 760
    standardButtons: Dialog.NoButton

    signal createRequested(var pieces, string shape, string color, string icon, string identifier, string description)
    signal editRequested(string blockId, var pieces, string shape, string color, string icon, string identifier, string description)

    // Set while editing an existing block; empty when making a new one.
    property string editBlockId: ""
    property var pendingDef: null
    property alias editor: editor

    function openForCreate() {
        editBlockId = "";
        pendingDef = null;
        open();
    }
    function openForEdit(def) {
        editBlockId = def.id;
        pendingDef = def;
        open();
    }
    function submit() {
        editor.submit();
    }
    onOpened: {
        if (pendingDef)
            editor.load(pendingDef);
        else
            editor.reset();
        editor.focusFirst();
    }
    onClosed: {
        editor.closePopups();
        editBlockId = "";
        pendingDef = null;
    }

    contentItem: BlockDefinitionEditor {
        id: editor
        detailsTitle: qsTr("Publishing")
        detailsIcon: "upload"
        detailsNote: qsTr("Details shown when sharing this block in a community pack.")
        textMenu: Component { Editing.TextEditingMenu {} }
        onCreateRequested: draft => {
            root.createRequested(draft.pieces, draft.shape, draft.color, draft.icon, draft.identifier, draft.description);
            root.accept();
        }
        onEditRequested: (blockId, draft) => {
            root.editRequested(blockId, draft.pieces, draft.shape, draft.color, draft.icon, draft.identifier, draft.description);
            root.accept();
        }
        onCancelled: root.reject()
    }
}
