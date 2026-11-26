import QtQuick
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

GridLayout {
    id: root
    required property var form
    required property var nodes
    // Percent heights measure the window, so they don't depend on this layout's own height.
    property real availableHeight: -1
    readonly property real heightBase: availableHeight >= 0 ? availableHeight : root.height
    // A fill-height part needs a window tall enough to fill, even when its
    // other content would size the window smaller.
    readonly property bool hasFillHeight: {
        for (const node of root.nodes) {
            const axes = WindowSizing.specs(String(root.form.properties(node).size ?? node.size ?? ""));
            if (axes.h === "fill" || WindowSizing.percent(axes.h) >= 0 || WindowSizing.weight(axes.h) > 0)
                return true;
        }
        return false;
    }
    columns: 1
    rowSpacing: 10
    columnSpacing: 10

    // Parts that stay as wide as their content unless their size says otherwise.
    function contentWidth(kind) {
        return kind === "button" || kind === "check" || kind === "icon";
    }
    // Summed weight hints, so "w:2*" gets twice the room of "w:1*".
    readonly property real widthWeightTotal: {
        let total = 0;
        for (const node of root.nodes) {
            const weight = WindowSizing.weight(WindowSizing.specs(String(root.form.properties(node).size ?? node.size ?? "")).w);
            if (weight > 0) total += weight;
        }
        return total;
    }
    readonly property real heightWeightTotal: {
        let total = 0;
        for (const node of root.nodes) {
            const weight = WindowSizing.weight(WindowSizing.specs(String(root.form.properties(node).size ?? node.size ?? "")).h);
            if (weight > 0) total += weight;
        }
        return total;
    }

    Repeater {
        model: root.nodes
        delegate: Loader {
            id: part
            required property var modelData
            required property int index
            readonly property var properties: root.form.properties(modelData)
            readonly property var axes: WindowSizing.specs(String(properties.size ?? modelData.size ?? ""))
            readonly property real widthPercent: WindowSizing.percent(axes.w)
            readonly property real widthPixels: WindowSizing.pixels(axes.w)
            readonly property real widthWeight: WindowSizing.weight(axes.w)
            readonly property real heightPercent: WindowSizing.percent(axes.h)
            readonly property real heightPixels: WindowSizing.pixels(axes.h)
            readonly property real heightWeight: WindowSizing.weight(axes.h)
            readonly property bool fillWidth: axes.w === "fill" || widthPercent >= 0 || widthWeight > 0
                || (axes.w === "" && !root.contentWidth(modelData.kind))
            readonly property bool fillHeight: axes.h === "fill" || heightPercent >= 0 || heightWeight > 0
                || (axes.h === "" && modelData.kind === "scroll")
            visible: properties.visible ?? true
            enabled: properties.enabled ?? true
            Layout.fillWidth: fillWidth
            Layout.fillHeight: fillHeight
            Layout.minimumWidth: 0
            Layout.alignment: Qt.AlignLeft | (fillHeight ? 0 : Qt.AlignTop)
            Layout.preferredWidth: widthPixels >= 0 ? widthPixels
                : widthPercent >= 0 ? root.width * widthPercent / 100
                : widthWeight > 0 && root.widthWeightTotal > 0 ? root.width * widthWeight / root.widthWeightTotal
                : implicitWidth
            Layout.maximumWidth: widthPercent >= 0 ? root.width * widthPercent / 100 : Number.POSITIVE_INFINITY
            Layout.preferredHeight: heightPixels >= 0 ? heightPixels
                : heightPercent >= 0 ? root.heightBase * heightPercent / 100
                : heightWeight > 0 && root.heightWeightTotal > 0 ? root.heightBase * heightWeight / root.heightWeightTotal
                : implicitHeight
            Layout.maximumHeight: heightPercent >= 0 ? root.heightBase * heightPercent / 100 : Number.POSITIVE_INFINITY
            Rectangle {
                visible: !!root.form.request.preview && root.form.selectionPath.slice(-1)[0] === modelData.id
                anchors.fill: parent
                anchors.margins: -3
                color: "transparent"
                radius: Theme.radius
                border.color: Theme.accent
                z: 2
            }
            Component.onCompleted: setSource(Qt.resolvedUrl("MacroFormControl.qml"), {
                form: root.form, node: modelData, nodeIndex: index
            })
        }
    }
}
