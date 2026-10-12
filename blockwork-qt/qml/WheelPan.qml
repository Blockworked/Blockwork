import QtQuick

// Faster wheel/touchpad scrolling for a ScrollView; declare it as a child and set flickable.
WheelHandler {
    id: handler
    // the ScrollView's contentItem, e.g. flickable: myScroll.contentItem
    property var flickable: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    orientation: Qt.Horizontal | Qt.Vertical
    target: null

    // Touchpad pixelDelta is truncated to ints; angleDelta has the real resolution (~12 per px).
    function pan(ad, pd) {
        if (ad !== 0 && ad % 120 === 0) return ad * 2;
        const d = ad !== 0 ? ad / 12 : pd;
        return d * Math.min(24, 3 + 1.5 * Math.abs(d));
    }

    // Flickable must not also handle the wheel or it overrides this
    onActiveChanged: if (flickable) flickable.interactive = !active

    onWheel: event => {
        const f = flickable;
        if (!f) return;
        let dx = pan(event.angleDelta.x, event.pixelDelta.x), dy = pan(event.angleDelta.y, event.pixelDelta.y);
        f.contentY = Math.max(0, Math.min(Math.max(0, f.contentHeight - f.height), f.contentY - dy));
        f.contentX = Math.max(0, Math.min(Math.max(0, f.contentWidth - f.width), f.contentX - dx));
        event.accepted = true;
    }
}
