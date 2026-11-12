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
    property string errorText: ""
    property string callKey: ""
    property int callSeq: 0

    function refresh() {
        report = ""; errorText = "";
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
            else diagnostics.report = JSON.stringify(JSON.parse(diagnostics.appBridge.replyJson), null, 2);
        }
    }

    Column {
        spacing: 10; width: 520
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Which input devices, permissions and services Blockwork can use right now. It doesn't include your macros or anything you typed.") }
        Text { visible: diagnostics.errorText.length > 0; text: diagnostics.errorText; color: Theme.danger; wrapMode: Text.WordWrap; width: parent.width }
        Rectangle {
            width: parent.width; height: 300; radius: Theme.radius; color: Theme.field; border.color: Theme.borderSoft
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
