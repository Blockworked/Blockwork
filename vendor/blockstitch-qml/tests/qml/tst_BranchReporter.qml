import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1200; height: 900
    property var definition: ({ id: "custom", shape: "ReturnsBool", pieces: [
        { kind: "Label", text: "custom reporter" }, { kind: "Branch", name: "first" }
    ] })
    ValueChip {
        id: chip
        x: 20; y: 20
        valueData: ({ kind: "Call", block_id: "custom", args: [] })
        blockDefinitions: [definition]
    }
    InstructionBlock {
        id: header
        x: 20; y: 380
        instruction: ({ type: "BlockHeader", block_id: "custom" })
        blockDefinitions: [definition]
    }
    property var dropped: null
    BlockCanvas {
        id: canvas
        x: 550; width: 650; height: 900
        blockDefinitions: [definition]
        floatingValues: [{ id: "floating", x: 0, y: 0, value: { kind: "Call", block_id: "custom", args: [], branches: [] } }]
    }
    PaletteDragController {
        id: controller
        anchors.fill: parent
        canvas: canvas
        holdLanding: false
        submit: (edit, reply) => { scene.dropped = edit; reply(true); }
    }
    TestCase {
        name: "BranchReporter"
        when: windowShown
        function descendants(item) {
            let result = [item];
            for (const child of item.children || []) result = result.concat(descendants(child));
            return result;
        }
        function test_paletteDropIntoEmptyReporterBranch() {
            definition = { id: "custom", shape: "ReturnsBool", pieces: [
                { kind: "Label", text: "reporter" }, { kind: "Branch", name: "first" }
            ] };
            waitForRendering(canvas);
            const containers = canvas.collectContainers();
            const branch = containers.find(c => c.strandId.indexOf("@reporter-branch:") === 0);
            verify(!!branch);
            compare(branch.list.length, 0);
            const spec = { kind: "instruction", instruction: { id: "palette", type: "Wait", duration: { kind: "Number", value: 10 } } };
            // The ghost's top-left lands at the mouth's connector.
            const mouth = descendants(canvas).find(o => o.isReporterBranch && o.branchStrandId === branch.strandId);
            const point = mouth.mapToItem(null, 0, 0);
            controller.begin(spec, point.x, point.y, 0, 0);
            verify(canvas.paletteSnapValid);
            compare(canvas.paletteSnapTargetId, branch.strandId);
            controller.end(point.x, point.y);
            compare(dropped.kind, "insert_instruction");
            compare(dropped.strandId, branch.strandId);
            compare(dropped.path, [{ index: 0 }]);
            canvas.floatingValues = [{ id: "floating", x: 0, y: 0, value: { kind: "Call", block_id: "custom", args: [], branches: [[dropped.instruction]] } }];
            tryVerify(() => !!canvas.collectContainers().find(c => c.strandId === branch.strandId && c.list.length === 1));
            const instruction = canvas.findBlockItem(canvas, dropped.instruction.id);
            verify(!!instruction);
            compare(instruction.strandId, branch.strandId);
            verify(instruction.dragState !== null);
            const location = instruction.fieldLocation("WaitDuration");
            compare(location.strand_id, branch.strandId);
        }
        function test_structure_data() {
            return [
                { tag: "boolean-one", shape: "ReturnsBool", count: 1 },
                { tag: "value-one", shape: "ReturnsValue", count: 1 },
                { tag: "boolean-two", shape: "ReturnsBool", count: 2 },
                { tag: "value-two", shape: "ReturnsValue", count: 2 }
            ];
        }
        function test_structure(data) {
            const pieces = [{ kind: "Label", text: "custom reporter" }, { kind: "Branch", name: "first" }];
            if (data.count === 2) pieces.push({ kind: "Label", text: "otherwise" }, { kind: "Branch", name: "a much longer second branch name" });
            definition = { id: "custom", shape: data.shape, pieces: pieces };
            tryCompare(chip, "hasBranches", true);
            waitForRendering(chip);
            const surface = findChild(chip, "reporterBranchSurface");
            verify(!!surface);
            compare(surface.shape, "wrap");
            compare(surface.headNotch, false);
            compare(surface.footTab, false);
            compare(surface.mouthHeights.length, data.count);
            compare(descendants(chip).filter(o => o.objectName === "reporterBranchPiece").length, 0);
            compare(surface.mouthHeights[0], 26);
            if (data.shape === "ReturnsBool") {
                compare(chip.branchPadX, chip.fullHeight / 2);
                compare(header.pvPadX, header.pvOuterHeight / 2);
            }
            const image = grabImage(chip);
            // The nested C-block and branch have distinct fills.
            compare(image.pixel(Math.round(chip.branchPadX + 5), 35), Qt.color(Theme.block));
            compare(image.pixel(Math.round(chip.branchPadX + 30), 85), Qt.color(Theme.field));
        }
    }
}
