import QtQuick
import QtQuick.Window
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
    // The element picker needs the list instead: `hint` is {app, name, notice} or null.
    signal elementList(var request, var hint)

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
        if (pickRequest || !win) return;
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
        const win = Window.window;
        if (!win) return;
        // show*() rather than assigning `visibility`, which fights the window's `visible` binding.
        if (savedVisibility === Window.Maximized) win.showMaximized();
        else if (savedVisibility === Window.FullScreen) win.showFullScreen();
        else win.showNormal();
        win.raise(); win.requestActivate();
    }
    // Picks another app's element for a native UI block, in the macro it was asked from.
    property string elementMacro: ""
    readonly property alias elementOverlay: elementOverlay
    function startElementPick(request) {
        const win = Window.window;
        if (pickRequest || !win) return;
        pickRequest = request;
        elementMacro = page.macro ? page.macro.id : "";
        savedVisibility = win.visibility;
        win.hide();
        picker.afterUnmap(() => {
            elementOverlay.begin();
            elementOverlay.automationOff = !(page.appState && page.appState.ui_automation_enabled === true);
        });
    }
    ElementPickerOverlay {
        id: elementOverlay
        call: (command, args, done) => page.daemonCall(command, args, done)
        onCanceled: { picker.pickRequest = null; picker.restoreWindow(); }
        onTurnOnAutomation: page.invoke("set_ui_automation_enabled", { enabled: true })
        onSearchInstead: {
            const request = picker.pickRequest;
            picker.pickRequest = null; picker.restoreWindow();
            picker.elementList(request, null);
        }
        onPicked: pick => {
            const request = picker.pickRequest;
            picker.pickRequest = null; picker.restoreWindow();
            const node = pick.node;
            if (node && node.matches === 1) {
                const problem = page.writeNativeUi(request, node.text, picker.elementMacro);
                if (problem) picker.failed(problem);
                return;
            }
            const hit = pick.hit;
            picker.elementList(request, node
                ? { app: node.app, name: node.name, notice: qsTr("%1 elements in %2 look the same as the one you clicked. Choose one that can be told apart, or give the element a name in its app.").arg(node.matches).arg(node.app) }
                : { app: "", name: "", notice: qsTr("%1 doesn't describe that part of its window to Blockwork. If the app is listed, its elements may still be found here.").arg(hit.app || hit.window) });
        }
    }
    ScreenOverlay {
        id: screenOverlay
        onCanceled: { picker.pickRequest = null; picker.restoreWindow(); }
        onPicked: (x, y, width, height) => {
            const request = picker.pickRequest;
            picker.pickRequest = null;
            if (request && request.kind === "region") { picker.setBlockRegion(request, x, y, width, height); return; }
            if (request && request.kind === "value") { picker.setValueRegion(request, x, y, width, height); return; }
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
}
