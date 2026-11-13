import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// Finds another app's element by searching its accessibility tree, for
// elements the screen picker can't point at (hidden, scrolled away, or in an
// app whose windows Blockwork can't locate). Writes the selector into the block.
AppDialog {
    id:root
    objectName:"nativeUiPickerDialog"
    required property Item page
    title:qsTr("Find a native UI element")
    width:Math.min(760,parent ? parent.width-32 : 760)
    height:Math.min(620,parent ? parent.height-32 : 620)
    standardButtons:Dialog.Cancel
    property var request:null
    property string macroId:""
    property var applications:[]
    property var nodes:[]
    property string errorText:""
    property string notice:""
    property bool busy:false
    property bool searched:false
    property int serial:0
    // An app and name to search once the app list arrives.
    property var wanted:null
    signal pickOnScreen(var request)
    function openFor(req, hint) {
        request=req;macroId=page.macro ? page.macro.id : "";applications=[];nodes=[];errorText="";searched=false;
        wanted=hint || null;notice=hint && hint.notice ? hint.notice : "";
        application.currentIndex=-1;filter.text=hint && hint.name ? hint.name : "";open();refreshApplications();
    }
    function refreshApplications() {
        const ticket=++serial;busy=true;
        page.daemonCall("native_ui_applications",{},(result,error)=>{
            if(ticket!==serial || !root.visible)return;
            busy=false;applications=result || [];application.currentIndex=-1;errorText=error || "";
            if(wanted && wanted.app){
                const index=applications.indexOf(wanted.app);
                wanted=null;
                if(index>=0){application.currentIndex=index;inspect();}
            }
        });
    }
    function inspect() {
        if(application.currentIndex<0)return;
        const ticket=++serial;busy=true;nodes=[];errorText="";
        const selector={scope:"App",app:applications[application.currentIndex],name:filter.text.trim()};
        page.daemonCall("native_ui_inspect",{selector:JSON.stringify(selector)},(result,error)=>{
            if(ticket!==serial || !root.visible)return;
            busy=false;searched=true;nodes=result || [];errorText=error || "";
        });
    }
    // Writes a selector into the block that asked; false with a reason if it can't.
    function write(req, text, forMacro) {
        if(!page.macro || page.macro.id!==forMacro)return qsTr("The macro changed. Pick the element again.");
        if(page.recordingBusy)return qsTr("Stop recording before changing this input.");
        page.invoke("put_value",{location:req.location,value:{kind:"Text",value:text}});
        return "";
    }
    function choose(node) {
        const problem=write(request,node.text,macroId);
        if(problem){errorText=problem;return;}
        root.accept();
    }
    onClosed:serial++
    contentItem:ColumnLayout {
        spacing:10
        Label {Layout.fillWidth:true;visible:root.notice.length>0;text:root.notice;color:Theme.warning;wrapMode:Text.WordWrap}
        RowLayout {
            Layout.fillWidth:true
            StitchComboBox {id:application;objectName:"nativeUiApplication";Layout.fillWidth:true;model:root.applications;displayText:currentIndex<0 ? qsTr("Choose an app") : currentText;enabled:!root.busy;onActivated:root.inspect()}
            StitchButton {iconName:"refresh-cw";text:qsTr("Refresh");enabled:!root.busy;onClicked:root.refreshApplications()}
            StitchButton {objectName:"nativeUiPickOnScreen";iconName:"scan";text:qsTr("Pick on screen");onClicked:{const req=root.request;root.reject();root.pickOnScreen(req);}}
        }
        StitchTextField {id:filter;objectName:"nativeUiNameFilter";Layout.fillWidth:true;placeholderText:qsTr("Filter by name, then press Enter");enabled:application.currentIndex>=0;onAccepted:root.inspect()}
        StitchListView {
            Layout.fillWidth:true;Layout.fillHeight:true;clip:true;spacing:6
            model:root.nodes
            delegate:Rectangle {
                id:row
                required property var modelData
                readonly property bool unique:modelData.matches===1
                width:ListView.view.width;implicitHeight:rowColumn.implicitHeight+20;radius:Theme.radius;color:Theme.panelRaised;border.color:Theme.border
                RowLayout {
                    anchors.fill:parent;anchors.margins:10;spacing:10
                    ColumnLayout {
                        id:rowColumn
                        Layout.fillWidth:true;spacing:3
                        Label {Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText;text:row.modelData.name || qsTr("Unnamed %1").arg(row.modelData.role || qsTr("element"));color:Theme.text;font.bold:true}
                        Label {Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText;text:row.modelData.role + (row.modelData.window ? " · " + row.modelData.window : "") + (row.modelData.enabled ? "" : " · " + qsTr("disabled"));color:Theme.textDim}
                        Label {Layout.fillWidth:true;elide:Text.ElideRight;textFormat:Text.PlainText;text:row.modelData.text;color:Theme.textDim;font.family:"monospace";font.pixelSize:11}
                        Label {
                            Layout.fillWidth:true;visible:!row.unique;wrapMode:Text.WordWrap;color:Theme.warning
                            text:qsTr("%1 elements look the same as this one, so a block can't tell them apart.").arg(row.modelData.matches)
                        }
                    }
                    StitchButton {text:qsTr("Use");primary:row.unique;onClicked:root.choose(row.modelData)}
                }
            }
        }
        Label {visible:root.nodes.length>=100;text:qsTr("Showing the first 100 matches. Narrow the name filter for more specific results.");Layout.fillWidth:true;wrapMode:Text.WordWrap;color:Theme.warning}
        Label {visible:root.busy;text:qsTr("Searching…");color:Theme.textDim}
        Label {visible:!root.busy && root.searched && !root.nodes.length && !root.errorText.length;text:qsTr("No matching elements.");color:Theme.textDim}
        Label {Layout.fillWidth:true;visible:!root.busy && application.currentIndex<0 && !root.errorText.length;text:qsTr("Choose the app the element is in. Password fields are never listed.");color:Theme.textDim;wrapMode:Text.WordWrap}
        Label {Layout.fillWidth:true;visible:text.length>0;text:root.errorText;color:Theme.danger;wrapMode:Text.WordWrap}
    }
}
