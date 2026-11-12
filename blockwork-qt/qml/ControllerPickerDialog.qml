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
    function refresh() {
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
        Label { Layout.fillWidth: true; text: qsTr("Enable Controller input in Settings first. Profiles stay stable across reconnects; identical connected profiles cannot be distinguished safely."); color:Theme.textDim;wrapMode:Text.WordWrap }
        RowLayout {
            Layout.fillWidth:true
            StitchButton {text:qsTr("Refresh");enabled:!root.busy;onClicked:root.refresh()}
            StitchButton {text:qsTr("Any single controller");onClicked:root.choose("")}
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
        Label {visible:!root.busy && !root.controllers.length && !root.errorText.length;text:qsTr("No controllers connected.");color:Theme.textDim}
        Label {Layout.fillWidth:true;visible:text.length>0;text:root.errorText;color:Theme.danger;wrapMode:Text.WordWrap}
    }
}
