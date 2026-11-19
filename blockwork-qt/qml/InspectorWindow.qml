import QtQuick
import QtQuick.Controls
import QtQuick.Window
import com.blockworked.Blockstitch 1.0

// Shows what is under the mouse while it moves around: the window, the mouse
// in screen and window coordinates, the pixel color and the native UI
// element, each with a button to copy it into a block. Pointing at this
// window itself keeps the last reading, so its copy buttons can be reached.
Window {
    id: root
    objectName: "inspectorWindow"
    title: inspectorTitle
    readonly property string inspectorTitle: "Blockwork inspector"
    width: 380; height: content.implicitHeight + 32
    minimumWidth: 320
    flags: Qt.Window | Qt.WindowStaysOnTopHint
    color: Theme.panel

    // daemonCall(command, args, done(result, error)).
    property var call: null
    property var reading: null
    property string errorText: ""
    property bool asking: false
    property int ticks: 0
    property bool pauseOnHover: true
    readonly property bool frozen: pauseOnHover && hover.hovered

    function refresh() {
        if (!call || asking || frozen) return;
        asking = true;
        // The color takes a screenshot, so it's read once a second.
        const withColor = ticks++ % 4 === 0 || !reading || !reading.color;
        call("inspect_mouse", { ignore: inspectorTitle, color: withColor }, (result, error) => {
            asking = false;
            if (error) { errorText = error; return; }
            errorText = "";
            if (result && !result.color && reading && reading.color && !withColor) result.color = reading.color;
            reading = result;
        });
    }
    Timer { interval: 250; repeat: true; running: root.visible; triggeredOnStart: true; onTriggered: root.refresh() }

    function rounded(n) { return Math.round(n || 0); }
    function note(part) {
        const notes = reading ? reading.notes || [] : [];
        const hit = notes.find(n => n[0] === part);
        return hit ? hit[1] : "";
    }
    readonly property var rows: {
        const r = reading;
        if (!r) return [];
        const w = r.window, e = r.element;
        const out = [
            [qsTr("Mouse on screen"), rounded(r.x) + ", " + rounded(r.y)],
            [qsTr("Window"), w ? w.title : note("window") || qsTr("None")],
            [qsTr("App"), w ? w.app : ""],
            [qsTr("Process ID"), w && w.pid ? String(w.pid) : ""],
            [qsTr("Mouse in window"), w ? rounded(w.mouse_x) + ", " + rounded(w.mouse_y) : ""],
            [qsTr("Window position"), w ? rounded(w.x) + ", " + rounded(w.y) : ""],
            [qsTr("Window size"), w ? rounded(w.width) + " × " + rounded(w.height) : ""],
            [qsTr("Color"), r.color || note("color")],
            [qsTr("Element"), e && !e.window_only ? (e.name || qsTr("Unnamed %1").arg(e.role)) : note("element")],
            [qsTr("Element role"), e && !e.window_only ? e.role : ""]
        ];
        return out.filter(row => row[1] !== "");
    }

    TextEdit { id: clipboardHelper; visible: false }
    function copy(text) { clipboardHelper.text = text; clipboardHelper.selectAll(); clipboardHelper.copy(); clipboardHelper.text = ""; }

    HoverHandler { id: hover }
    Column {
        id: content
        x: 16; y: 16; width: parent.width - 32
        spacing: 8
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: root.frozen ? qsTr("Paused while the mouse is over this window.")
                : qsTr("Move the mouse over any window. Point here to pause and copy a value.")
        }
        Text { visible: root.errorText.length > 0; width: parent.width; wrapMode: Text.WordWrap; color: Theme.danger; text: root.errorText }
        Repeater {
            model: root.rows
            delegate: Row {
                required property var modelData
                required property int index
                width: content.width
                spacing: 8
                Text { width: 120; text: modelData[0]; color: Theme.textDim; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                Rectangle {
                    visible: modelData[0] === qsTr("Color") && /^#/.test(modelData[1])
                    width: visible ? 14 : 0; height: 14; radius: 3; color: visible ? modelData[1] : "transparent"
                    border.color: Theme.border; anchors.verticalCenter: parent.verticalCenter
                }
                Text {
                    objectName: "inspectorValue_" + index
                    width: parent.width - 120 - 16 - 30 - (modelData[0] === qsTr("Color") ? 22 : 0)
                    text: modelData[1]; color: Theme.text; font.pixelSize: 12; elide: Text.ElideRight
                    anchors.verticalCenter: parent.verticalCenter
                }
                StitchButton {
                    width: 30; implicitHeight: 26; iconName: "copy"
                    Accessible.name: qsTr("Copy %1").arg(modelData[0])
                    onClicked: root.copy(modelData[1])
                }
            }
        }
    }
}
