import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockstitch 1.0
Item {
    id: scene
    width: 320; height: 360
    BwDialog {
        id: dialog
        title: "Touch dialog"
        standardButtons: Dialog.Ok | Dialog.Cancel
        Column {
            width: 480
            Repeater { model: 12; BwTextField { width: 480; text: "Editable field " + index } }
        }
    }
    TestCase {
        name: "TouchDialogs"
        when: windowShown
        function init() { Motion.durationScale=0;Theme.touchMode=true; }
        function cleanup() { dialog.close();wait(20);Theme.touchMode=false; }
        function test_bounded_and_scrollable() {
            dialog.open();tryCompare(dialog,"opened",true);wait(30);
            verify(dialog.width<=scene.width);verify(dialog.height<=scene.height);
            verify(dialog.contentItem.contentHeight > dialog.contentItem.height);
            verify(dialog.contentItem.contentWidth > dialog.contentItem.width);
            dialog.contentItem.contentY=120;verify(dialog.contentItem.contentY>0);
            verify(dialog.standardButton(Dialog.Ok).height>=48);
            keyClick(Qt.Key_Escape);tryCompare(dialog,"opened",false);
        }
    }
}
