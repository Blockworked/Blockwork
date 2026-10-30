import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1000
    height: 760

    Component { id: editorFactory; BlockDefinitionEditor { width: 1000; height: 760 } }

    TestCase {
        name: "DefinitionSeparatorSelection"
        when: windowShown

        function descendants(item) {
            let result = [item];
            for (const child of item.children || [])
                result = result.concat(descendants(child));
            return result;
        }

        function test_separator_clears_previous_selection_data() {
            return [
                {tag: "branch_to_empty_label", previous: 2, label: ""},
                {tag: "header_to_existing_label", previous: 0, label: "otherwise"}
            ];
        }

        function test_separator_clears_previous_selection(data) {
            const editor = createTemporaryObject(editorFactory, scene);
            editor.load({pieces: [
                {kind: "Label", text: "example"},
                {kind: "Branch", name: "branch1"},
                {kind: "Branch", name: "branch2"}
            ]});
            if (data.label.length)
                editor.commitSeparator(0, data.label);
            const previous = editor.findPiece(editor, data.previous);
            verify(!!previous);
            previous.startEdit();
            tryCompare(editor, "editingPiece", data.previous);
            tryVerify(() => editor.activeEditor && editor.activeEditor.activeFocus);
            const separator = descendants(editor).find(o => o.placeholderText === "Add label" && o.visible);
            verify(!!separator);
            mouseClick(separator);
            tryCompare(separator, "activeFocus", true);
            compare(editor.selectedPiece, -1);
            compare(editor.editingPiece, -1);
            verify(!previous.selected);
            separator.text = "else";
            editor.commitSeparatorEdits();
            compare(editor.resultPieces().find(p => p.kind === "Label" && p.text === "else").text, "else");
        }
    }
}
