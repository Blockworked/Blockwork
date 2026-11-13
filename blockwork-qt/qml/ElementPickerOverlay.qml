import QtQml
import QtQuick
import QtQuick.Window
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// A see-through layer over every monitor for picking another app's button,
// field or slider: the element under the pointer is outlined and named, and a
// click picks it. The daemon answers in screen coordinates, the same space as
// ScreenOverlay.toScreen.
Item {
    id: root
    visible: false

    // The overlay's own windows, which the daemon skips when hit testing.
    readonly property string overlayTitle: "Blockwork element picker"
    // daemonCall(command, args, done(result, error)).
    property var call: null
    property bool active: false
    // The element under the pointer, as native_ui_element_at answers it.
    property var hover: null
    property string message: ""
    property bool picking: false
    // Settings > Integrations > Native UI automation is off; the bar offers to turn it on.
    property bool automationOff: false

    signal picked(var pick)
    signal canceled()
    signal searchInstead()
    signal turnOnAutomation()

    function begin() {
        hover = null; message = ""; picking = false; pending = null; asking = false;
        active = true;
    }
    function end() { active = false; hover = null; }
    function cancel() { end(); canceled(); }

    // Hover lookups: one at a time, always for the latest point.
    property var pending: null
    property bool asking: false
    function lookAt(point) {
        if (automationOff) return;
        pending = point;
        if (!asking) askNext();
    }
    function askNext() {
        if (!pending || !active || picking || !call) return;
        const point = pending;
        pending = null; asking = true;
        call("native_ui_element_at", { x: point.x, y: point.y, ignore: overlayTitle }, (hit, error) => {
            asking = false;
            if (!root.active) return;
            if (error) { root.message = error; root.hover = null; }
            else { root.message = ""; root.hover = hit || null; }
            throttle.restart();
        });
    }
    Timer { id: throttle; interval: 30; onTriggered: root.askNext() }

    function pickAt(point) {
        if (picking || !call || automationOff) return;
        picking = true; message = "";
        call("native_ui_pick", { x: point.x, y: point.y, ignore: overlayTitle }, (pick, error) => {
            picking = false;
            if (!root.active) return;
            if (error) { root.message = error; return; }
            if (!pick) { root.message = qsTr("There's no app window there. Point at a control in another app."); return; }
            root.end();
            root.picked(pick);
        });
    }

    // Overlay-local to screen coordinates, and back (see ScreenOverlay).
    function factor() {
        const plugin = Qt.platform.pluginName;
        return plugin === "xcb" || plugin === "windows";
    }
    function toScreen(screenInfo, lx, ly) {
        const f = factor() ? screenInfo.devicePixelRatio : 1;
        if (Qt.platform.pluginName === "windows")
            return { x: screenInfo.virtualX + lx * f, y: screenInfo.virtualY + ly * f };
        return { x: (screenInfo.virtualX + lx) * f, y: (screenInfo.virtualY + ly) * f };
    }
    function toLocal(screenInfo, x, y) {
        const f = factor() ? screenInfo.devicePixelRatio : 1;
        if (Qt.platform.pluginName === "windows")
            return { x: (x - screenInfo.virtualX) / f, y: (y - screenInfo.virtualY) / f, f: f };
        return { x: x / f - screenInfo.virtualX, y: y / f - screenInfo.virtualY, f: f };
    }
    function describe(hit) {
        if (!hit) return "";
        if (hit.window_only) return hit.window || hit.app;
        if (hit.role === "password text") return qsTr("Password field");
        return hit.name || qsTr("Unnamed %1").arg(hit.role || qsTr("element"));
    }
    function detail(hit) {
        if (!hit) return "";
        if (hit.window_only) return qsTr("This window doesn't describe its controls to Blockwork");
        if (hit.role === "password text") return qsTr("Password fields can't be picked or read");
        const where = hit.window && hit.window !== hit.app ? hit.app + " · " + hit.window : hit.app;
        return (hit.role ? hit.role + " · " : "") + where + (hit.enabled ? "" : " · " + qsTr("disabled"));
    }

    function overlayAt(index) { return overlays.objectAt(index); }
    Instantiator {
        id: overlays
        model: root.active ? Qt.application.screens : []
        delegate: Window {
            id: overlay
            required property var modelData
            required property int index
            screen: modelData
            flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
            color: "transparent"
            title: root.overlayTitle
            Component.onCompleted: { showFullScreen(); raise(); requestActivate(); }

            // The hovered element in this overlay's coordinates.
            readonly property var box: {
                const hit = root.hover;
                if (!hit) return null;
                const a = root.toLocal(modelData, hit.x, hit.y);
                return { x: a.x, y: a.y, width: hit.width / a.f, height: hit.height / a.f };
            }
            readonly property bool boxHere: !!box && box.x < width && box.y < height
                && box.x + box.width > 0 && box.y + box.height > 0

            readonly property alias tagItem: tag
            Item {
                anchors.fill: parent
                focus: true
                Keys.onEscapePressed: root.cancel()

                // A faint veil, so it's clear the screen is being picked from.
                Rectangle { anchors.fill: parent; color: "#14000000" }

                Rectangle {
                    visible: overlay.boxHere
                    x: overlay.boxHere ? overlay.box.x - 2 : 0
                    y: overlay.boxHere ? overlay.box.y - 2 : 0
                    width: overlay.boxHere ? overlay.box.width + 4 : 0
                    height: overlay.boxHere ? overlay.box.height + 4 : 0
                    radius: 3
                    color: Qt.alpha(Theme.accent, root.hover && root.hover.window_only ? 0.06 : 0.16)
                    border.color: root.hover && root.hover.window_only ? Theme.textDim : Theme.accent
                    border.width: 2
                    Behavior on x { NumberAnimation { duration: Motion.quick; easing.type: Easing.OutCubic } }
                    Behavior on y { NumberAnimation { duration: Motion.quick; easing.type: Easing.OutCubic } }
                    Behavior on width { NumberAnimation { duration: Motion.quick; easing.type: Easing.OutCubic } }
                    Behavior on height { NumberAnimation { duration: Motion.quick; easing.type: Easing.OutCubic } }
                }

                // What a click would pick, beside the outline.
                Rectangle {
                    id: tag
                    objectName: "elementPickerTag"
                    visible: overlay.boxHere
                    readonly property real below: overlay.boxHere ? overlay.box.y + overlay.box.height + 8 : 0
                    x: overlay.boxHere ? Math.max(8, Math.min(parent.width - width - 8, overlay.box.x)) : 0
                    y: below + height + 8 < parent.height ? below : Math.max(8, (overlay.boxHere ? overlay.box.y : 0) - height - 8)
                    width: Math.min(420, Math.max(tagName.implicitWidth, tagDetail.implicitWidth) + 20)
                    height: tagColumn.implicitHeight + 14
                    radius: 6
                    color: Qt.alpha(Theme.window, 0.96); border.color: Theme.border
                    Column {
                        id: tagColumn
                        anchors.left: parent.left; anchors.right: parent.right; anchors.margins: 10
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2
                        Text {
                            id: tagName
                            width: parent.width; elide: Text.ElideRight; textFormat: Text.PlainText
                            text: root.describe(root.hover)
                            color: Theme.text; font.pixelSize: 13; font.bold: true
                        }
                        Text {
                            id: tagDetail
                            width: parent.width; elide: Text.ElideRight; textFormat: Text.PlainText
                            text: root.detail(root.hover)
                            color: Theme.textDim; font.pixelSize: 12
                        }
                    }
                }

                MouseArea {
                    id: area
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: root.picking ? Qt.BusyCursor : Qt.CrossCursor
                    onPositionChanged: mouse => root.lookAt(root.toScreen(overlay.modelData, mouse.x, mouse.y))
                    onClicked: mouse => {
                        if (mouse.button === Qt.LeftButton) root.pickAt(root.toScreen(overlay.modelData, mouse.x, mouse.y));
                    }
                }

                // Instructions and actions, top middle of every monitor.
                Rectangle {
                    id: bar
                    anchors.top: parent.top; anchors.topMargin: 16; anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(parent.width - 32, barColumn.implicitWidth + 24)
                    height: barColumn.implicitHeight + 16; radius: 10
                    color: Qt.alpha(Theme.panel, 0.96); border.color: Theme.border
                    // Keep the pointer's hover from reaching the screen under the bar.
                    MouseArea { anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
                    Column {
                        id: barColumn
                        anchors.centerIn: parent; spacing: 6
                        Row {
                            spacing: 10
                            anchors.horizontalCenter: parent.horizontalCenter
                            LucideIcon { name: "scan"; width: 16; height: 16; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: root.automationOff ? qsTr("Picking elements needs native UI automation, which is off")
                                    : root.picking ? qsTr("Reading the app…") : qsTr("Click a button, field or slider in another app")
                                color: Theme.text; font.pixelSize: 13
                            }
                            StitchButton {
                                visible: root.automationOff; primary: true; text: qsTr("Turn on")
                                onClicked: { root.automationOff = false; root.message = ""; root.turnOnAutomation(); }
                            }
                            StitchButton { iconName: "list"; text: qsTr("Search a list"); onClicked: { root.end(); root.searchInstead(); } }
                            StitchButton { text: qsTr("Cancel"); onClicked: root.cancel() }
                        }
                        Text {
                            visible: root.message.length > 0
                            width: Math.min(560, implicitWidth)
                            anchors.horizontalCenter: parent.horizontalCenter
                            wrapMode: Text.WordWrap; textFormat: Text.PlainText
                            text: root.message
                            color: Theme.danger; font.pixelSize: 12
                        }
                    }
                }
            }
        }
    }
}
