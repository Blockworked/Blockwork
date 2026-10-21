import QtQuick
import com.blockworked.Blockstitch 1.0

// Smooth wheel/touchpad scrolling for a ScrollView or ListView; declare it as a child and set flickable.
WheelHandler {
    id: handler
    // the ScrollView's contentItem, e.g. flickable: myScroll.contentItem
    property var flickable: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    orientation: Qt.Horizontal | Qt.Vertical
    target: null

    // See SmoothScroll.isTouchpad: a scroll phase or a touchpad device, never pixelDelta,
    // which mouse wheels send too on Wayland and with libinput on X11.
    function isTouchpad(event) { return smooth.isTouchpad(event); }

    // Wheel: Qt's own Flickable step (wheelScrollLines * 24 px per notch), no acceleration.
    // Touchpad pixelDelta is truncated to ints; angleDelta has the real resolution (~12 per px).
    function pan(ad, pd, touchpad) {
        if (!touchpad) return ad / 120 * Qt.styleHints.wheelScrollLines * 24;
        const d = ad !== 0 ? ad / 12 : pd;
        return d * Math.min(24, 3 + 1.5 * Math.abs(d));
    }

    // Flickable must not also handle the wheel or it overrides this
    onActiveChanged: if (flickable) flickable.interactive = !active

    onWheel: event => {
        const f = flickable;
        if (!f) return;
        // A wheel notch glides to its target; touchpad pans already arrive smooth.
        const tp = isTouchpad(event);
        smooth.scrollBy(pan(event.angleDelta.x, event.pixelDelta.x, tp), pan(event.angleDelta.y, event.pixelDelta.y, tp), !tp);
        event.accepted = true;
    }

    property SmoothScroll smooth: SmoothScroll { flickable: handler.flickable }
}
