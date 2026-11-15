import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

AppDialog {
    id: root
    objectName: "controllerPickerDialog"
    required property Item page
    title: qsTr("Choose a controller")
    width: Math.min(620, parent ? parent.width - 32 : 620)
    height: Math.min(520, parent ? parent.height - 32 : 520)
    standardButtons: Dialog.Cancel
    property var request: null
    property string macroId: ""
    property var controllers: []
    property string errorText: ""
    property bool busy: false
    property int serial: 0
    function openFor(req) { request=req; macroId=page.macro ? page.macro.id : ""; controllers=[]; errorText=""; open(); refresh(); }
    readonly property bool inputOn: page.appState.controller_input_enabled === true
    function turnOn() {
        page.invoke("set_controller_input_enabled", {enabled: true});
        errorText = "";
    }
    onInputOnChanged: if (visible) refresh()
    // Keep the list current while open, so a controller plugged in now shows up.
    Timer { interval: 1500; repeat: true; running: root.visible && root.inputOn; onTriggered: if (!root.busy) root.refresh() }
    function refresh() {
        if (!inputOn) { controllers = []; errorText = ""; return; }
        const ticket=++serial; busy=true;
        page.daemonCall("list_game_controllers", {}, (result,error) => {
            if (ticket !== serial || !root.visible) return;
            busy=false; controllers=result || []; errorText=error || "";
        });
    }
    function choose(profile) {
        if (!page.macro || page.macro.id !== macroId) {errorText=qsTr("The macro changed. Reopen the controller picker.");return;}
        if (page.recordingBusy) {errorText=qsTr("Stop recording before changing this input.");return;}
        page.invoke("put_value", {location:request.location, value:{kind:"Text",value:profile}});
        root.accept();
    }
    onClosed: serial++
    contentItem: ColumnLayout {
        spacing: 10
        Label { Layout.fillWidth: true; text: qsTr("The block keeps working after the controller is unplugged and plugged back in. Two identical controllers can't be told apart, so connect only one of them."); color:Theme.textDim;wrapMode:Text.WordWrap }
        // Controller input is off: offer to turn it on here rather than send the user to Settings.
        RowLayout {
            Layout.fillWidth:true;visible:!root.inputOn
            Label {Layout.fillWidth:true;text:qsTr("Controller input is off, so Blockwork can't see any controllers.");color:Theme.warning;wrapMode:Text.WordWrap}
            StitchButton {objectName:"turnOnControllerInput";text:qsTr("Turn on");primary:true;onClicked:root.turnOn()}
        }
        RowLayout {
            Layout.fillWidth:true
            StitchButton {text:qsTr("Refresh");iconName:"refresh-cw";enabled:!root.busy && root.inputOn;onClicked:root.refresh()}
            StitchButton {text:qsTr("Any controller");onClicked:root.choose("")}
            Label {Layout.fillWidth:true;text:qsTr("Uses whichever one is connected.");color:Theme.textDim;elide:Text.ElideRight}
        }
        StitchListView {
            Layout.fillWidth:true;Layout.fillHeight:true;clip:true;spacing:8
            model:root.controllers
            delegate:Rectangle {
                id:row
                required property var modelData
                readonly property bool ambiguous:root.controllers.filter(g => g.profile === modelData.profile).length > 1
                width:ListView.view.width;implicitHeight:76;radius:Theme.radius;color:Theme.panelRaised;border.color:Theme.border
                RowLayout {
                    anchors.fill:parent;anchors.margins:10
                    ColumnLayout {
                        Layout.fillWidth:true
                        Label {textFormat:Text.PlainText;text:row.modelData.name;color:Theme.text;font.bold:true;Layout.fillWidth:true;elide:Text.ElideRight}
                        Label {textFormat:Text.PlainText;text:row.ambiguous ? qsTr("Identical profiles connected; unplug one.") : row.modelData.profile;color:row.ambiguous ? Theme.warning : Theme.textDim;Layout.fillWidth:true;elide:Text.ElideRight}
                    }
                    StitchButton {text:qsTr("Choose");enabled:!row.ambiguous;onClicked:root.choose(row.modelData.profile)}
                }
            }
        }
        Label {visible:root.inputOn && !root.busy && !root.controllers.length && !root.errorText.length;text:qsTr("No controllers connected. Plug one in, then press Refresh.");color:Theme.textDim}
        Label {Layout.fillWidth:true;visible:text.length>0;text:root.errorText;color:Theme.danger;wrapMode:Text.WordWrap}
    }
}
