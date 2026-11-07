pragma Translator: "EditorPage"

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

AppDialog {
    id: dialog
    title: qsTr("Macro settings")
    standardButtons: Dialog.Close
    required property var macro
    required property var invoke
    required property var permissions
    required property var guardedAbilities
    readonly property var settings: macro && macro.settings ? macro.settings : ({})
    width: Math.min(560, parent ? parent.width - 32 : 560)
    height: Math.min(660, parent ? parent.height - 32 : 660)
    onOpened: editor.reset()
    contentItem: MacroSettingsEditor {
        id: editor
        macro: dialog.macro
        invoke: dialog.invoke
        permissions: dialog.permissions
        guardedAbilities: dialog.guardedAbilities
    }
}
