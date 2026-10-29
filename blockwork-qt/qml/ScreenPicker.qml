import QtQuick
import QtQuick.Window
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// Hides the editor and lets the user pick from the live screen: an image for
// an image block, a region for a find or detect block, or a point or region
// for a screen reporter. Writes the result into the block through `page`.
Item {
    id: picker
    // The EditorPage whose blocks this edits: invoke(), daemonCall() and the block helpers.
    required property Item page
    // A pick went wrong; the page shows `message`.
    signal failed(var message)

    // { kind: "image" | "region", strandId, path, instruction }, { kind: "value", location, value, point }
    property var pickRequest: null
    property int savedVisibility: Window.Windowed
    // Gives the compositor a moment to take a window off screen.
    function afterUnmap(action) { unmapDelay.action = action; unmapDelay.restart(); }
    Timer { id: unmapDelay; interval: 300; property var action: null; onTriggered: if (action) action() }
    // Blocks whose region is in screen coordinates rather than a window's.
    readonly property var screenRegionTypes: ["FindImage", "ClickImage", "WaitForImage"]
    function startScreenPick(request) {
        const win = Window.window;
        if (pickRequest || (!page.androidHost && !win)) return;
        if (page.androidHost) {
            pickRequest = request;
            if (request.kind === "element") screenOverlay.beginElement();
            else screenOverlay.begin(!!request.region || request.kind === "image" || request.kind === "region" || (request.kind === "value" && !request.point), "");
            return;
        }
        pickRequest = request;
        // Out of the way, so whatever is behind the editor can be picked.
        savedVisibility = win.visibility;
        win.hide();
        const onScreen = request.kind === "region" && picker.screenRegionTypes.indexOf((request.instruction || {}).type) >= 0;
        const hint = onScreen ? qsTr("Drag out the part of the screen to search")
            : request.kind === "value" ? (request.point ? qsTr("Click the point to read") : qsTr("Drag out the part of the screen to read"))
            : { image: qsTr("Drag around the image to look for"), region: qsTr("Drag out the part of the window to search") }[request.kind];
        const regionOnly = request.kind === "value" ? !request.point : true;
        picker.afterUnmap(() => screenOverlay.begin(regionOnly, hint));
    }
    function restoreWindow() {
        if (page.androidHost) return;
        const win = Window.window;
        if (!win) return;
        // show*() rather than assigning `visibility`, which fights the window's `visible` binding.
        if (savedVisibility === Window.Maximized) win.showMaximized();
        else if (savedVisibility === Window.FullScreen) win.showFullScreen();
        else win.showNormal();
        win.raise(); win.requestActivate();
    }
    ScreenOverlay {
        id: screenOverlay
        objectName: "screenOverlay"
        onFailed: error => { picker.pickRequest = null; picker.restoreWindow(); picker.failed(error); }
        onElementPicked: selector => {
            const request = picker.pickRequest;
            picker.pickRequest = null;
            if (!request) return;
            const updated = JSON.parse(JSON.stringify(request.instruction));
            updated.selector = selector;
            page.invoke("edit_instruction", {strandId: request.strandId, path: request.path, instruction: updated});
        }
        onCanceled: { picker.pickRequest = null; picker.restoreWindow(); }
        onPicked: (x, y, width, height) => {
            const request = picker.pickRequest;
            picker.pickRequest = null;
            if (request && request.kind === "region") { picker.setBlockRegion(request, x, y, width, height); return; }
            if (request && request.kind === "value") { picker.setValueRegion(request, x, y, width, height); return; }
            if (page.androidHost && screenOverlay.captureResult) {
                const result = screenOverlay.captureResult;
                if (request && request.kind === "touch") {
                    const updated = JSON.parse(JSON.stringify(request.instruction));
                    updated[request.end ? "end_x" : "x"] = page.numberValue(x);
                    updated[request.end ? "end_y" : "y"] = page.numberValue(y);
                    page.invoke("edit_instruction", {strandId: request.strandId, path: request.path, instruction: updated});
                } else if (request && request.kind === "image") page.setBlockImage(request, result.image);
                else toolResult.show(result, !!request && !!request.region);
                return;
            }
            // Capture once the overlay itself is gone.
            picker.afterUnmap(() => page.daemonCall("screen_grab", { x: x, y: y, width: width, height: height }, (result, error) => {
                picker.restoreWindow();
                if (!result) { picker.failed(error); return; }
                if (request) page.setBlockImage(request, result.image);
            }));
        }
    }
    // A screen reporter's x, y (and width, height) from a pick, in screen coordinates.
    function setValueRegion(request, x, y, width, height) {
        picker.restoreWindow();
        const n = v => page.numberValue(Math.round(v));
        const updated = JSON.parse(JSON.stringify(request.value));
        updated.args = request.point ? [n(x), n(y)] : [n(x), n(y), n(width), n(height)];
        page.invoke("put_value", { location: request.location, value: updated });
    }
    // A detect block's region is stored relative to its window, so look the window up.
    // An image block's is plain screen coordinates.
    function setBlockRegion(request, x, y, width, height) {
        picker.restoreWindow();
        const ins = request.instruction || {};
        if (picker.screenRegionTypes.indexOf(ins.type) >= 0) {
            const updated = JSON.parse(JSON.stringify(ins));
            updated.region = [x, y, width, height].map(Math.round).join(",");
            page.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
            return;
        }
        const pattern = ins.pattern && ins.pattern.kind === "Text" ? ins.pattern.value : null;
        if (pattern === null) { picker.failed(qsTr("Type the window's name into the block to pick a region of it.")); return; }
        page.daemonCall("window_bounds", { pattern: pattern, by: ins.by || "Title" }, (bounds, error) => {
            if (!bounds) { picker.failed(error); return; }
            const r = v => Math.round(v);
            const updated = JSON.parse(JSON.stringify(ins));
            updated.region = [r(x - bounds.x), r(y - bounds.y), r(width), r(height)].join(",");
            page.invoke("edit_instruction", { strandId: request.strandId, path: request.path, instruction: updated });
        });
    }
    AppDialog {
        id: toolResult
        title: qsTr("Screen selection")
        property var pick: null
        property bool region: false
        function show(result, isRegion) { pick = result; region = isRegion; open(); }
        standardButtons: Dialog.Close
        width: Math.min(420, page.width - 32)
        contentItem: Column {
            spacing: 8
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.text
                text: !toolResult.pick ? "" : qsTr("x %1   y %2").arg(toolResult.pick.x).arg(toolResult.pick.y)
                    + (toolResult.region ? "\n" + qsTr("width %1   height %2").arg(toolResult.pick.width).arg(toolResult.pick.height) : "")
                    + "\n" + qsTr("Reference display: %1 × %2 physical pixels").arg(toolResult.pick.display_width).arg(toolResult.pick.display_height)
            }
            Repeater {
                model: toolResult.region ? [["AverageColor", qsTr("Average color here")], ["ClickImage", qsTr("Tap this image")], ["WaitForImage", qsTr("Wait for this image")], ["FindImage", qsTr("Find this image")]]
                    : [["PixelColor", qsTr("Color at this point")], ["Touch", qsTr("Tap here")]]
                delegate: StitchButton {
                    required property var modelData
                    text: modelData[1]
                    implicitHeight: 48
                    onClicked: { picker.addSelection(modelData[0], toolResult.pick); toolResult.close(); }
                }
            }
        }
    }
    function addSelection(type, pick) {
        const n = v => page.numberValue(Math.round(v));
        const at = page.selectionPosition();
        if (type === "PixelColor" || type === "AverageColor") {
            const args = [n(pick.x), n(pick.y)];
            if (type === "AverageColor") args.push(n(pick.width), n(pick.height));
            page.invoke("create_floating_value", {x: at.x, y: at.y, value: {kind: "Op", op: type, args: args, saved: page.textValue("")}, originBlockId: null});
        } else {
            const block = page.defaultInstruction(type);
            if (type === "Touch") { block.x = n(pick.x); block.y = n(pick.y); }
            else block.image = pick.image;
            page.invoke("add_strand", {x: at.x, y: at.y, instruction: block});
        }
    }

}
