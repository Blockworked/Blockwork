import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch

BwMenu {
    id: menu
    required property Item editor
    readonly property bool editable: !editor.readOnly
    readonly property bool selected: editor.selectionStart !== editor.selectionEnd
    readonly property bool canCopy: selected && (editor.echoMode === undefined || editor.echoMode === TextInput.Normal)

    // Use the same in-window rendering as the canvas menus on every platform.
    popupType: Popup.Item
    delegate: BwMenuItem {
        iconName: action ? action.objectName : ""
        opacity: enabled ? 1 : 0.4
    }

    Action {
        objectName: "undo"
        text: qsTr("Undo")
        enabled: menu.editable && menu.editor.canUndo
        onTriggered: menu.editor.undo()
    }
    Action {
        objectName: "redo"
        text: qsTr("Redo")
        enabled: menu.editable && menu.editor.canRedo
        onTriggered: menu.editor.redo()
    }
    MenuSeparator {
        contentItem: Rectangle { implicitHeight: 1; color: Theme.border }
    }
    Action {
        objectName: "scissors"
        text: qsTr("Cut")
        enabled: menu.editable && menu.canCopy
        onTriggered: menu.editor.cut()
    }
    Action {
        objectName: "copy"
        text: qsTr("Copy")
        enabled: menu.canCopy
        onTriggered: menu.editor.copy()
    }
    Action {
        objectName: "clipboard-paste"
        text: qsTr("Paste")
        enabled: menu.editable && menu.editor.canPaste
        onTriggered: menu.editor.paste()
    }
    Action {
        objectName: "trash"
        text: qsTr("Delete")
        enabled: menu.editable && menu.selected
        onTriggered: menu.editor.remove(menu.editor.selectionStart, menu.editor.selectionEnd)
    }
    MenuSeparator {
        contentItem: Rectangle { implicitHeight: 1; color: Theme.border }
    }
    Action {
        objectName: "text-select"
        text: qsTr("Select All")
        enabled: menu.editor.length > 0
        onTriggered: menu.editor.selectAll()
    }
}
