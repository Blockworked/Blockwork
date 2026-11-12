import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// Shown when a recording stops: what was captured, and what to keep before
// its blocks go into the recording target.
AppDialog {
    id: review
    title: qsTr("Review recording")
    required property Item page
    property var reviewData: null
    readonly property var summary: reviewData ? reviewData.summary : ({})
    // Stays open across state updates until inserted or discarded.
    closePolicy: Popup.NoAutoClose
    modal: true

    property bool keepKeys: true
    property bool keepButtons: true
    property bool keepScrolls: true
    property bool keepMovement: true
    property bool keepTouch: true
    property bool keepController: true
    property bool keepWaits: true
    property bool trimLeadingWait: false
    property bool capWaits: false
    property int maxWaitSeconds: 5
    property bool releaseHeld: true

    function seconds(ms) { return (Math.round(ms / 100) / 10).toLocaleString(Qt.locale(), "f", 1); }
    function options() {
        return { keepKeys: keepKeys, keepButtons: keepButtons, keepScrolls: keepScrolls,
                 keepMovement: keepMovement, keepTouch: keepTouch, keepController: keepController,
                 keepWaits: keepWaits, trimLeadingWait: trimLeadingWait,
                 maxWaitMs: capWaits ? maxWaitSeconds * 1000 : 0, releaseHeld: releaseHeld };
    }
    function resetChoices() {
        keepKeys = keepButtons = keepScrolls = keepMovement = keepTouch = keepController = keepWaits = true;
        trimLeadingWait = summary.leading_wait_ms >= 1000;
        capWaits = false; releaseHeld = true;
    }
    function insert() { page.invoke("apply_recording_review", { options: options() }); close(); }
    function discard() { page.invoke("discard_recording_review"); close(); }
    function show(next) {
        const fresh = !opened;
        reviewData = next;
        if (fresh) { resetChoices(); open(); }
    }
    function rowText(row) {
        const detail = row.detail;
        switch (row.source) {
        case "wait": return qsTr("Wait %1 ms").arg(detail);
        case "key": return row.direction === "press" ? qsTr("Press key %1").arg(detail)
                         : row.direction === "release" ? qsTr("Release key %1").arg(detail)
                         : qsTr("Type %1").arg(detail);
        case "button": return row.direction === "press" ? qsTr("Press mouse %1").arg(detail)
                            : row.direction === "release" ? qsTr("Release mouse %1").arg(detail)
                            : qsTr("Click mouse %1").arg(detail);
        case "scroll": return qsTr("Scroll %1").arg(detail);
        case "movement": return qsTr("Move mouse to %1").arg(detail);
        case "touch": {
            const parts = detail.split("|");
            return parts.length === 2 ? qsTr("Touch with %n finger(s) for %1 ms", "", Number(parts[0])).arg(parts[1]) : qsTr("Touch paths");
        }
        case "controller": return row.direction === "press" ? qsTr("Press controller %1").arg(detail)
                                : row.direction === "release" ? qsTr("Release controller %1").arg(detail)
                                : detail === "reset" ? qsTr("Reset controller") : qsTr("Controller %1").arg(detail);
        default: return detail;
        }
    }

    readonly property var sources: [
        { key: "keys", label: qsTr("Keys"), prop: "keepKeys" },
        { key: "buttons", label: qsTr("Mouse buttons"), prop: "keepButtons" },
        { key: "scrolls", label: qsTr("Scrolling"), prop: "keepScrolls" },
        { key: "movements", label: qsTr("Mouse movement"), prop: "keepMovement" },
        { key: "touch", label: qsTr("Touch"), prop: "keepTouch" },
        { key: "controller", label: qsTr("Controller"), prop: "keepController" }
    ]

    footer: Row {
        spacing: 8; padding: 12; layoutDirection: Qt.RightToLeft
        StitchButton { objectName: "insertRecording"; primary: true; text: qsTr("Insert blocks"); onClicked: review.insert() }
        StitchButton { objectName: "discardRecording"; text: qsTr("Discard"); onClicked: review.discard() }
    }

    Column {
        spacing: 12; width: 440
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
            text: qsTr("%n block(s) recorded into %1, taking %2 s to play.", "", review.summary.total || 0)
                .arg(review.reviewData ? review.reviewData.macro_name : "").arg(review.seconds(review.summary.duration_ms || 0))
        }
        Flow {
            width: parent.width; spacing: 10
            Repeater {
                model: review.sources.filter(s => (review.summary[s.key] || 0) > 0)
                StitchCheckBox {
                    required property var modelData
                    text: qsTr("%1 (%2)").arg(modelData.label).arg(review.summary[modelData.key])
                    checked: review[modelData.prop]
                    onToggled: review[modelData.prop] = checked
                }
            }
        }
        Rectangle { width: parent.width; height: 1; color: Theme.borderSoft }
        StitchCheckBox { objectName: "keepWaits"; text: qsTr("Keep the timing between inputs"); checked: review.keepWaits; onToggled: review.keepWaits = checked }
        StitchCheckBox {
            visible: review.keepWaits && (review.summary.leading_wait_ms || 0) > 0
            text: qsTr("Skip the %1 s before the first input").arg(review.seconds(review.summary.leading_wait_ms || 0))
            checked: review.trimLeadingWait; onToggled: review.trimLeadingWait = checked
        }
        Row {
            spacing: 8
            visible: review.keepWaits && (review.summary.longest_wait_ms || 0) > 1000
            StitchCheckBox { text: qsTr("Shorten waits longer than"); checked: review.capWaits; onToggled: review.capWaits = checked; anchors.verticalCenter: parent.verticalCenter }
            SpinBox { from: 1; to: 600; value: review.maxWaitSeconds; enabled: review.capWaits; editable: true; onValueModified: review.maxWaitSeconds = value; Accessible.name: qsTr("Seconds") }
            Text { text: qsTr("seconds"); color: Theme.textDim; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
        }
        StitchCheckBox {
            visible: (review.summary.unreleased || 0) > 0
            text: qsTr("Release %n key(s) or button(s) still held at the end", "", review.summary.unreleased || 0)
            checked: review.releaseHeld; onToggled: review.releaseHeld = checked
        }
        Rectangle {
            width: parent.width; height: 200; radius: Theme.radius
            color: Theme.field; border.color: Theme.borderSoft
            StitchListView {
                id: rows
                objectName: "reviewRows"
                anchors.fill: parent; anchors.margins: 6; clip: true
                model: review.reviewData ? review.reviewData.rows : []
                delegate: Text {
                    required property var modelData
                    width: rows.width; elide: Text.ElideRight
                    text: review.rowText(modelData)
                    color: modelData.source === "wait" ? Theme.textDim : Theme.text
                    font.pixelSize: 12; topPadding: 2; bottomPadding: 2
                }
            }
        }
        Text {
            visible: review.reviewData && review.reviewData.rows.length < (review.summary.total || 0)
            text: qsTr("Showing the first %1 blocks.").arg(review.reviewData ? review.reviewData.rows.length : 0)
            color: Theme.textDim; font.pixelSize: 11
        }
    }
}
