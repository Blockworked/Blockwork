import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1200; height: 800
    property var calls: []
    function invoke(command, args) { calls.push({command: command, args: args}); }
    MockBridge { id: bridge; silent: true }
    readonly property var stopBlock: ({ id: "open", type: "Stop", scope: "All" })
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: ({
                current_macro: {id:"macro", name:"Test", speed_multiplier:1, settings:{always_listen:false}, signals:[], variables:[], lists:[], comments:[],
                    floating_values:[], block_defs:[],
                    strands:[{id:"strand",x:40,y:40,instructions:[{id:"run",type:"WhenRan"}, scene.stopBlock]}]},
                recording_phase:{phase:"Idle"}, key_capture:null, can_undo:false, can_redo:false,
                record_mouse_movement:false, record_mouse_relative:false, absolute_mouse_position_available:true
            })
        }
    }
    Component {
        id: blockComponent
        InstructionBlock {
            instruction: scene.stopBlock
            strandId: "strand"
            path: [{index: 1}]
            onInstructionEdited: (strand, address, edited) => {
                scene.calls.push({strand: strand, path: address, instruction: edited});
                instruction = edited;
            }
        }
    }
    TestCase {
        name: "StopBlock"
        when: windowShown
        function descendants(item) {
            let result = [item];
            for (const child of item.children || []) result = result.concat(descendants(child));
            return result;
        }
        function test_dropdown_edits_scope_and_updates_connector() {
            createTemporaryObject(editorComponent, scene);
            const block = createTemporaryObject(blockComponent, scene);
            verify(block);
            const dropdown = descendants(block).find(o => o.choices && o.choices.length === 3);
            verify(!!dropdown);
            compare(dropdown.displayText, "all");
            verify(block.isCap);
            scene.calls = [];
            for (const index of [2, 1, 0]) {
                dropdown.currentIndex = index;
                dropdown.activated(index);
                compare(scene.calls[scene.calls.length - 1].instruction.scope, dropdown.choices[index].value);
                compare(block.isCap, index !== 2);
            }
            compare(scene.calls[0].strand, "strand");
            compare(scene.calls[0].path, [{index: 1}]);
        }
        function test_scopes_and_connectors() {
            const page = createTemporaryObject(editorComponent, scene);
            compare(page.defaultInstruction("Stop").scope, "All");
            verify(page.catalog.paletteSections.some(section => section.items.indexOf("Stop") >= 0));
            verify(page.catalog.paletteSections.some(section => section.items.indexOf("StopMacroNamed") >= 0));
            const row = BlockRegistry.row("Stop");
            compare(row.head[0].text, "stop");
            const dropdown = row.head.find(p => p.kind === "dropdown");
            compare(dropdown.key, "scope");
            compare(dropdown.options.map(o => o.label), ["all", "this strand", "other strands in macro"]);
            for (const option of dropdown.options) {
                const ins = {type: "Stop", scope: option.value};
                compare(BlockRegistry.isCap(ins, []), option.value !== "OtherStrands");
            }
            const named = BlockRegistry.row("StopMacroNamed");
            compare(named.head[0].text, "stop macro");
            compare(named.head[1].key, "target");
        }
    }
}
