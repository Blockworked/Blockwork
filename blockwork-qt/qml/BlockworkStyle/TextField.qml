import QtQuick.Controls
import QtQuick.Controls.Basic as Basic

Basic.TextField {
    id: control
    ContextMenu.menu: TextEditingMenu { editor: control }
}
