import QtQuick
import QtQuick.Shapes
import QtTest
import com.blockworked.Blockstitch 1.0

Rectangle {
    id: scene
    width: 320; height: 260
    color: "#27282b"

    BlockSurface {
        id: surface
        x: 10; y: 10; width: 280; height: 230
        shape: "wrap"
        fill: "#3b3c40"
        outline: "#4a4b50"
        headHeight: 50; midHeight: 34; footHeight: 26
        mouthHeights: [40, 60]
    }

    TestCase {
        name: "BlockSurface"
        when: windowShown

        function test_emptyMouths_data() {
            return [
                { tag: "curve", renderer: Shape.CurveRenderer, hovered: false },
                { tag: "curve-selected", renderer: Shape.CurveRenderer, hovered: true },
                { tag: "geometry", renderer: Shape.GeometryRenderer, hovered: false },
                { tag: "geometry-selected", renderer: Shape.GeometryRenderer, hovered: true }
            ];
        }

        function test_emptyMouths(data) {
            surface.preferredRendererType = data.renderer;
            surface.hovered = data.hovered;
            waitForRendering(surface);
            const image = grabImage(scene);
            for (const y of [70, 154]) {
                compare(image.pixel(20, y + 10), Qt.color("#3b3c40"));
                for (const x of [23, 25, 28, 100])
                    compare(image.pixel(x + 10, y + 10), Qt.color("#27282b"));
            }
        }
    }
}
