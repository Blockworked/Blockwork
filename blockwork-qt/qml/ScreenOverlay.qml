import QtQml
import QtQuick
import QtQuick.Window
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// A see-through layer over every monitor: click a point or drag out a
// region on the live screen. Reports screen coordinates (the space the
// daemon's capture and mouse blocks use).
Item {
    id: root
    visible: false

    property bool active: false
    // Only a dragged region counts (capturing an image).
    property bool requireRegion: false
    property string hint: ""
    // The selection, in the local coordinates of `selScreen`'s overlay.
    property int selScreen: -1
    property bool selecting: false
    property bool hasSelection: false
    property real selX: 0
    property real selY: 0
    property real selW: 0
    property real selH: 0

    signal picked(real x, real y, real width, real height)
    signal canceled()

    function begin(regionOnly, text) {
        requireRegion = regionOnly; hint = text;
        selScreen = -1; selecting = false; hasSelection = false;
        active = true;
    }
    function end() { active = false; }

    // Screen coordinates from a point on one overlay. X11 captures native
    // pixels; elsewhere screen coordinates are Qt's own logical ones.
    function toScreen(screenInfo, lx, ly) {
        const f = Qt.platform.pluginName === "xcb" ? screenInfo.devicePixelRatio : 1;
        return { x: (screenInfo.virtualX + lx) * f, y: (screenInfo.virtualY + ly) * f, f: f };
    }
    function finish(screenInfo, lx, ly, w, h) {
        const p = toScreen(screenInfo, lx, ly);
        end();
        picked(p.x, p.y, w * p.f, h * p.f);
    }
    function accept() {
        if (!hasSelection || selScreen < 0) return;
        const win = overlays.objectAt(selScreen);
        if (win) finish(win.modelData, selX, selY, selW, selH);
    }

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
            title: "Pick from the screen"
            Component.onCompleted: { showFullScreen(); raise(); requestActivate(); }

            readonly property bool mine: root.selScreen === index && (root.selecting || root.hasSelection)
            readonly property real hx: mine ? root.selX : 0
            readonly property real hy: mine ? root.selY : 0
            readonly property real hw: mine ? root.selW : 0
            readonly property real hh: mine ? root.selH : 0

            Item {
                anchors.fill: parent
                focus: true
                Keys.onEscapePressed: { root.end(); root.canceled(); }
                Keys.onReturnPressed: root.accept()
                Keys.onEnterPressed: root.accept()

                // Dim everything but the selection.
                readonly property color shade: "#66000000"
                Rectangle { x: 0; y: 0; width: parent.width; height: overlay.hy; color: parent.shade }
                Rectangle { x: 0; y: overlay.hy + overlay.hh; width: parent.width; height: parent.height - y; color: parent.shade }
                Rectangle { x: 0; y: overlay.hy; width: overlay.hx; height: overlay.hh; color: parent.shade }
                Rectangle { x: overlay.hx + overlay.hw; y: overlay.hy; width: parent.width - x; height: overlay.hh; color: parent.shade }
                Rectangle {
                    visible: overlay.mine
                    x: overlay.hx - 1; y: overlay.hy - 1; width: overlay.hw + 2; height: overlay.hh + 2
                    color: "transparent"; border.color: Theme.accent; border.width: 2
                }

                MouseArea {
                    id: area
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.CrossCursor
                    property real anchorX: 0
                    property real anchorY: 0
                    onPressed: mouse => {
                        anchorX = mouse.x; anchorY = mouse.y;
                        root.selScreen = overlay.index; root.hasSelection = false; root.selecting = true;
                        root.selX = mouse.x; root.selY = mouse.y; root.selW = 0; root.selH = 0;
                    }
                    onPositionChanged: mouse => {
                        if (!root.selecting || root.selScreen !== overlay.index) return;
                        const x = Math.max(0, Math.min(width, mouse.x)), y = Math.max(0, Math.min(height, mouse.y));
                        root.selX = Math.min(anchorX, x); root.selY = Math.min(anchorY, y);
                        root.selW = Math.abs(x - anchorX); root.selH = Math.abs(y - anchorY);
                    }
                    onReleased: {
                        if (!root.selecting) return;
                        root.selecting = false;
                        if (root.selW >= 3 && root.selH >= 3) root.hasSelection = true;
                        else if (!root.requireRegion) root.finish(overlay.modelData, anchorX, anchorY, 0, 0);
                    }
                }

                // Where the mouse is, in screen coordinates.
                Rectangle {
                    visible: area.containsMouse
                    readonly property var at: root.toScreen(overlay.modelData, area.mouseX, area.mouseY)
                    x: Math.min(parent.width - width - 8, area.mouseX + 18)
                    y: Math.min(parent.height - height - 8, area.mouseY + 18)
                    width: coords.implicitWidth + 14; height: coords.implicitHeight + 8; radius: 5
                    color: "#e6202124"; border.color: Theme.border
                    Text {
                        id: coords
                        anchors.centerIn: parent
                        text: root.selecting && overlay.mine
                            ? Math.round(root.selW * parent.at.f) + " × " + Math.round(root.selH * parent.at.f)
                            : Math.round(parent.at.x) + ", " + Math.round(parent.at.y)
                        color: Theme.text; font.pixelSize: 12
                    }
                }

                // Instructions and actions, top middle of every monitor.
                Rectangle {
                    anchors.top: parent.top; anchors.topMargin: 16; anchors.horizontalCenter: parent.horizontalCenter
                    width: bar.implicitWidth + 24; height: 46; radius: 10
                    color: "#f0242527"; border.color: Theme.border
                    Row {
                        id: bar
                        anchors.centerIn: parent; spacing: 10
                        LucideIcon { name: "crosshair"; width: 16; height: 16; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
                        Text { text: root.hint; color: Theme.text; font.pixelSize: 13; anchors.verticalCenter: parent.verticalCenter }
                        BwButton { text: "Cancel"; onClicked: { root.end(); root.canceled(); } }
                        BwButton { iconName: "check"; text: "Use selection"; primary: true; enabled: root.hasSelection; onClicked: root.accept() }
                    }
                }
            }
        }
    }
}
