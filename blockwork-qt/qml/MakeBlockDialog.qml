import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

Dialog {
    id:root
    modal:true; anchors.centerIn:parent; title:"Make a Block"; width:700
    standardButtons:Dialog.NoButton
    signal createRequested(var pieces,string shape,string color)
    property int selectedPiece:0
    property string blockShape:"Normal"
    property string blockColor:"#3b3c40"

    ListModel { id:pieces }
    function resetForm() { pieces.clear();pieces.append({pieceKind:"Label",pieceName:"block name",valueType:"Number"});selectedPiece=0;blockShape="Normal";blockColor="#3b3c40"; }
    function addPiece(kind,name,valueType) { pieces.append({pieceKind:kind,pieceName:name,valueType:valueType||"Number"});selectedPiece=pieces.count-1; }
    function resultPieces() { let out=[];for(let i=0;i<pieces.count;i++){const p=pieces.get(i);out.push(p.pieceKind==="Label"?{kind:"Label",id:uuid(),text:p.pieceName}:{kind:"Input",id:uuid(),name:p.pieceName,value_type:p.valueType});}return out; }
    function uuid(){return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g,c=>{const r=Math.random()*16|0;return(c==="x"?r:(r&3|8)).toString(16);});}
    onOpened:resetForm()

    contentItem:ColumnLayout {
        spacing:12
        Rectangle {
            Layout.fillWidth:true;Layout.preferredHeight:230;radius:6;color:Theme.canvas;border.color:Theme.border;clip:true
            Canvas { anchors.fill:parent;onPaint:{const c=getContext("2d");c.fillStyle="#3c3d42";for(let x=10;x<width;x+=22)for(let y=10;y<height;y+=22){c.beginPath();c.arc(x,y,1,0,Math.PI*2);c.fill();}} }
            Item {
                anchors.centerIn:parent;width:Math.max(220,previewRow.implicitWidth+62);height:62
                BlockSurface { anchors.fill:parent;shape:root.blockShape==="Ending"?"cap":"stack";fill:root.blockColor }
                Row {
                    id:previewRow;x:42;y:16;spacing:4
                    LucideIcon{name:"blocks";color:Theme.textDim;width:16;height:16;anchors.verticalCenter:parent.verticalCenter}
                    Repeater { model:pieces
                        delegate:Rectangle {
                            required property int index;required property string pieceKind;required property string pieceName;required property string valueType
                            height:29;width:pieceText.implicitWidth+(pieceKind==="Label"?12:20);radius:valueType==="Bool"?12:5
                            color:valueType==="Bool"?"transparent":root.selectedPiece===index?"#274c68":pieceKind==="Label"?"transparent":Theme.field
                            border.color:valueType==="Bool"?"transparent":root.selectedPiece===index?Theme.accent:pieceKind==="Label"?"transparent":Theme.border
                            Canvas { anchors.fill:parent;visible:valueType==="Bool";onPaint:{const c=getContext("2d");c.beginPath();const n=Math.min(height*.32,width/2);c.moveTo(n,.5);c.lineTo(width-n,.5);c.lineTo(width-.5,height/2);c.lineTo(width-n,height-.5);c.lineTo(n,height-.5);c.lineTo(.5,height/2);c.closePath();c.fillStyle=root.selectedPiece===index?"#274c68":Theme.field;c.fill();c.strokeStyle=root.selectedPiece===index?Theme.accent:Theme.border;c.stroke();} }
                            Text{id:pieceText;anchors.centerIn:parent;text:pieceKind==="Label"?pieceName:"("+pieceName+")";color:pieceKind==="Label"?Theme.text:Theme.accent;font.pixelSize:12;font.weight:pieceKind==="Label"?Font.Normal:Font.DemiBold}
                            TapHandler{onTapped:root.selectedPiece=index;onDoubleTapped:{root.selectedPiece=index;pieceEditor.text=pieceName;pieceEditor.forceActiveFocus();pieceEditor.selectAll();}}
                        }
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth:true
            Text { text:"Selected piece";color:Theme.textDim;font.pixelSize:12 }
            BwTextField { id:pieceEditor;Layout.fillWidth:true;text:pieces.count&&root.selectedPiece<pieces.count?pieces.get(root.selectedPiece).pieceName:"";onEditingFinished:if(pieces.count)pieces.setProperty(root.selectedPiece,"pieceName",text) }
            BwButton { iconName:"arrow-left";text:"";enabled:root.selectedPiece>0;onClicked:{pieces.move(root.selectedPiece,root.selectedPiece-1,1);root.selectedPiece--;} }
            BwButton { iconName:"chevron-down";text:"";enabled:root.selectedPiece<pieces.count-1;onClicked:{pieces.move(root.selectedPiece,root.selectedPiece+1,1);root.selectedPiece++;} }
            BwButton { iconName:"trash";text:"";danger:true;enabled:pieces.count>1;onClicked:{pieces.remove(root.selectedPiece);root.selectedPiece=Math.max(0,root.selectedPiece-1);} }
        }
        RowLayout {
            Layout.fillWidth:true;spacing:8
            BwButton { Layout.fillWidth:true;iconName:"plus";text:"Add a label";onClicked:root.addPiece("Label","label","Number") }
            BwButton { Layout.fillWidth:true;iconName:"plus";text:"Add an input";onClicked:root.addPiece("Input","value","Number") }
            BwButton { Layout.fillWidth:true;iconName:"plus";text:"Add a Boolean";onClicked:root.addPiece("Input","condition","Bool") }
        }
        Text { text:"Block shape";color:Theme.textDim;font.pixelSize:11;font.weight:Font.Bold }
        RowLayout {
            Layout.fillWidth:true;spacing:8
            Repeater { model:["Normal","Ending","ReturnsValue","ReturnsBool"]
                delegate:BwButton { required property string modelData;Layout.fillWidth:true;text:modelData==="ReturnsValue"?"Returns a value":modelData==="ReturnsBool"?"Returns a Boolean":modelData;primary:root.blockShape===modelData;onClicked:root.blockShape=modelData }
            }
        }
        Text { text:"Color";color:Theme.textDim;font.pixelSize:11;font.weight:Font.Bold }
        Row { spacing:8
            Repeater { model:["#3b3c40","#176fa6","#7755a8","#2b8060","#a56b2f","#a84855"]
                delegate:Rectangle { required property string modelData;width:30;height:30;radius:15;color:modelData;border.width:root.blockColor===modelData?3:1;border.color:root.blockColor===modelData?Theme.accent:Theme.border;TapHandler{onTapped:root.blockColor=modelData} }
            }
        }
        Rectangle { Layout.fillWidth:true;height:1;color:Theme.borderSoft }
        RowLayout { Layout.fillWidth:true
            Item { Layout.fillWidth:true }
            BwButton { text:"Cancel";onClicked:root.reject() }
            BwButton { text:"Create block";iconName:"plus";primary:true;enabled:pieces.count>0;onClicked:{root.createRequested(root.resultPieces(),root.blockShape,root.blockColor);root.accept();} }
        }
    }
}
