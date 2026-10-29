import QtQuick
import com.blockworked.Android 1.0

Item {
    id: root
    property var captureResult: null
    signal canceled()
    signal failed(string error)
    signal picked(real x, real y, real width, real height)
    signal elementPicked(var selector)
    function begin(regionOnly, hint) {
        captureResult = null;
        nativePicker.begin(regionOnly ? "region" : "point", hint || "");
    }
    function beginElement() {
        captureResult = null;
        nativePicker.begin("element", "Choose an element");
    }
    NativeScreenPicker {
        id: nativePicker
        onResult: json => {
            const result = JSON.parse(json);
            if (result.error) root.failed(result.error);
            else if (result.canceled) root.canceled();
            else if (result.selector) root.elementPicked(result.selector);
            else {
                root.captureResult = result;
                root.picked(result.x, result.y, result.width, result.height);
            }
        }
    }
}
