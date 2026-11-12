import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

AppDialog {
    id:root
    objectName:"nativeUiPickerDialog"
    required property Item page
    title:qsTr("Choose a native UI element")
    width:Math.min(760,parent ? parent.width-32 : 760)
    height:Math.min(620,parent ? parent.height-32 : 620)
    standardButtons:Dialog.Cancel
    property var request:null
    property string macroId:""
    property var applications:[]
    property var nodes:[]
    property string errorText:""
    property bool busy:false
    property int serial:0
    function openFor(req) {
        request=req;macroId=page.macro ? page.macro.id : "";applications=[];nodes=[];errorText="";
        application.currentIndex=-1;filter.text="";open();refreshApplications();
    }
    function refreshApplications() {
        const ticket=++serial;busy=true;
        page.daemonCall("native_ui_applications",{},(result,error)=>{
            if(ticket!==serial || !root.visible)return;
            busy=false;applications=result || [];application.currentIndex=-1;errorText=error || "";
        });
    }
    function inspect() {
        if(application.currentIndex<0)return;
        const ticket=++serial;busy=true;nodes=[];errorText="";
        const selector={scope:"App",app:applications[application.currentIndex],name:filter.text.trim()};
        page.daemonCall("native_ui_inspect",{selector:JSON.stringify(selector)},(result,error)=>{
            if(ticket!==serial || !root.visible)return;
            busy=false;nodes=result || [];errorText=error || "";
        });
    }
    function choose(selector) {
        if(!page.macro || page.macro.id!==macroId){errorText=qsTr("The macro changed. Reopen the element picker.");return;}
        if(page.recordingBusy){errorText=qsTr("Stop recording before changing this input.");return;}
        page.invoke("put_value",{location:request.location,value:{kind:"Text",value:JSON.stringify(selector)}});
        root.accept();
    }
    onClosed:serial++
    contentItem:ColumnLayout {
        spacing:10
        Label {Layout.fillWidth:true;text:qsTr("Choose an app, then a control. Only names, roles and stable IDs are inspected here; password controls are excluded. Actions resolve the selector again and refuse ambiguous matches.");color:Theme.textDim;wrapMode:Text.WordWrap}
        RowLayout {
            Layout.fillWidth:true
            StitchComboBox {id:application;objectName:"nativeUiApplication";Layout.fillWidth:true;model:root.applications;displayText:currentIndex<0 ? qsTr("Choose an app") : currentText;enabled:!root.busy;onActivated:root.inspect()}
            StitchButton {text:qsTr("Refresh apps");enabled:!root.busy;onClicked:root.refreshApplications()}
        }
        RowLayout {
            Layout.fillWidth:true
            StitchTextField {id:filter;objectName:"nativeUiNameFilter";Layout.fillWidth:true;placeholderText:qsTr("Filter element names");onAccepted:root.inspect()}
            StitchButton {text:qsTr("Search");enabled:!root.busy && application.currentIndex>=0;onClicked:root.inspect()}
        }
        StitchListView {
            Layout.fillWidth:true;Layout.fillHeight:true;clip:true;spacing:6
            model:root.nodes
            delegate:Rectangle {
                id:row
                required property var modelData
                width:ListView.view.width;implicitHeight:76;radius:Theme.radius;color:Theme.panelRaised;border.color:Theme.border
                RowLayout {
                    anchors.fill:parent;anchors.margins:10
                    ColumnLayout {
                        Layout.fillWidth:true;spacing:3
                        Label {Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText;text:row.modelData.name || qsTr("Unnamed element");color:Theme.text;font.bold:true}
                        Label {Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText;text:row.modelData.role + (row.modelData.id ? " · " + row.modelData.id : "") + (row.modelData.enabled ? "" : " · " + qsTr("disabled"));color:Theme.textDim}
                    }
                    StitchButton {text:qsTr("Choose");onClicked:root.choose(row.modelData.selector)}
                }
            }
        }
        Label {visible:root.nodes.length>=100;text:qsTr("Showing the first 100 matches. Narrow the name filter for more specific results.");Layout.fillWidth:true;wrapMode:Text.WordWrap;color:Theme.warning}
        Label {visible:!root.busy && application.currentIndex>=0 && !root.nodes.length && !root.errorText.length;text:qsTr("No matching elements.");color:Theme.textDim}
        Label {Layout.fillWidth:true;visible:text.length>0;text:root.errorText;color:Theme.danger;wrapMode:Text.WordWrap}
    }
}
