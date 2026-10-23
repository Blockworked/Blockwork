import QtQuick
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

Rectangle {
    id: root
    property string title: ""
    property string iconName: ""
    default property alias content: body.data
    implicitHeight: header.height + body.implicitHeight + 36
    radius: 16
    color: Theme.field
    border.color: Theme.borderSoft

    Rectangle {
        id: header
        width: parent.width; height: 48
        color: Theme.panelRaised
        border.color: Theme.border
        radius: root.radius
        Rectangle { anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom; height: root.radius; color: parent.color }
        Row { anchors.left: parent.left; anchors.leftMargin: 18; anchors.verticalCenter: parent.verticalCenter; spacing: 9
            LucideIcon { name: root.iconName; color: Theme.textDim; width: 16; height: 16; anchors.verticalCenter:parent.verticalCenter }
            Text { text: root.title.toUpperCase(); color: Theme.textDim; font.pixelSize: 12; font.weight: Font.Bold; font.letterSpacing: 1.2; anchors.verticalCenter:parent.verticalCenter }
        }
    }
    ColumnLayout { id: body; anchors.left: parent.left; anchors.right: parent.right; anchors.top: header.bottom; anchors.margins: 18; spacing: 12 }
}
