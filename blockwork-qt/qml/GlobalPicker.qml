import QtQuick

NamePicker {
    required property var globals
    names: (globals || []).map(g => g.name)
    chooseTitle: qsTr("Choose a global")
    createTitle: qsTr("New global")
    newText: qsTr("New global…")
    createCommand: "create_global"
    scopeText: qsTr("Shared by all macros")
}
