import QtQuick.Controls
import QtQuick.Controls.Basic as Basic

Basic.TextArea {
    id: control
    ContextMenu.menu: TextEditingMenu { editor: control }
}
