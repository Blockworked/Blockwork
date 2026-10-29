import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1100; height: 600
    property var edited: null

    readonly property var pickComponent: ({ key: "pick", label: "which match", icon: "list-ordered",
        fresh: () => ({ mode: "Nth", index: { kind: "Number", value: 1 } }),
        pieces: [{ kind: "label", text: "match" }, { kind: "dropdown", key: "pick", index: "mode",
                   options: [{ value: "Nth", label: "number" }, { value: "NearMouse", label: "nearest mouse" }] }] })
    readonly property var regionComponent: ({ key: "region", label: "search region", fresh: "",
        pieces: [{ kind: "label", text: "in region" }, { kind: "text", key: "region" }] })
    readonly property var spec: ({ icon: "search", head: [{ kind: "label", text: "find" }, { kind: "components" }, { kind: "label", text: "now" }],
        components: [pickComponent, regionComponent] })

    InstructionBlock {
        id: block
        x: 20; y: 20
        strandId: "s1"; path: [{ index: 0 }]
        instruction: ({ id: "f1", type: "TestFind" })
        onInstructionEdited: (sid, p, n) => { scene.edited = n; block.instruction = n; }
    }

    PalettePanel {
        id: palette
        x: 400; width: 650; height: 600
        instructionTypes: ["TestFind"]
    }
    SignalSpy { id: activatedSpy; target: palette; signalName: "blockActivated" }
    SignalSpy { id: draggedSpy; signalName: "dragStarted" }

    TestCase {
        name: "BlockComponents"
        when: windowShown

        function initTestCase() { BlockRegistry.registerRows({ TestFind: scene.spec }); }
        function init() { scene.edited = null; block.instruction = { id: "f1", type: "TestFind" }; }

        function kinds(pieces) { return pieces.map(p => p.kind === "label" ? p.text : p.kind); }

        function descendants(item) {
            let result = [item];
            for (const child of item.children || []) result = result.concat(descendants(child));
            return result;
        }

        function test_paletteComponentsCanBeAddedRemovedAndInserted() {
            const entry = descendants(palette).find(o => o.spec && o.spec.kind === "instruction");
            verify(!!entry);
            entry.instruction = { id: "palette-TestFind", type: "TestFind" };
            const row = descendants(entry).find(o => o.paletteMode === true);
            const plus = descendants(entry).find(o => o.objectName === "addComponentButton");
            verify(!!plus);
            mouseClick(plus);
            const menu = findChild(plus, "addComponentMenu");
            verify(!!menu);
            tryCompare(menu, "count", 2);
            tryCompare(menu, "opened", true);
            mouseClick(menu.itemAt(0));
            tryVerify(() => !!entry.instruction.pick);
            menu.close();
            compare(entry.instruction.pick.mode, "Nth");
            compare(entry.spec.instruction.pick.mode, "Nth");
            verify(!("pick" in palette.prefab("TestFind")));

            activatedSpy.clear();
            entry.activated();
            compare(activatedSpy.count, 1);
            compare(activatedSpy.signalArguments[0][1].pick.mode, "Nth");
            draggedSpy.target = entry;
            draggedSpy.clear();
            row.dragBegan("", [], 0, 20, 20, 4, 4, row.width, row.height);
            compare(draggedSpy.count, 1);
            compare(draggedSpy.signalArguments[0][0].instruction.pick.mode, "Nth");
            entry.dragCanceled();

            const remove = descendants(entry).find(o => o.objectName === "removeComponentButton");
            verify(!!remove);
            tryCompare(menu, "visible", false);
            waitForRendering(remove);
            mouseClick(remove);
            verify(!("pick" in entry.instruction));
            compare(kinds(row.headPieces), ["find", "addComponent", "now"]);
        }

        function test_plainBlockShowsOnlyTheHeadAndThePlus() {
            compare(kinds(block.headPieces), ["find", "addComponent", "now"]);
            compare(kinds(BlockRegistry.rowPieces(scene.spec, block.instruction, true)), ["find", "addComponent", "now"]);
        }

        function test_addingAComponentFillsItsFreshValue() {
            block.addComponent(scene.pickComponent);
            compare(scene.edited.pick, { mode: "Nth", index: { kind: "Number", value: 1 } });
            compare(kinds(block.headPieces), ["find", "match", "dropdown", "removeComponent", "addComponent", "now"]);
        }

        function test_componentsKeepDeclaredOrderAndThePlusGoesWhenAllAreOn() {
            block.addComponent(scene.regionComponent);
            block.addComponent(scene.pickComponent);
            compare(kinds(block.headPieces), ["find", "match", "dropdown", "removeComponent", "in region", "text", "removeComponent", "now"]);
            compare(BlockRegistry.missingComponents(scene.spec, block.instruction).length, 0);
        }

        function test_removingTakesTheKeyOut() {
            block.addComponent(scene.regionComponent);
            block.removeComponent(scene.regionComponent);
            verify(!("region" in scene.edited));
            compare(kinds(block.headPieces), ["find", "addComponent", "now"]);
        }

        function test_aNamedIndexEditsOneFieldOfTheComponent() {
            block.addComponent(scene.pickComponent);
            block.setPieceField(scene.pickComponent.pieces[1], "NearMouse");
            compare(scene.edited.pick.mode, "NearMouse");
            compare(scene.edited.pick.index, { kind: "Number", value: 1 });
            compare(block.pieceValue(scene.pickComponent.pieces[1]), "NearMouse");
        }

        function test_piecesStayTheSameObjectsAcrossEdits() {
            block.addComponent(scene.pickComponent);
            const before = block.headPieces;
            block.setPieceField(scene.pickComponent.pieces[1], "NearMouse");
            verify(block.headPieces === before);
        }

        function test_aRowWithoutComponentsIsUnchanged() {
            const plain = { head: [{ kind: "label", text: "a" }, { kind: "label", text: "b", when: i => !!i.b }] };
            compare(kinds(BlockRegistry.rowPieces(plain, { b: false }, false)), ["a"]);
            compare(kinds(BlockRegistry.rowPieces(plain, { b: true }, false)), ["a", "b"]);
        }
    }
}
