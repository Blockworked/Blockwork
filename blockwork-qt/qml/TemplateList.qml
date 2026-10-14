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
            Layout.fillWidth: true; Layout.preferredHeight: 74
            radius: 10; color: cardMouse.containsMouse ? "#30343b" : "#2a2b2f"
            border.color: cardMouse.containsMouse ? Theme.accent : "#3a3b3f"
            RowLayout {
                anchors.fill: parent; anchors.margins: 12; spacing: 12
                LucideIcon { name: card.modelData.icon; width: 22; height: 22; color: Theme.accent; Layout.alignment: Qt.AlignTop }
                ColumnLayout { Layout.fillWidth: true; spacing: 3
                    Text { text: card.modelData.name; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                    Text { Layout.fillWidth: true; text: card.modelData.description; color: "#9fa0a6"; font.pixelSize: 12; wrapMode: Text.WordWrap; maximumLineCount: 2; elide: Text.ElideRight }
                }
            }
            MouseArea { id: cardMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.picked(card.modelData.id) }
            Accessible.role: Accessible.Button; Accessible.name: card.modelData.name
        }
    }
}
