import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// A block's name, identifier and what it does, from the block menu's Details.
AppDialog {
    id: detailsDialog
    standardButtons: Dialog.NoButton
    width: Math.min(430, parent.width - 32)
    padding: 0; topPadding: 0; bottomPadding: 0
    showClose: false
    property string detailName: ""
    property string detailIdentifier: ""
    property string detailExplainer: ""
    function show(name, identifier, explainer) {
        detailName = name; detailIdentifier = identifier; detailExplainer = explainer;
        open();
    }
    contentItem: Column {
        spacing: 12; padding: 18
        Row {
            width: parent.width - 36
            spacing: 9
            LucideIcon { name: "info"; width: 20; height: 20; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
            Text {
                width: parent.width - 29
                text: detailsDialog.detailName
                color: Theme.text; font.pixelSize: 17; font.weight: Font.Bold
                wrapMode: Text.Wrap
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        Rectangle {
            width: parent.width - 36
            height: Math.max(30, identifierText.implicitHeight + 12)
            radius: 5; color: Theme.field; border.color: Theme.border
            Text {
                id: identifierText
                anchors.centerIn: parent
                width: parent.width - 16
                text: detailsDialog.detailIdentifier
                color: Theme.accent; font.family: "monospace"; font.pixelSize: 12
                wrapMode: Text.WrapAnywhere
                horizontalAlignment: Text.AlignHCenter
            }
        }
        Text { width: parent.width - 36; text: detailsDialog.detailExplainer; color: Theme.textDim; font.pixelSize: 13; wrapMode: Text.Wrap }
        Row { anchors.right: parent.right; anchors.rightMargin: 18; StitchButton { text: qsTr("Close"); onClicked: detailsDialog.close() } }
    }
}
