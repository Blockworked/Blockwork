import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockwork 1.0

Item {
    width: 400; height: 300

    Component {
        id: windowComponent
        ApplicationWindow {
            width: 400; height: 300
            visible: true
            color: "white"
            property real shadowMargin: 18
            property real cornerRadius: 10
            property alias dialog: dialog
            AppDialog { id: dialog; width: 100; height: 100 }
        }
    }

    TestCase {
        name: "DialogBackdrop"
        when: windowShown

        function test_frame_bounds() {
            const window = createTemporaryObject(windowComponent, null);
            verify(window);
            window.dialog.open();
            tryCompare(window.dialog, "opened", true);
            wait(400);
            let image = grabImage(window.contentItem);
            compare(image.pixel(2, 150), Qt.color("white"), "shadow padding stays clear");
            compare(image.pixel(18, 18), Qt.color("white"), "rounded corner stays clear");
            verify(image.pixel(35, 150) !== Qt.color("white"), "frame interior is dimmed");

            window.shadowMargin = 0;
            window.cornerRadius = 0;
            waitForRendering(window.contentItem);
            image = grabImage(window.contentItem);
            verify(image.pixel(2, 150) !== Qt.color("white"), "maximized frame dims to the edge");
            window.close();
        }
    }
}
