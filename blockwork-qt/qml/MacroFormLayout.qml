import QtQuick
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

GridLayout {
    id: root
    required property var form
    required property var nodes
    columns: 1
    rowSpacing: 10
    columnSpacing: 10

    Repeater {
        model: root.nodes
        delegate: Loader {
            required property var modelData
            required property int index
            readonly property var properties: root.form.properties(modelData)
            visible: properties.visible ?? true
            enabled: properties.enabled ?? true
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            Layout.minimumWidth: 0
            Layout.preferredWidth: modelData.kind === "spacer" ? 1 : Math.min(implicitWidth, 160)
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
