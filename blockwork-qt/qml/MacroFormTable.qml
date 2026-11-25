import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

ColumnLayout {
    id: root
    required property var rows
    readonly property var header: rows.length ? rows[0] : []
    spacing: 4

    component Cells: RowLayout {
        required property var cells
        property bool heading: false
        spacing: 8
        Repeater {
            model: root.header.length
            delegate: Text {
                required property int index
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                Layout.minimumWidth: 0
                text: String(cells[index] ?? "")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: Theme.text
                font.pixelSize: 13
                font.bold: heading
                Accessible.name: text
                id: cell
                ToolTip {
                    visible: cell.truncated && hover.hovered
                    text: cell.text
                    contentItem: Text {
                        text: cell.text
                        textFormat: Text.PlainText
                        color: Theme.text
                    }
                }
                HoverHandler { id: hover }
            }
        }
    }

    Cells {
        Layout.fillWidth: true
        cells: root.header
        heading: true
    }

    ListView {
        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(240, Math.max(0, root.rows.length - 1) * 30)
        model: root.rows.slice(1)
        clip: true
        ScrollBar.vertical: ScrollBar {}
        delegate: Rectangle {
            required property var modelData
            required property int index
            width: ListView.view.width
            implicitHeight: 30
            color: index % 2 ? "transparent" : Theme.panelRaised
            radius: Theme.radius
            Cells {
                anchors.fill: parent
                anchors.leftMargin: 6; anchors.rightMargin: 6
                cells: parent.modelData
            }
        }
    }
}
