import QtQuick

// Faster wheel/touchpad scrolling for a ScrollView; declare it as a child and set flickable.
WheelHandler {
    id: handler
    // the ScrollView's contentItem, e.g. flickable: myScroll.contentItem
    property var flickable: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    orientation: Qt.Horizontal | Qt.Vertical
    target: null

    // Touchpads send pixelDelta, a scroll phase or come from a touchpad device; a mouse
    // wheel sends none of those, only angleDelta in steps of 120 per notch (less on hi-res wheels).
    function isTouchpad(event) {
        return event.pixelDelta.x !== 0 || event.pixelDelta.y !== 0 || event.phase !== Qt.NoScrollPhase
            || (event.device && event.device.type === PointerDevice.TouchPad);
    }

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
        const tp = isTouchpad(event);
        let dx = pan(event.angleDelta.x, event.pixelDelta.x, tp), dy = pan(event.angleDelta.y, event.pixelDelta.y, tp);
        f.contentY = Math.max(0, Math.min(Math.max(0, f.contentHeight - f.height), f.contentY - dy));
        f.contentX = Math.max(0, Math.min(Math.max(0, f.contentWidth - f.width), f.contentX - dx));
        event.accepted = true;
    }
}
