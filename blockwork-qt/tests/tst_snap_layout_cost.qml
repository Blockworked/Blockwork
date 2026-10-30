import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1200; height: 800
    Component {
        id: lookupCanvasComponent
        BlockCanvas {
            property var branchScans: []
            function branchContainers(node, out) { branchScans.push(node); }
        }
    }
    Component {
        id: canvasComponent
        BlockCanvas {
            width: scene.width; height: scene.height
            property var searches: []
            function findBlockItem(item, id) {
                searches.push(id);
                return null;
            }
        }
    }
    TestCase {
        name: "SnapLayoutCost"
        when: windowShown
        function all(item) {
            let result = [item];
            for (const child of item.children || []) result = result.concat(all(child));
            return result;
        }
        function block(canvas, id) {
            return all(canvas).find(item => item.instruction && item.instruction.id === id && item.paletteMode === false);
        }
        function test_missing_strands_do_not_search_reporter_branches() {
            const canvas = createTemporaryObject(lookupCanvasComponent, scene);
            canvas.strands = [{id:"present",x:0,y:0,instructions:[]}];
            canvas.branchScans = [];
            for (const id of ["", null, undefined, "deleted-strand"])
                compare(canvas.strandById(id), null);
            compare(canvas.strandById("present").id, "present");
            compare(canvas.branchScans.length, 0, "empty and stale strand IDs cannot name reporter branches");
            compare(canvas.strandById("@reporter-branch:missing"), null);
            verify(canvas.branchScans.length > 0, "reporter branch IDs still use branch lookup");
        }
        function test_nested_attach_uses_reported_height() {
            const canvas = createTemporaryObject(canvasComponent, scene);
            canvas.strands = [{id:"main",x:0,y:1000,instructions:[
                {id:"loop",type:"While",condition:{kind:"Bool"},body:[
                    {id:"if",type:"If",condition:{kind:"Bool"},body:[]}]}]}];
            wait(30);
            const before = canvas.needHalfH;
            canvas.searches = [];
            const updated = JSON.parse(JSON.stringify(canvas.strands));
            updated[0].instructions[0].body[0].body = [{id:"attached",type:"Wait",duration:{kind:"Number",value:100}}];
            canvas.strands = updated;
            wait(30);
            compare(canvas.searches.length, 0, "sizing must not search the control tree as a nested block initializes");
            const loop = block(canvas, "loop");
            compare(canvas.needHalfH, 1000 + loop.parent.implicitHeight);
            verify(canvas.needHalfH > before, "canvas includes the new nested content");
            const attached = block(canvas, "attached");
            verify(!!attached);
            compare(attached.path, [{index:0,slot:0},{index:0,slot:0},{index:0}]);

            const edited = JSON.parse(JSON.stringify(updated));
            edited[0].instructions[0].body[0].body[0].duration.value = 500;
            canvas.variables = ["new variable"];
            canvas.strands = edited;
            wait(30);
            compare(attached.instruction.duration.value, 500);
            compare(attached.variables, ["new variable"], "nested properties remain bound after loading");
            canvas.strands = [{id:"main",x:0,y:1000,instructions:[]}];
            wait(30);
            compare(canvas.needHalfH, 1000, "empty strands report zero height");
            canvas.strands = [];
            wait(30);
            compare(canvas.needHalfH, 0, "removed strands no longer affect canvas height");
        }
    }
}
