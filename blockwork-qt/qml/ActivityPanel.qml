import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// Status strip under the canvas: the latest run event for the open macro,
// expanding into the run log.
Rectangle {
    id: root
    property var entries: []
    property string macroId: ""
    property bool sheetMode: false
    property bool expanded: false
    property bool allMacros: false
    property real panelHeight: 220
    readonly property int stripHeight: sheetMode ? 48 : 30
    signal blockRequested(string blockId)
    signal clearRequested()

    implicitHeight: expanded ? panelHeight : stripHeight
    // Slides open and shut; dragging the edge resizes without lag.
    Behavior on implicitHeight {
        enabled: Motion.enabled && !edgeDrag.pressed
        NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic }
    }
    // The log stays shown while the panel slides shut.
    readonly property bool open: expanded || height > stripHeight + 0.5
    color: Theme.panel; border.color: Theme.borderSoft
    clip: true

    readonly property var mine: entries.filter(e => e.macro_id === macroId)
    readonly property var shown: allMacros ? entries : mine
    readonly property var latest: mine.length ? mine[mine.length - 1] : null
    readonly property var lastRunEvent: {
        for (let i = mine.length - 1; i >= 0; --i)
            if (mine[i].event === "Started" || mine[i].event === "Ended") return mine[i];
        return null;
    }
    readonly property bool running: !!lastRunEvent && lastRunEvent.event === "Started"
    // Errors since this macro last started or fired.
    readonly property int recentErrors: {
        let n = 0;
        for (let i = mine.length - 1; i >= 0; --i) {
            if (mine[i].event === "Started" || mine[i].event === "Fired") break;
            if (mine[i].level === "Error") n++;
        }
        return n;
    }
    property real now: Date.now()
    Timer { interval: 1000; repeat: true; running: root.visible && !!root.latest; triggeredOnStart: true; onTriggered: root.now = Date.now() }

    // Seq of the entry a marker click pointed at, highlighted briefly.
    property real focusSeq: -1
    Timer { id: focusClear; interval: 2000; onTriggered: root.focusSeq = -1 }
    function showBlockError(blockId) {
        allMacros = false;
        expanded = true;
        for (let i = shown.length - 1; i >= 0; --i) {
            if (shown[i].block_id !== blockId) continue;
            focusSeq = shown[i].seq;
            focusClear.restart();
            Qt.callLater(() => list.positionViewAtIndex(i, ListView.Center));
            return;
        }
    }

    function ago(ms) {
        const s = Math.max(0, Math.round((now - ms) / 1000));
        if (s < 5) return qsTr("just now");
        if (s < 60) return qsTr("%1s ago").arg(s);
        if (s < 3600) return qsTr("%1m ago").arg(Math.floor(s / 60));
        if (s < 86400) return qsTr("%1h ago").arg(Math.floor(s / 3600));
        return Qt.locale().toString(new Date(ms), qsTr("MMM d", "date format for older log entries"));
    }
    function elapsed(ms) {
        const s = Math.max(0, Math.floor((now - ms) / 1000));
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
    }
    function statusText() {
        if (running) return qsTr("Running - %1").arg(elapsed(lastRunEvent.time_ms));
        if (!latest) return qsTr("No activity yet");
        const message = latest.message.split("\n")[0];
        const summary = latest.level === "Error" ? qsTr("Error: %1").arg(message) : message;
        return qsTr("%1 - %2", "log message and relative time").arg(summary).arg(ago(latest.time_ms));
    }

    // Drag the top edge to resize while open.
    MouseArea {
        id: edgeDrag
        visible: root.expanded && !root.sheetMode
        anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; height: 5; z: 10
        cursorShape: Qt.SizeVerCursor
        property real startY: 0
        property real startHeight: 0
        onPressed: mouse => { startY = mapToItem(null, mouse.x, mouse.y).y; startHeight = root.panelHeight; }
        onPositionChanged: mouse => {
            const y = mapToItem(null, mouse.x, mouse.y).y;
            root.panelHeight = Math.max(120, Math.min(root.parent ? root.parent.height * 0.7 : 600, startHeight + startY - y));
        }
    }

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Item {
            Layout.fillWidth: true; Layout.preferredHeight: root.stripHeight
            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: if (!root.sheetMode) root.expanded = !root.expanded }
            RowLayout {
                anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 8; spacing: 8
                Rectangle {
                    width: 8; height: 8; radius: 4
                    color: root.running ? Theme.accent : root.latest && root.latest.level === "Error" ? Theme.danger : Theme.textDim
                    SequentialAnimation on opacity {
                        running: root.running && Motion.enabled; loops: Animation.Infinite
                        onRunningChanged: if (!running) parent.opacity = 1
                        NumberAnimation { to: 0.3; duration: Motion.dur(600) }
                        NumberAnimation { to: 1; duration: Motion.dur(600) }
                    }
                }
                Text {
                    Layout.fillWidth: true; text: root.statusText(); textFormat: Text.PlainText; elide: Text.ElideRight
                    color: !root.running && root.latest && root.latest.level === "Error" ? Theme.danger : Theme.textDim; font.pixelSize: 12
                }
                Text {
                    visible: root.recentErrors > 0
                    text: qsTr("%n error(s)", "", root.recentErrors); color: Theme.danger; font.pixelSize: 12; font.weight: Font.DemiBold
                }
                Row {
                    visible: root.expanded; spacing: 4
                    BwButton {
                        text: qsTr("This macro"); implicitHeight: root.sheetMode ? 48 : 22; font.pixelSize: 11; primary: !root.allMacros
                        onClicked: root.allMacros = false
                    }
                    BwButton {
                        text: qsTr("All"); implicitHeight: root.sheetMode ? 48 : 22; font.pixelSize: 11; primary: root.allMacros
                        onClicked: root.allMacros = true
                    }
                    BwButton {
                        iconName: "trash"; text: ""; implicitWidth: root.sheetMode ? 48 : 22; implicitHeight: root.sheetMode ? 48 : 22; enabled: root.entries.length > 0
                        ToolTip.visible: hovered; ToolTip.text: qsTr("Clear the log")
                        onClicked: root.clearRequested()
                    }
                }
                LucideIcon {
                    visible: !root.sheetMode; name: "chevron-up"; rotation: root.expanded ? 180 : 0; color: Theme.textDim; width: 16; height: 16
                    Behavior on rotation { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }
                }
            }
        }
        Rectangle { visible: root.open; Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.borderSoft }
        Item {
            visible: root.open
            Layout.fillWidth: true; Layout.fillHeight: true
            Text {
                visible: root.shown.length === 0
                anchors.centerIn: parent; color: Theme.textDim; font.pixelSize: 12
                text: root.allMacros ? qsTr("Nothing yet. Runs, triggers, log blocks and errors show up here.") : qsTr("Nothing yet for this macro.")
            }
            ListView {
                id: list
                anchors.fill: parent; anchors.margins: 4
                clip: true; spacing: 1
                model: root.shown
                // Follow new entries unless the user scrolled up to read.
                property bool atEnd: true
                onMovementEnded: atEnd = atYEnd
                onCountChanged: if (atEnd) Qt.callLater(() => list.positionViewAtEnd())
                Component.onCompleted: positionViewAtEnd()
                ScrollBar.vertical: ScrollBar {}
                delegate: Rectangle {
                    id: row
                    required property var modelData
                    readonly property bool error: modelData.level === "Error"
                    readonly property bool linked: error && !!modelData.block_id && modelData.macro_id === root.macroId
                    width: list.width; height: logRow.implicitHeight + 6; radius: 4
                    color: modelData.seq === root.focusSeq ? "#4dff4848" : rowHover.hovered && linked ? "#40ff4848" : error ? "#26ff4848" : "transparent"
                    Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                    HoverHandler { id: rowHover; cursorShape: row.linked ? Qt.PointingHandCursor : Qt.ArrowCursor }
                    TapHandler { enabled: row.linked; onTapped: root.blockRequested(row.modelData.block_id) }
                    RowLayout {
                        id: logRow
                        x: 8; width: parent.width - 16; anchors.verticalCenter: parent.verticalCenter; spacing: 10
                        Text { text: Qt.formatTime(new Date(row.modelData.time_ms), "hh:mm:ss"); color: Theme.textDim; font.family: "monospace"; font.pixelSize: 11; Layout.alignment: Qt.AlignTop }
                        Text {
                            visible: root.allMacros
                            text: row.modelData.source; textFormat: Text.PlainText; color: Theme.textDim; font.pixelSize: 12; font.weight: Font.DemiBold
                            Layout.preferredWidth: 110; elide: Text.ElideRight; Layout.alignment: Qt.AlignTop
                        }
                        Text {
                            Layout.fillWidth: true; text: row.modelData.message; textFormat: Text.PlainText; wrapMode: Text.Wrap; font.pixelSize: 12
                            color: row.error ? Theme.danger : row.modelData.event ? Theme.textDim : Theme.text
                        }
                        LucideIcon { visible: row.linked; name: "crosshair"; color: Theme.danger; width: 14; height: 14; Layout.alignment: Qt.AlignTop }
                    }
                }
            }
        }
    }
}
