import QtQuick
import QtTest
import com.blockworked.Blockstitch

TestCase {
    name: "BlockRegistry"

    function test_aNeutralHostAdvertisesNoHostFeatures() {
        compare(BlockRegistry.recordingTargets, false);
        for (const op of ["BatteryPercentage", "PluggedIn", "ClipboardText", "ClipboardHasImage", "ClipboardHasFiles"])
            compare(BlockRegistry.operator(op), null, op);
        compare(BlockRegistry.iconFor("MoveMouse"), "blocks");
        compare(BlockRegistry.iconFor("WhenRan"), "blocks");
        compare(BlockRegistry.iconFor("If"), "git-branch");
        compare(BlockRegistry.descriptor("BatteryPercentage"), null);
    }

    function test_freshValuesAreTheCoreDefaults() {
        const letter = BlockRegistry.freshValue("LetterOf");
        compare(letter.op, "LetterOf");
        compare(letter.args, [{ kind: "Number", value: 1 }, { kind: "Text", value: "" }]);
        compare(letter.saved, { kind: "Number", value: 0 });
        compare(BlockRegistry.freshValue("Join3").op, "Join");
        compare(BlockRegistry.freshValue("Join3").args.length, 3);
        compare(BlockRegistry.freshValue("And").args, [{ kind: "Bool" }, { kind: "Bool" }]);
        compare(BlockRegistry.freshValue("Math").args[0], { kind: "Text", value: "Abs" });
        compare(BlockRegistry.freshValue("Case").args[1], { kind: "Text", value: "Upper" });
        compare(BlockRegistry.freshValue("True").args, []);
        compare(BlockRegistry.freshValue("NoSuchKind"), null);
        // Every boolean-result built-in draws as one.
        for (const kind in BlockRegistry.descriptors) {
            const d = BlockRegistry.descriptor(kind);
            compare(BlockRegistry.isBoolOp(d.op), d.result === "bool", kind);
        }
    }

    function test_nameSlotsTakeTheFirstName() {
        compare(BlockRegistry.freshValue("ListItem", { lists: ["scores", "names"] }).args,
                [{ kind: "Number", value: 1 }, { kind: "Text", value: "scores" }]);
        compare(BlockRegistry.freshValue("DictHasKey", { dicts: ["d"] }).args,
                [{ kind: "Text", value: "d" }, { kind: "Text", value: "key" }]);
        compare(BlockRegistry.freshValue("ListLength", { lists: [] }).args, [{ kind: "Text", value: "" }]);
    }

    function test_hostDescriptorsAndDropdownDefaults() {
        BlockRegistry.registerDescriptors([
            { kind: "Pick", op: "Pick", arity: 2, host: true,
              args: [{ type: "text", default: { kind: "Text", value: "" } }, { type: "text", default: { kind: "Text", value: "keep" } }] }
        ]);
        BlockRegistry.registerOperators({ Pick: { prefix: "pick", result: "text", enumArg: { index: 0, options: [{ value: "first", label: "first" }] } } });
        compare(BlockRegistry.freshValue("Pick").args, [{ kind: "Text", value: "first" }, { kind: "Text", value: "keep" }]);
        BlockRegistry.registerIcons({ Pick: "hand" });
        compare(BlockRegistry.iconFor("Pick"), "hand");
        verify(BlockRegistry.descriptor("Add") !== null);
    }

    function test_freshCallArgsUseEachInputsDefault() {
        const def = { pieces: [
            { kind: "Label", text: "move" },
            { kind: "Input", value_type: "Number", default_value: { kind: "Number", value: 10 } },
            { kind: "Input", value_type: "Bool", default_value: { kind: "Bool", value: true } },
            { kind: "Branch" },
            { kind: "Input", value_type: "Text", default_value: { kind: "Text", value: "hi" } },
            { kind: "Input", value_type: "Bool" },
            { kind: "Input", value_type: "Number", default_value: { kind: "Number", value: NaN } }
        ] };
        compare(BlockRegistry.freshCallArgs(def), [
            { kind: "Number", value: 10 },
            { kind: "Op", op: "True", args: [], saved: { kind: "Number", value: 0 } },
            { kind: "Text", value: "hi" },
            { kind: "Bool" },
            { kind: "Number", value: 0 }
        ]);
        compare(BlockRegistry.freshCallArgs(null), []);
    }
}
