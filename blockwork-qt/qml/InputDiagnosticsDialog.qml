import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch 1.0

// What input this session can see and send, as text to read or paste into a
// bug report. Reading it changes nothing.
AppDialog {
    id: diagnostics
    title: qsTr("Input diagnostics")
    standardButtons: Dialog.Close
    property var appBridge: null
    property string report: ""
    property var data: null
    // The report in plain words: one row per thing a macro can use, ok or not.
    readonly property var checks: data ? summarize(data) : []
    function summarize(r) {
        const linux = r.os === "linux";
        const rows = [];
        const add = (label, ok, detail) => rows.push({ label: label, ok: ok, detail: detail });
        add(qsTr("Recording keys and mouse"), r.capture.available, r.capture.available ? qsTr("Working") : qsTr("Not available"));
        add(qsTr("Playing keys and mouse"), r.output.available, r.output.available ? qsTr("Working") : qsTr("Not available"));
        const touchOk = r.touch.playback !== "unsupported" && (!linux || r.output.uinputWritable);
        add(qsTr("Touch playback"), touchOk, r.touch.playback === "unsupported" ? qsTr("Not supported on this system")
            : touchOk ? qsTr("Ready") : qsTr("Needs write access to /dev/uinput"));
        const pads = r.controllers.found;
        add(qsTr("Controller input"), r.controllers.input && pads.ok,
            !r.controllers.input ? qsTr("Off in Settings") : !pads.ok ? pads.error
            : qsTr("%n controller(s) connected", "", pads.value.length));
        const padOut = r.controllers.virtualOutputSupported && r.controllers.virtualOutput && r.output.uinputWritable;
        add(qsTr("Virtual controller"), padOut, !r.controllers.virtualOutputSupported ? qsTr("Not supported on this system")
            : !r.controllers.virtualOutput ? qsTr("Off in Settings") : padOut ? qsTr("Ready") : qsTr("Needs write access to /dev/uinput"));
        const apps = r.uiAutomation.applications;
        add(qsTr("Native UI automation"), r.uiAutomation.enabled && apps.ok, !r.uiAutomation.supported ? qsTr("Not supported on this system")
            : !r.uiAutomation.enabled ? qsTr("Off in Settings") : !apps.ok ? apps.error : qsTr("%n app(s) visible", "", apps.value));
        add(qsTr("Start at system startup"), r.startup.startAtStartup, r.startup.startAtStartup ? qsTr("On") : qsTr("Off in Settings"));
        return rows;
    }
    property string errorText: ""
    property string callKey: ""
    property int callSeq: 0

    function refresh() {
        report = ""; data = null; errorText = "";
        if (!appBridge) { errorText = qsTr("Diagnostics aren't available here."); return; }
        callKey = "diagnostics" + (++callSeq);
        appBridge.callCommand(callKey, "input_diagnostics", "{}");
    }
    function copy() { reportText.selectAll(); reportText.copy(); reportText.deselect(); }
    onAboutToShow: refresh()

    Connections {
        target: diagnostics.appBridge
        ignoreUnknownSignals: true
        function onReplySerialChanged() {
            if (diagnostics.appBridge.replyKey !== diagnostics.callKey) return;
            if (diagnostics.appBridge.replyError.length) diagnostics.errorText = diagnostics.appBridge.replyError;
            else {
                diagnostics.data = JSON.parse(diagnostics.appBridge.replyJson);
                diagnostics.report = JSON.stringify(diagnostics.data, null, 2);
            }
        }
    }

    Column {
        spacing: 10; width: 520
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Which input devices, permissions and services Blockwork can use right now. It doesn't include your macros or anything you typed.") }
        Text { visible: diagnostics.errorText.length > 0; text: diagnostics.errorText; color: Theme.danger; wrapMode: Text.WordWrap; width: parent.width }
        Column {
            objectName: "diagnosticsChecks"
            width: parent.width; spacing: 4
            Repeater {
                model: diagnostics.checks
                Row {
                    required property var modelData
                    width: parent.width; spacing: 8
                    LucideIcon { name: modelData.ok ? "circle-check" : "circle-alert"; width: 16; height: 16
                        color: modelData.ok ? Theme.successBorder : Theme.textDim; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: modelData.label; color: Theme.text; font.pixelSize: 13; width: 190; elide: Text.ElideRight }
                    Text { text: modelData.detail; color: Theme.textDim; font.pixelSize: 13; width: parent.width - 214; elide: Text.ElideRight }
                }
            }
        }
        Text { text: qsTr("Details"); color: Theme.textDim; font.pixelSize: 12 }
        Rectangle {
            width: parent.width; height: 180; radius: Theme.radius; color: Theme.field; border.color: Theme.borderSoft
            ScrollView {
                anchors.fill: parent; anchors.margins: 6
                TextArea {
                    id: reportText
                    objectName: "diagnosticsReport"
                    readOnly: true; selectByMouse: true; wrapMode: TextEdit.NoWrap
                    text: diagnostics.report.length ? diagnostics.report : qsTr("Checking…")
                    color: Theme.text; font.family: "monospace"; font.pixelSize: 11
                    background: null
                }
            }
        }
        Row { spacing: 8
            StitchButton { text: qsTr("Copy"); iconName: "copy"; enabled: diagnostics.report.length > 0; onClicked: diagnostics.copy() }
            StitchButton { text: qsTr("Check again"); iconName: "refresh-cw"; onClicked: diagnostics.refresh() }
        }
    }
}
