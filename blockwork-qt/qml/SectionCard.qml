import QtQuick
import QtQuick.Layouts

Rectangle {
    id: root
    property string title: ""
    property string glyph: ""
    default property alias content: body.data
    implicitHeight: header.height + body.implicitHeight + 20
    radius: 16
    color: "#292a2d"
    border.color: "#3d3e42"

    Rectangle {
        id: header
        width: parent.width; height: 48
        color: "#343538"
        border.color: "#414247"
        radius: root.radius
        Rectangle { anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom; height: root.radius; color: parent.color }
        Row { anchors.left: parent.left; anchors.leftMargin: 18; anchors.verticalCenter: parent.verticalCenter; spacing: 9
            Text { text: root.glyph; color: "#9da0a8"; font.pixelSize: 16 }
            Text { text: root.title.toUpperCase(); color: "#a7a8ae"; font.pixelSize: 12; font.weight: Font.Bold; font.letterSpacing: 1.2 }
        }
    }
    ColumnLayout { id: body; anchors.left: parent.left; anchors.right: parent.right; anchors.top: header.bottom; anchors.margins: 18; spacing: 12 }
}
