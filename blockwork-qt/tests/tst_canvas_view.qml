import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    id: scene
    width: 1200; height: 800
    Component { id: canvasComponent; BlockCanvas { width: 1000; height: 700 } }
    TestCase {
        name: "CanvasView"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function test_left_edge_drop_keeps_view_and_grid() {
            const canvas = createTemporaryObject(canvasComponent, scene);
            wait(30);
            canvas.setZoom(0.5);
            const flick = all(canvas).find(o => o.contentX !== undefined && o.contentWidth !== undefined);
            const grid = all(canvas).find(o => o.source && String(o.source).startsWith("data:image/svg"));
            verify(!!flick); verify(!!grid);
            flick.contentX = 0;
            const point = canvas.mapToItem(null, 8, 120);
            for (let i = 0; i < 4; ++i) {
                const before = canvas.workspacePoint(point.x, point.y);
                const gridBefore = grid.mapToItem(canvas, 0, 0);
                canvas.strands = canvas.strands.concat([{id:"left" + i, x:Math.round(before.x), y:Math.round(before.y + i * 100),
                    instructions:[{id:"wait" + i,type:"Wait",duration:{kind:"Number",value:100}}]}]);
                wait(40);
                const after = canvas.workspacePoint(point.x, point.y);
                fuzzyCompare(after.x, before.x, 0.1, "left edge world x");
                fuzzyCompare(after.y, before.y, 0.1, "left edge world y");
                const gridAfter = grid.mapToItem(canvas, 0, 0);
                const phase = (delta) => ((delta / canvas.zoom % 22) + 33) % 22 - 11;
                fuzzyCompare(phase(gridAfter.x - gridBefore.x), 0, 0.1, "grid x stays anchored");
                fuzzyCompare(phase(gridAfter.y - gridBefore.y), 0, 0.1, "grid y stays anchored");
                flick.contentX = 0;
            }
        }
        function test_resize_keeps_world_point_data() {
            const rows = [];
            for (const zoom of [0.5, 1, 1.8])
                for (const sign of [-1, 1])
                    rows.push({tag: zoom + "_" + sign, zoom: zoom, sign: sign});
            return rows;
        }
        function test_resize_keeps_rendered_dots() {
            const canvas = createTemporaryObject(canvasComponent, scene);
            wait(60);
            canvas.setZoom(0.5);
            wait(30);
            const before = grabImage(canvas);
            canvas.strands = [{id:"far",x:2400,y:1800,
                instructions:[{id:"wait",type:"Wait",duration:{kind:"Number",value:100}}]}];
            wait(60);
            const grown = grabImage(canvas);
            canvas.strands = [];
            wait(60);
            const shrunk = grabImage(canvas);
            let dots = 0;
            for (let y = 50; y < 150; ++y) {
                for (let x = 50; x < 250; ++x) {
                    const color = before.pixel(x,y);
                    if (color !== before.pixel(50,50)) ++dots;
                    compare(grown.pixel(x,y),color,"dot stays fixed on growth at " + x + "," + y);
                    compare(shrunk.pixel(x,y),color,"dot stays fixed on shrink at " + x + "," + y);
                }
            }
            verify(dots > 0,"capture includes visible dots");
        }
        function test_resize_keeps_world_point(data) {
            const canvas = createTemporaryObject(canvasComponent, scene);
            wait(30);
            canvas.setZoom(data.zoom);
            const scenePoint = canvas.mapToItem(null, canvas.width / 2, canvas.height / 2);
            const before = canvas.workspacePoint(scenePoint.x, scenePoint.y);
            canvas.strands = [{id:"far", x:data.sign * 2400, y:data.sign * 1800,
                instructions:[{id:"wait",type:"Wait",duration:{kind:"Number",value:100}}]}];
            wait(30);
            const after = canvas.workspacePoint(scenePoint.x, scenePoint.y);
            fuzzyCompare(after.x, before.x, 0.1, "horizontal growth");
            fuzzyCompare(after.y, before.y, 0.1, "vertical growth");
            canvas.strands = [];
            wait(30);
            const removed = canvas.workspacePoint(scenePoint.x, scenePoint.y);
            fuzzyCompare(removed.x, before.x, 0.1, "horizontal shrink");
            fuzzyCompare(removed.y, before.y, 0.1, "vertical shrink");
        }
    }
}
