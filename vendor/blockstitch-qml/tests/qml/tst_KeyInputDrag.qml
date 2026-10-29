import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0

Item {
    width: 500; height: 400

    Flickable {
        id: flick
        anchors.fill: parent
        contentWidth: 1000; contentHeight: 1200
        acceptedButtons: Qt.LeftButton
        ValueChip {
            id: key
            x: 300; y: 300
            width: implicitWidth; height: implicitHeight
            keyMode: true; paletteMode: true
            valueData: ({kind: "Text", value: "a"})
        }
    }
    SignalSpy { id: capture; target: key; signalName: "keyCaptureRequested" }

    TestCase {
        name: "KeyInputDrag"
        when: windowShown
        function init() {
            flick.cancelFlick();
            flick.contentX = 200;
            flick.contentY = 200;
            capture.clear();
        }
        function test_drag_does_not_pan_data() {
            return [{tag: "down_right", dx: 80, dy: 80}, {tag: "up_left", dx: -80, dy: -80}];
        }
        function test_drag_does_not_pan(data) {
            mouseDrag(key, key.width / 2, key.height / 2, data.dx, data.dy, Qt.LeftButton, Qt.NoModifier, 20);
            wait(100);
            compare(flick.contentX, 200);
            compare(flick.contentY, 200);
            compare(capture.count, 0);
        }
        function test_click_starts_key_capture() {
            mouseClick(key, key.width / 2, key.height / 2);
            compare(capture.count, 1);
        }
        function test_background_still_pans() {
            mouseDrag(flick, 300, 250, -80, -80, Qt.LeftButton, Qt.NoModifier, 20);
            verify(flick.contentX > 200);
            verify(flick.contentY > 200);
            compare(capture.count, 0);
        }
    }
}
