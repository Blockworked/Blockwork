import QtQuick

// Sidebar launcher for a document's lists or dicts: one checkable row per
// collection that toggles its canvas monitor, with a rename/delete menu.
Column {
    id: root
    // [{name, editor_visible, editor_x, editor_y, items | entries}]
    property var collections: []
    property string noun: "list"
    property string emptyText: ""
    property real rowWidth: 230
    signal editorStateRequested(string name, bool visible, int x, int y)
    signal renameRequested(string name)
    signal deleteRequested(string name)
    signal detailsRequested(string name)
    spacing: 2

    readonly property var sorted: [...(collections || [])].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }))
    function countOf(c) { return (c.items || c.entries || []).length; }

    Text {
        visible: !root.sorted.length && root.emptyText.length > 0
        text: root.emptyText; width: root.rowWidth; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
    }
    Repeater {
        model: root.sorted
        delegate: Rectangle {
            id: row; required property var modelData
            width: root.rowWidth; height: 30; radius: Theme.radius; color: hover.hovered ? Theme.panelRaised : "transparent"
            Row {
                anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; spacing: 7
                BwCheckBox {
                    anchors.verticalCenter: parent.verticalCenter; checked: row.modelData.editor_visible
                    Accessible.name: qsTr("Show %1 on canvas").arg(row.modelData.name)
                    onToggled: root.editorStateRequested(row.modelData.name, checked, row.modelData.editor_x || 36, row.modelData.editor_y || 36)
                }
                Text { anchors.verticalCenter: parent.verticalCenter; text: row.modelData.name; color: Theme.text; font.pixelSize: 12; width: Math.max(80, row.width - 100); elide: Text.ElideRight }
                Text { anchors.verticalCenter: parent.verticalCenter; text: String(root.countOf(row.modelData)); color: Theme.textDim; font.pixelSize: 11 }
            }
            HoverHandler { id: hover }
            TapHandler { acceptedButtons: Qt.RightButton; gesturePolicy: TapHandler.ReleaseWithinBounds; onTapped: menu.popup() }
            BwMenu {
                id: menu
                BwMenuItem { iconName: "equal"; text: root.noun === "dict" ? qsTr("Rename dict") : root.noun === "variable" ? qsTr("Rename variable") : qsTr("Rename list"); onTriggered: root.renameRequested(row.modelData.name) }
                BwMenuItem { iconName: "info"; text: qsTr("Details"); onTriggered: root.detailsRequested(row.modelData.name) }
                BwMenuItem { iconName: "trash"; danger: true; text: root.noun === "dict" ? qsTr("Delete dict") : root.noun === "variable" ? qsTr("Delete variable") : qsTr("Delete list"); onTriggered: root.deleteRequested(row.modelData.name) }
            }
        }
    }
}
