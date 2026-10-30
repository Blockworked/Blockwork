import QtQuick
import com.blockworked.Blockstitch 1.0

// One block a pack defines, drawn the way the palette draws it. Nothing
// here can be pressed or dragged.
Item {
    id: root
    required property var definition
    property var definitions: []
    readonly property bool reporter: definition.shape === "ReturnsValue" || definition.shape === "ReturnsBool"
    implicitWidth: loader.item ? loader.item.implicitWidth : 0
    implicitHeight: loader.item ? loader.item.implicitHeight : 0
    enabled: false

    function callInstruction(definition) {
        const branches = (definition.pieces || []).filter(p => p.kind === "Branch").length;
        const ins = { id: "library-call", type: branches ? "BranchCallBlock" : "CallBlock", block_id: definition.id, args: BlockRegistry.freshCallArgs(definition) };
        if (branches) ins.branches = Array.from({ length: branches }, () => []);
        return ins;
    }

    Loader {
        id: loader
        sourceComponent: root.reporter ? reporterBlock : callBlock
    }
    Component {
        id: callBlock
        PaletteBlock {
            instruction: root.callInstruction(root.definition)
            blockDefinitions: root.definitions
            blockColor: root.definition.color || Theme.block
        }
    }
    Component {
        id: reporterBlock
        PaletteValue {
            valueData: ({ kind: "Call", block_id: root.definition.id, args: BlockRegistry.freshCallArgs(root.definition), branches: [], saved: { kind: "Number", value: 0 } })
            editable: false; forceBoolean: root.definition.shape === "ReturnsBool"
            blockDefinitions: root.definitions
        }
    }
}
