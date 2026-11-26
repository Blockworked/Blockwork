import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// A table of strings: the first row is the header. A selectable table
// writes the clicked row back through `selected`, and clicking a header
// sorts the rows.
ColumnLayout {
    id: root
    required property var rows
    property bool selectable: false
    property bool sortable: false
    property string selectedValue: ""
    signal selected(var row)
    property int sortColumn: -1
    property bool ascending: true
    readonly property var header: rows.length ? rows[0] : []
    readonly property var body: {
        const out = rows.slice(1);
        if (sortColumn < 0) return out;
        const column = sortColumn, direction = ascending ? 1 : -1;
        return out.slice().sort((a, b) => {
            const x = String(a[column] ?? ""), y = String(b[column] ?? "");
            const nx = Number(x), ny = Number(y);
            const numbers = x.trim() !== "" && y.trim() !== "" && Number.isFinite(nx) && Number.isFinite(ny);
            return direction * (numbers ? nx - ny : x.localeCompare(y, undefined, {numeric: true, sensitivity: "base"}));
        });
    }
    spacing: 4

    function isSelected(row) {
        if (!root.selectable || root.selectedValue === "") return false;
        try {
            const picked = JSON.parse(root.selectedValue);
            if (Array.isArray(picked))
                return JSON.stringify(picked) === JSON.stringify(row);
            if (picked && typeof picked === "object")
                return root.header.every((key, index) => String(picked[key] ?? "") === String(row[index] ?? ""));
        } catch (_) {}
        return false;
    }
    function sortBy(column) {
        if (!root.sortable) return;
        if (sortColumn === column) ascending = !ascending;
        else { sortColumn = column; ascending = true; }
    }

    component Cells: RowLayout {
        id: cells
        required property var cells
        property bool heading: false
        spacing: 8
        Repeater {
            model: root.header.length
            delegate: Item {
                required property int index
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                Layout.minimumWidth: 0
                implicitHeight: cell.implicitHeight
                Text {
                    id: cell
                    width: parent.width
                    text: String(cells.cells[index] ?? "")
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: Theme.text
                    font.pixelSize: 13
                    font.bold: cells.heading
                    Accessible.name: text
                    ToolTip {
                        visible: cell.truncated && hover.hovered
                        text: cell.text
                        contentItem: Text {
                            text: cell.text
                            textFormat: Text.PlainText
                            color: Theme.text
                        }
                    }
                    HoverHandler {
                        id: hover
                        cursorShape: cells.heading && root.sortable ? Qt.PointingHandCursor : Qt.ArrowCursor
                    }
                }
                LucideIcon {
                    visible: cells.heading && root.sortable && root.sortColumn === index
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: 12; height: 12
                    name: root.ascending ? "chevron-up" : "chevron-down"
                    color: Theme.textDim
                }
                TapHandler { enabled: cells.heading && root.sortable; onTapped: root.sortBy(index) }
            }
        }
    }

    Cells {
        Layout.fillWidth: true
        cells: root.header
        heading: true
    }

    ListView {
        id: table
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.preferredHeight: Math.min(280, Math.max(0, root.body.length) * 30)
        model: root.body
        clip: true
        ScrollBar.vertical: ScrollBar {}
        delegate: Rectangle {
            id: line
            required property var modelData
            required property int index
            readonly property bool highlighted: root.isSelected(modelData)
            width: ListView.view.width
            implicitHeight: 30
            color: highlighted ? Theme.selection : index % 2 ? "transparent" : Theme.panelRaised
            radius: Theme.radius
            Cells {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                cells: line.modelData
            }
            TapHandler {
                enabled: root.selectable
                onTapped: root.selected(line.modelData)
            }
            HoverHandler { cursorShape: root.selectable ? Qt.PointingHandCursor : Qt.ArrowCursor }
        }
    }
}
