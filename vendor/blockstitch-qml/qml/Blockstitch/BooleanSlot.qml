import QtQuick
import QtQuick.Shapes

// Empty boolean socket. It accepts drops through its owner's value location.
Shape {
    id: root
    property bool dropHighlighted: false
    implicitWidth: 48
    implicitHeight: 27
    preferredRendererType: Shape.CurveRenderer
    readonly property string outline: {
        const w = width, h = height, n = Math.min(h * .32, w / 2);
        return "M" + n + " .5 L" + (w - n) + " .5 L" + (w - .5) + " " + h / 2
            + " L" + (w - n) + " " + (h - .5) + " L" + n + " " + (h - .5) + " L.5 " + h / 2 + " Z";
    }
    ShapePath {
        strokeColor: root.dropHighlighted ? Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, .35) : "transparent"
        strokeWidth: root.dropHighlighted ? 6 : 0
        fillColor: "transparent"
        joinStyle: ShapePath.RoundJoin
        PathSvg { path: root.outline }
    }
    ShapePath {
        strokeColor: root.dropHighlighted ? Theme.accent : Theme.border
        strokeWidth: root.dropHighlighted ? 2.4 : 1
        fillColor: Theme.field
        joinStyle: ShapePath.RoundJoin
        PathSvg { path: root.outline }
    }
}
