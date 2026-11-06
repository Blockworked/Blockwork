import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// Lists the current macro's named signals, with rename and delete.
AppDialog {
    id: signalsDialog
    title: qsTr("Signals"); standardButtons: Dialog.Close
    required property Item page
    // Asks the page for its "New signal" picker.
    signal newSignalRequested()
    signal renameRequested(string name)
    Column {
        width: 400; spacing: 10
        Text { width: parent.width; text: qsTr("Named signals in this macro. Expressions can also send and receive names outside this list."); wrapMode: Text.WordWrap; color: Theme.textDim }
        ScrollView {
            width: parent.width; height: Math.min(300, signalRows.implicitHeight)
            clip: true
            Column {
                id: signalRows; width: 380; spacing: 6
                Repeater {
                    model: signalsDialog.page.macro ? signalsDialog.page.macro.signals || [] : []
                    delegate: RowLayout {
                        required property string modelData
                        width: signalRows.width; spacing: 6
                        Text { Layout.fillWidth: true; text: modelData; elide: Text.ElideRight; color: Theme.text }
                        BwButton { text: qsTr("Rename"); onClicked: signalsDialog.renameRequested(modelData) }
                        BwButton {
                            text: qsTr("Delete")
                            onClicked: signalsDialog.page.daemonCall("delete_signal", { name: modelData }, (result, error) => { signalsError.text = error || ""; })
                        }
                    }
                }
            }
        }
        Text { visible: !signalsDialog.page.macro || !(signalsDialog.page.macro.signals || []).length; text: qsTr("No named signals yet."); color: Theme.textDim }
        Text { id: signalsError; width: parent.width; visible: text.length > 0; wrapMode: Text.WordWrap; color: Theme.danger }
        BwButton { text: qsTr("New signal…"); iconName: "plus"; onClicked: signalsDialog.newSignalRequested() }
    }
    onOpened: signalsError.text = ""
}
