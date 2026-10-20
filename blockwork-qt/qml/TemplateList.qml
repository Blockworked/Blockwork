import QtQuick
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// Starter macros as clickable cards; `picked(id)` fires with the template id.
GridLayout {
    id: root
    property var templates: []
    signal picked(string id)
    columns: width > 520 ? 2 : 1
    columnSpacing: 10; rowSpacing: 10

    Repeater {
        model: root.templates
        delegate: Rectangle {
            id: card
            required property var modelData
            Layout.fillWidth: true
            Layout.minimumWidth: 0; Layout.preferredWidth: 1
            Layout.preferredHeight: Math.max(92, cardContent.implicitHeight + 24)
            radius: 10; color: cardMouse.containsMouse ? "#30343b" : "#2a2b2f"
            border.color: cardMouse.containsMouse ? Theme.accent : "#3a3b3f"
            RowLayout {
                id: cardContent
                anchors.fill: parent; anchors.margins: 12; spacing: 12
                LucideIcon {
                    name: card.modelData.icon; color: Theme.accent
                    Layout.minimumWidth: 22; Layout.preferredWidth: 22; Layout.maximumWidth: 22
                    Layout.preferredHeight: 22; Layout.alignment: Qt.AlignTop
                }
                ColumnLayout { Layout.fillWidth: true; Layout.minimumWidth: 0; spacing: 3
                    Text { Layout.fillWidth: true; Layout.minimumWidth: 0; text: card.modelData.name; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold; wrapMode: Text.WordWrap }
                    Text { Layout.fillWidth: true; Layout.minimumWidth: 0; text: card.modelData.description; color: "#9fa0a6"; font.pixelSize: 12; wrapMode: Text.WordWrap }
                }
            }
            MouseArea { id: cardMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.picked(card.modelData.id) }
            Accessible.role: Accessible.Button; Accessible.name: card.modelData.name
        }
    }
}
