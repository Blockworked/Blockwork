import QtQuick
import QtQuick.Controls
import QtTest
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 640; height: 420
    Rectangle { id: anchor; x: 120; y: 360; width: 110; height: 34 }
    TimePicker { id: picker }
    TestCase {
        name: "TimePicker"
        when: windowShown
        function test_footer_is_in_its_own_window_data() {
            return [{tag: "12-hour", use12h: true, withDate: false},
                    {tag: "24-hour", use12h: false, withDate: false},
                    {tag: "date-time", use12h: true, withDate: true}];
        }
        function test_footer_is_in_its_own_window(data) {
            picker.use12h = data.use12h;
            picker.withDate = data.withDate;
            picker.openAt(anchor, 17, 30);
            tryCompare(picker, "opened", true);
            const popupWindow = picker.contentItem.Window.window;
            verify(popupWindow !== scene.Window.window);
            verify(popupWindow.visible);
            compare(picker.parent, anchor);
            const expected = anchor.mapToGlobal(picker.x, picker.y);
            const actual = popupWindow.contentItem.mapToGlobal(0, 0);
            verify(Math.abs(actual.x - expected.x) <= 2, "The popup stays aligned with its anchor horizontally");
            verify(Math.abs(actual.y - expected.y) <= 2, "The popup stays aligned with its anchor vertically");
            const footer = picker.contentItem.children[1];
            const bottom = footer.mapToItem(popupWindow.contentItem, 0, footer.height).y;
            verify(bottom <= popupWindow.height, "The footer fits inside the popup window");
            verify(bottom > 0);
            const setButton = footer.children.find(child => child.text === "Set");
            verify(!!setButton);
            mouseClick(setButton);
            tryCompare(picker, "visible", false);
            compare(picker.hour, 17);
            compare(picker.minute, 30);
        }
    }
}
