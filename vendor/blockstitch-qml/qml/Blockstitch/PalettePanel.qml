import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes

Rectangle {
    id: root
    property var instructionTypes: []
    property var listInstructionTypes: []
    property var variables: []
    property var lists: []
    // Dicts are opt-in: a host shows the section once its backend has dict commands.
    property bool dictsEnabled: false
    property var dicts: []
    property var dictInstructionTypes: []
    property var blockDefinitions: []
    // Host reporters, same [op, result, arity] shape as `operators`, with an
    // optional fourth entry giving the fresh block's args.
    property var extraOperators: []
    // Host type/op -> category id, checked before the built-in rules.
    property var extraCategories: ({})
    readonly property var allOperators: operators.concat(Array.from(extraOperators || []))
    property var keyCapture: null
    property string standaloneKey: ""
    property bool trashActive: false  // a canvas block or value is being dragged
    property bool trashArmed: false   // ...and is currently over the panel
    signal blockActivated(string type, var instruction)
    signal dragStarted(var spec, real sceneX, real sceneY, real offsetX, real offsetY)
    signal dragMoved(real sceneX, real sceneY)
    signal dragEnded(real sceneX, real sceneY)
    signal dragCanceled()
    signal resizeRequested(real width)
    signal customBlockActivated(var definition)
    signal valueActivated(var value)
    signal makeVariableRequested()
    signal makeListRequested()
    signal listEditorStateRequested(string name, bool visible, int x, int y)
    signal renameListRequested(string name)
    signal deleteListRequested(string name)
    signal makeDictRequested()
    signal dictEditorStateRequested(string name, bool visible, int x, int y)
    signal renameDictRequested(string name)
    signal deleteDictRequested(string name)
    signal makeBlockRequested()
    // Optional: a library of shared blocks, and sharing a block as a pack.
    property bool blockLibraryEnabled: false
    signal browseBlocksRequested()
    signal shareBlockRequested(var definition)
    signal renameVariableRequested(string name)
    signal deleteVariableRequested(string name)
    signal editBlockRequested(var definition)
    signal deleteBlockRequested(string blockId)
    signal standaloneKeyCaptureRequested()
    signal appPickerRequested(var instruction)
    signal pieceActionRequested(string action, var instruction)
    signal detailsRequested(string name, string identifier, string explainer)
    implicitWidth: 318
    color: Theme.panel
    border.color: Theme.borderSoft

    function n(v) { return {kind:"Number",value:v}; }
    function t(v) { return {kind:"Text",value:v}; }
    function prefab(type) {
        const registered=BlockRegistry.prefab(type); if(registered) return registered;
        const i={id:"palette-"+type,type:type};
        if(type==="WhenBatteryDischargedTo") i.threshold=n(20); else if(type==="WhenBatteryChargedTo") i.threshold=n(100);
        else if(type==="WhenTime") i.schedule={kind:"Daily",hour:9,minute:0}; else if(type==="Wait") i.duration=n(1000);
        else if(type==="Text") i.text=t("text"); else if(type==="Key"){i.key=t(standaloneKey||"a");i.direction="Click";} else if(type==="WhenKey"){i.key=t(standaloneKey||"a");i.direction="Press";} else if(type==="Button"){i.button="Left";i.direction="Click";}
        else if(type==="MoveMouse"){i.x=n(0);i.y=n(0);i.coordinate="Relative";} else if(type==="Scroll"){i.amount=n(4);i.axis="Vertical";}
        else if(type==="Command") i.command=""; else if(type==="OpenApp"||type==="CloseApp"){i.name="";i.command="";i.icon=null;}
        else if(type==="SetClipboard") i.value=t("text");
        else if(type==="AddToList"){i.name=firstListName();i.value=t("thing");}
        else if(type==="DeleteOfList"){i.name=firstListName();i.index=n(1);}
        else if(type==="DeleteAllOfList"||type==="ReverseList")i.name=firstListName();
        else if(type==="ShiftList"){i.name=firstListName();i.amount=n(1);}
        else if(type==="InsertIntoList"){i.name=firstListName();i.value=t("thing");i.index=n(1);}
        else if(type==="ReplaceItemOfList"){i.name=firstListName();i.index=n(1);i.value=t("thing");}
        else if(type==="If"){i.condition={kind:"Bool"};i.body=[];} else if(type==="IfElse"){i.condition={kind:"Bool"};i.then_body=[];i.else_body=[];}
        else if(type==="Repeat"){i.count=n(10);i.body=[];} else if(type==="Forever"){i.body=[];} else if(type==="While"){i.condition={kind:"Bool"};i.body=[];}
        else if(type==="Return")i.value=n(0); else if(type==="SetVariable"){i.name=(variables||[])[0]||"variable";i.value=n(0);} else if(type==="ChangeVariable"){i.name=(variables||[])[0]||"variable";i.value=n(1);}
        return i;
    }
    function firstListName(){return lists&&lists.length?lists[0].name:"";}
    readonly property var operators: [
        ["Add","number",2],["Sub","number",2],["Mul","number",2],["Div","number",2],["Mod","number",2],["Round","number",1],["Math","number",2],["Random","number",2],
        ["Join","text",2],["Join3","text",3],["NewLine","text",0],["Tab","text",0],["IndexOf","text",2],["LastIndexOf","text",2],["LetterOf","text",2],["Length","text",1],["Case","text",2],
        ["Eq","bool",2],["Neq","bool",2],["Gt","bool",2],["Lt","bool",2],["Gte","bool",2],["Lte","bool",2],["And","bool",2],["Or","bool",2],["Not","bool",1],["True","bool",0],["False","bool",0],["CurrentTime","number",1]
    ]
    readonly property var listOperators: [["ListItem","text",2],["ListItemNumber","number",2],["ListAmount","number",2],["ListLength","number",1],["ListContains","bool",2],["ListItemExists","bool",2],["ListIsEmpty","bool",1],["ListAsJson","text",1]]
    readonly property var dictOperators: [["DictValue","text",2],["DictHasKey","bool",2],["DictSize","number",1],["DictKeys","text",1],["DictAsJson","text",1],["DictIsEmpty","bool",1]]
    // A fresh value for an [op, result, arity, args?] entry: the given args,
    // else blockstitch-core's defaults (see BlockRegistry.freshValue).
    function operatorValue(spec) {
        if(spec[3])return {kind:"Op",op:spec[0],args:JSON.parse(JSON.stringify(spec[3])),saved:n(0)};
        const fresh=BlockRegistry.freshValue(spec[0],{lists:(lists||[]).map(l=>l.name),dicts:(dicts||[]).map(d=>d.name)});
        if(fresh) return fresh;
        let args=[]; for(let i=0;i<spec[2];i++) args.push(spec[1]==="text"?t(""):n(0));
        return {kind:"Op",op:spec[0],args:args,saved:n(0)};
    }
    readonly property var communityPacks: {
        const packs = [];
        for (const definition of blockDefinitions || []) {
            if (!definition.read_only) continue;
            const memberships = definition.packs && definition.packs.length ? definition.packs
                : [{id:definition.pack_name || "community",name:definition.pack_name || qsTr("Community blocks")}];
            for (const membership of memberships) {
                let pack = packs.find(p => p.id === membership.id);
                if (!pack) {
                    pack = {id:membership.id,name:membership.name,blocks:[]};
                    packs.push(pack);
                }
                pack.blocks.push(definition);
            }
        }
        return packs.sort((a,b) => a.name.localeCompare(b.name));
    }

    function customArguments(definition){ return BlockRegistry.freshCallArgs(definition); }
    function customInstruction(definition){
        const args=customArguments(definition);
        const count=(definition.pieces||[]).filter(p=>p.kind==="Branch").length;
        const i={id:"palette-call",type:count?"BranchCallBlock":"CallBlock",block_id:definition.id,args:args};
        if(count)i.branches=Array.from({length:count},()=>[]);
        return i;
    }
    readonly property var customMenu: [{id:"edit",text:qsTr("Edit block"),icon:"blocks"},{id:"details",text:qsTr("Details"),icon:"info"}]
        .concat(root.blockLibraryEnabled?[{id:"share",text:qsTr("Share as a pack..."),icon:"package"}]:[])
        .concat([{id:"delete",text:qsTr("Delete block"),icon:"trash",danger:true}])
    function customMenuAction(id,definition){
        if(definition.read_only) return;
        if(id==="edit") root.editBlockRequested(definition);
        else if(id==="share") root.shareBlockRequested(definition);
        else if(id==="delete") root.deleteBlockRequested(definition.id);
    }
    function isReporter(definition){return definition.shape==="ReturnsValue"||definition.shape==="ReturnsBool";}
    function customValue(definition){
        const args=customArguments(definition);
        return {kind:"Call",block_id:definition.id,args:args,branches:[],saved:n(0)};
    }
    function customLabel(definition){return (definition.pieces||[]).filter(p=>p.kind!=="Branch").map(p=>p.kind==="Label"?p.text:"("+p.name+")").join(" ");}

    // ---- search and category filtering ----
    // rail | chips | accordion | flyout | tiles
    property string layout: "rail"
    property string category: "all"
    property bool flyoutOpen: false
    property bool paletteDragging: false
    property string searchText: ""
    FontMetrics { id: railMetrics; font.pixelSize:10 }
    readonly property int railWidth: {
        let widest = 0;
        for (const item of railModel) widest = Math.max(widest, railMetrics.advanceWidth(item.name));
        return Math.min(96, Math.max(54, Math.ceil(widest) + 20));
    }
    readonly property bool railVisible: layout==="rail"||layout==="flyout"
    readonly property bool flyoutShown: layout==="flyout"&&flyoutOpen&&!paletteDragging
    readonly property real contentWidth: Math.max(160, width-12-(railVisible?railWidth+1:0)-16)
    readonly property var railModel: layout==="flyout"?[{id:"search",name:"Search",icon:"search"}].concat(categories.slice(1)):categories
    onLayoutChanged: { searchText=""; flyoutOpen=false; category=layout==="accordion"?"input":"all"; }
    onDragStarted: paletteDragging=true
    onDragEnded: paletteDragging=false
    onDragCanceled: paletteDragging=false
    function railActivated(id) {
        if(layout==="flyout") {
            const target=id==="search"?"all":id;
            if(flyoutOpen&&category===target&&!searching) { flyoutOpen=false; return; }
            searchText=""; category=target; flyoutOpen=true;
            if(id==="search") Qt.callLater(()=>searchField.forceActiveFocus());
        } else { searchText=""; category=id; }
    }
    function categoryCount(c) {
        if(c==="custom" || c==="community") return (blockDefinitions||[]).filter(def => !!def.read_only === (c==="community")).length;
        let n=0;
        const types=[].concat(Array.from(instructionTypes||[]));
        types.forEach(t=>{ if(instructionCategory(t)===c) n++; });
        if(c==="data") n+=Array.from(listInstructionTypes||[]).length+listOperators.length+(dictsEnabled?dictInstructionTypes.length+dictOperators.length:0);
        allOperators.forEach(o=>{ if(operatorCategory(o[0])===c) n++; });
        return n;
    }
    // App-specific categories ({id, name, icon}), listed before Custom.
    property var appCategories: []
    readonly property var categories: [
        {id:"all",name:qsTr("All"),icon:"layout-grid"},{id:"events",name:qsTr("Events"),icon:"zap"},{id:"input",name:qsTr("Input"),icon:"keyboard"},
        {id:"control",name:qsTr("Flow"),icon:"git-branch"},{id:"data",name:qsTr("Data"),icon:"variable"},{id:"operators",name:qsTr("Operators"),icon:"equal"},
        {id:"system",name:qsTr("System"),icon:"settings"}
    ].concat(Array.from(appCategories || [])).concat([{id:"custom",name:qsTr("Custom"),icon:"blocks"}]).concat(root.blockLibraryEnabled?[{id:"community",name:qsTr("Community"),icon:"package"}]:[])
    readonly property var systemOperators: ["CurrentTime"]
    function instructionCategory(type) {
        if(extraCategories[type]) return extraCategories[type];
        if(type.indexOf("When")===0) return "events";
        if(["Text","Key","Button","MoveMouse","Scroll"].indexOf(type)>=0) return "input";
        if(["Wait","If","IfElse","Repeat","Forever","While","EscapeLoop","ContinueLoop","Return"].indexOf(type)>=0) return "control";
        if(["Command","OpenApp","CloseApp"].indexOf(type)>=0) return "system";
        return "data";
    }
    function operatorCategory(op) { return extraCategories[op] || (systemOperators.indexOf(op)>=0 ? "system" : "operators"); }
    function humanize(s) { return String(s).replace(/([a-z0-9])([A-Z])/g,"$1 $2").toLowerCase(); }
    function matches(label) { return humanize(label).indexOf(searchText.trim().toLowerCase())>=0; }
    readonly property bool searching: searchText.trim().length>0
    readonly property bool noResults: {
        if(!searching) return false;
        const labels=[].concat(Array.from(instructionTypes||[]),Array.from(listInstructionTypes||[]),
            dictsEnabled?Array.from(dictInstructionTypes||[]):[],
            allOperators.map(o=>o[0]),listOperators.map(o=>o[0]),dictsEnabled?dictOperators.map(o=>o[0]):[],
            Array.from(variables||[]),Array.from(blockDefinitions||[]).map(d=>customLabel(d)),communityPacks.map(p=>p.name));
        return !labels.some(l=>matches(l));
    }

    // The block list for one category ("" = every category). A Component so the
    // accordion can host one per open section.
    Component {
        id:blockListComponent
        Column {
            id:blk
            property string only: ""
            spacing:8
            function itemShown(c,label) { return root.searching ? root.matches(label) : (only===""||only===c); }
            function sectionShown(c) { return !root.searching && (only===""||only===c); }
                Repeater {
                    model:root.instructionTypes
                    delegate:PaletteBlock {
                        required property string modelData
                        visible:blk.itemShown(root.instructionCategory(modelData),modelData)
                        instruction:root.prefab(modelData); spec:({kind:"instruction",type:modelData,instruction:instruction})
                        variables:Array.from(root.variables||[]); lists:Array.from(root.lists||[]); blockDefinitions:root.blockDefinitions; keyCapture:root.keyCapture
                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                        onActivated:root.blockActivated(modelData,instruction); onKeyCaptureRequested:root.standaloneKeyCaptureRequested()
                        onAppPickerRequested: instruction => root.appPickerRequested(instruction)
                        onPieceActionRequested: (action, instruction) => root.pieceActionRequested(action, instruction)
                        onDetailsRequested:type=>root.detailsRequested(type,type,qsTr("See what this block does and where it can be used."))
                    }
                }
                Text { visible:blk.sectionShown("operators"); text:qsTr("OPERATORS"); color:Theme.textDim; font.pixelSize:10; font.weight:Font.Bold; font.letterSpacing:1; topPadding:8 }
                Repeater {
                    model:root.allOperators
                    delegate:PaletteValue {
                        required property var modelData
                        visible:blk.itemShown(root.operatorCategory(modelData[0]),modelData[0])
                        valueData:root.operatorValue(modelData); spec:({kind:"value",value:valueData})
                        blockDefinitions:root.blockDefinitions
                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                        onActivated:root.valueActivated(valueData)
                        onDetailsRequested:kind=>root.detailsRequested(kind,kind,qsTr("A value or operator block that can be placed in an input."))
                    }
                }
                Row {
                    visible:blk.sectionShown("data"); spacing:8; topPadding:8
                    Text { text:qsTr("VARIABLES"); color:Theme.textDim; font.pixelSize:10; font.weight:Font.Bold; font.letterSpacing:1; anchors.verticalCenter:parent.verticalCenter }
                    BwButton { text:qsTr("Make a Variable"); implicitHeight:Math.max(28,Theme.minimumTarget); font.pixelSize:11; onClicked:root.makeVariableRequested() }
                }
                Repeater {
                    model:root.variables||[]
                    delegate:PaletteValue {
                        required property string modelData
                        visible:blk.itemShown("data",modelData)
                        valueData:({kind:"Var",name:modelData}); editable:false; spec:({kind:"value",value:valueData})
                        blockDefinitions:root.blockDefinitions
                        extraMenu:[{id:"rename",text:qsTr("Rename variable"),icon:"equal"},{id:"delete",text:qsTr("Delete variable"),icon:"trash",danger:true}]
                        onExtraMenuTriggered:id=>id==="rename"?root.renameVariableRequested(modelData):root.deleteVariableRequested(modelData)
                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                        onActivated:root.valueActivated(valueData)
                    }
                }
                Row {
                    visible:blk.sectionShown("data"); spacing:8; topPadding:8
                    Text { text:qsTr("LISTS"); color:Theme.textDim; font.pixelSize:10; font.weight:Font.Bold; font.letterSpacing:1; anchors.verticalCenter:parent.verticalCenter }
                    BwButton { text:qsTr("Make a List"); implicitHeight:Math.max(28,Theme.minimumTarget); font.pixelSize:11; onClicked:root.makeListRequested() }
                }
                CollectionPanel {
                    visible:blk.sectionShown("data")
                    collections:root.lists; noun:"list"; rowWidth:Math.max(230,root.width-28-root.railWidth-1)
                    onEditorStateRequested:(name,visible,x,y)=>root.listEditorStateRequested(name,visible,x,y)
                    onRenameRequested:name=>root.renameListRequested(name)
                    onDeleteRequested:name=>root.deleteListRequested(name)
                    onDetailsRequested:name=>root.detailsRequested(name,"List",qsTr("A document-wide ordered collection of text and number items."))
                }
                Repeater {
                    model:root.listOperators
                    delegate:PaletteValue {
                        required property var modelData
                        visible:blk.itemShown("data",modelData[0])
                        valueData:root.operatorValue(modelData); spec:({kind:"value",value:valueData})
                        blockDefinitions:root.blockDefinitions
                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                        onActivated:root.valueActivated(valueData)
                        onDetailsRequested:kind=>root.detailsRequested(kind,kind,qsTr("A list reporter block that reads data without changing the list."))
                    }
                }
                Repeater {
                    model:root.listInstructionTypes
                    delegate:PaletteBlock {
                        required property string modelData
                        visible:blk.itemShown("data",modelData)
                        instruction:root.prefab(modelData); spec:({kind:"instruction",type:modelData,instruction:instruction})
                        variables:Array.from(root.variables||[]); lists:Array.from(root.lists||[]); blockDefinitions:root.blockDefinitions; keyCapture:root.keyCapture
                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                        onActivated:root.blockActivated(modelData)
                        onDetailsRequested:type=>root.detailsRequested(type,type,qsTr("See what this block does and where it can be used."))
                    }
                }
                Row {
                    visible:root.dictsEnabled && blk.sectionShown("data"); spacing:8; topPadding:8
                    Text { text:qsTr("DICTS"); color:Theme.textDim; font.pixelSize:10; font.weight:Font.Bold; font.letterSpacing:1; anchors.verticalCenter:parent.verticalCenter }
                    BwButton { text:qsTr("Make a Dict"); implicitHeight:Math.max(28,Theme.minimumTarget); font.pixelSize:11; onClicked:root.makeDictRequested() }
                }
                CollectionPanel {
                    visible:root.dictsEnabled && blk.sectionShown("data")
                    collections:root.dicts; noun:"dict"; rowWidth:Math.max(230,root.width-28-root.railWidth-1)
                    emptyText:qsTr("Make a dict, then check it to open its editor on the canvas.")
                    onEditorStateRequested:(name,visible,x,y)=>root.dictEditorStateRequested(name,visible,x,y)
                    onRenameRequested:name=>root.renameDictRequested(name)
                    onDeleteRequested:name=>root.deleteDictRequested(name)
                    onDetailsRequested:name=>root.detailsRequested(name,"Dict",qsTr("A document-wide collection of text and number values, looked up by key."))
                }
                Repeater {
                    model:root.dictsEnabled?root.dictOperators:[]
                    delegate:PaletteValue {
                        required property var modelData
                        visible:blk.itemShown("data",modelData[0])
                        valueData:root.operatorValue(modelData); spec:({kind:"value",value:valueData})
                        blockDefinitions:root.blockDefinitions
                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                        onActivated:root.valueActivated(valueData)
                        onDetailsRequested:kind=>root.detailsRequested(kind,kind,qsTr("A dict reporter block that reads data without changing the dict."))
                    }
                }
                Repeater {
                    model:root.dictsEnabled?root.dictInstructionTypes:[]
                    delegate:PaletteBlock {
                        required property string modelData
                        visible:blk.itemShown("data",modelData)
                        instruction:root.prefab(modelData); spec:({kind:"instruction",type:modelData,instruction:instruction})
                        variables:Array.from(root.variables||[]); lists:Array.from(root.lists||[]); blockDefinitions:root.blockDefinitions; keyCapture:root.keyCapture
                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                        onActivated:root.blockActivated(modelData)
                        onDetailsRequested:type=>root.detailsRequested(type,type,qsTr("See what this block does and where it can be used."))
                    }
                }
                Row {
                    visible:blk.sectionShown("custom"); spacing:8; topPadding:8
                    Text { text:qsTr("MY BLOCKS"); color:Theme.textDim; font.pixelSize:10; font.weight:Font.Bold; font.letterSpacing:1; anchors.verticalCenter:parent.verticalCenter }
                    BwButton { text:qsTr("Make a Block"); implicitHeight:Math.max(28,Theme.minimumTarget); font.pixelSize:11; onClicked:root.makeBlockRequested() }
                }
                Repeater {
                    model:(root.blockDefinitions||[]).filter(def=>!def.read_only)
                    delegate:Loader {
                        id:customItem; required property var modelData
                        visible:blk.itemShown("custom",root.customLabel(modelData))
                        sourceComponent:root.isReporter(modelData)?customReporter:customCall
                        Component {
                            id:customCall
                            PaletteBlock {
                                instruction:root.customInstruction(customItem.modelData); spec:({kind:"custom",definition:customItem.modelData,instruction:instruction,color:blockColor})
                                variables:Array.from(root.variables||[]); lists:Array.from(root.lists||[]); blockDefinitions:root.blockDefinitions; keyCapture:root.keyCapture; blockColor:customItem.modelData.color||Theme.block
                                onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                                onActivated:root.customBlockActivated(customItem.modelData)
                                onDetailsRequested:root.detailsRequested(root.customLabel(customItem.modelData),customItem.modelData.id,qsTr("A custom block defined in this macro."))
                                extraMenu:root.customMenu
                                onExtraMenuTriggered:id=>root.customMenuAction(id,customItem.modelData)
                            }
                        }
                        Component {
                            id:customReporter
                            PaletteValue {
                                valueData:root.customValue(customItem.modelData); editable:false; forceBoolean:customItem.modelData.shape==="ReturnsBool"; callLabel:root.customLabel(customItem.modelData)
                                blockDefinitions:root.blockDefinitions
                                spec:({kind:"value",value:valueData,forceBoolean:forceBoolean,label:callLabel})
                                onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                                onActivated:root.valueActivated(valueData)
                                onDetailsRequested:root.detailsRequested(root.customLabel(customItem.modelData),customItem.modelData.id,qsTr("A custom reporter block defined in this macro."))
                                extraMenu:root.customMenu
                                onExtraMenuTriggered:id=>root.customMenuAction(id,customItem.modelData)
                            }
                        }
                    }
                }
                Row {
                    visible:root.blockLibraryEnabled && blk.sectionShown("community"); spacing:8; topPadding:8
                    Text { text:qsTr("COMMUNITY BLOCKS"); color:Theme.textDim; font.pixelSize:10; font.weight:Font.Bold; font.letterSpacing:1; anchors.verticalCenter:parent.verticalCenter }
                    BwButton { text:qsTr("Browse"); iconName:"book-open"; implicitHeight:Math.max(28,Theme.minimumTarget); font.pixelSize:11; onClicked:root.browseBlocksRequested() }
                }
                Repeater {
                    model:root.communityPacks
                    delegate:Column {
                        id: packSection
                        required property var modelData
                        readonly property bool matchesPack: root.searching && root.matches(modelData.name)
                        visible: modelData.blocks.some(def => matchesPack || blk.itemShown("community",root.customLabel(def)))
                        spacing:8; topPadding:6
                        Text {
                            id: packHeading
                            objectName: "communityPackHeading"
                            width: root.contentWidth
                            text: packSection.modelData.name
                            color: Theme.textDim; font.pixelSize:12; font.weight:Font.DemiBold
                            elide: Text.ElideRight
                            ToolTip.visible: headingHover.hovered && truncated
                            ToolTip.text: text
                            HoverHandler { id: headingHover }
                        }
                        Repeater {
                            model: packSection.modelData.blocks
                            delegate:Loader {
                                id:communityItem; required property var modelData
                                visible:packSection.matchesPack || blk.itemShown("community",root.customLabel(modelData))
                                sourceComponent:root.isReporter(modelData)?communityReporter:communityCall
                                Component {
                                    id:communityCall
                                    PaletteBlock {
                                        instruction:root.customInstruction(communityItem.modelData); spec:({kind:"custom",definition:communityItem.modelData,instruction:instruction,color:blockColor})
                                        variables:Array.from(root.variables||[]); lists:Array.from(root.lists||[]); blockDefinitions:root.blockDefinitions; keyCapture:root.keyCapture; blockColor:communityItem.modelData.color||Theme.block
                                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                                        onActivated:root.customBlockActivated(communityItem.modelData)
                                        onDetailsRequested:root.detailsRequested(root.customLabel(communityItem.modelData),communityItem.modelData.id,qsTr("A custom block defined in this macro."))
                                        extraMenu:[{id:"details",text:qsTr("Details"),icon:"info"}]
                                        onExtraMenuTriggered:id=>root.customMenuAction(id,communityItem.modelData)
                                    }
                                }
                                Component {
                                    id:communityReporter
                                    PaletteValue {
                                        valueData:root.customValue(communityItem.modelData); editable:false; forceBoolean:communityItem.modelData.shape==="ReturnsBool"; callLabel:root.customLabel(communityItem.modelData)
                                        blockDefinitions:root.blockDefinitions
                                        spec:({kind:"value",value:valueData,forceBoolean:forceBoolean,label:callLabel})
                                        onDragStarted:(sp,sx,sy,ox,oy)=>root.dragStarted(sp,sx,sy,ox,oy); onDragMoved:(sx,sy)=>root.dragMoved(sx,sy); onDragEnded:(sx,sy)=>root.dragEnded(sx,sy); onDragCanceled:root.dragCanceled()
                                        onActivated:root.valueActivated(valueData)
                                        onDetailsRequested:root.detailsRequested(root.customLabel(communityItem.modelData),communityItem.modelData.id,qsTr("A custom reporter block defined in this macro."))
                                        extraMenu:[{id:"details",text:qsTr("Details"),icon:"info"}]
                                        onExtraMenuTriggered:id=>root.customMenuAction(id,communityItem.modelData)
                                    }
                                }
                            }
                        }
                    }
                }
            Text {
                visible:root.noResults; text:qsTr("No blocks match \"%1\"").arg(root.searchText.trim())
                color:Theme.textDim; font.pixelSize:12; topPadding:8
            }
        }
    }

    RowLayout {
        anchors.fill:parent; spacing:0
        Flickable {
            id:rail
            visible:root.railVisible
            Layout.preferredWidth:root.railWidth; Layout.fillHeight:true; clip:true
            contentHeight:railColumn.height; interactive:contentHeight>height; boundsBehavior:Flickable.StopAtBounds
            // Only switch once the pointer rests on an icon, so sweeping across the rail doesn't flicker
            Timer {
                id:hoverIntent; interval:140
                property string pending: ""
                onTriggered: if (!root.searching && root.layout==="rail") root.category = pending
            }
            Column {
                id:railColumn
                width:parent.width; spacing:6; topPadding:6
                Repeater {
                    model:root.railModel
                    delegate:Rectangle {
                        required property var modelData
                        readonly property bool current: root.layout==="flyout"
                            ? root.flyoutOpen && !root.searching && (modelData.id==="search"?root.category==="all":root.category===modelData.id)
                            : !root.searching && root.category===modelData.id
                        width:railColumn.width-8; x:4; height:Math.max(46,Theme.minimumTarget); radius:6
                        color:current?Theme.panelRaised:(railHover.hovered?Theme.field:"transparent")
                        opacity:root.searching&&root.layout==="rail"?0.5:1
                        Column {
                            anchors.centerIn:parent; width:parent.width-8; spacing:3
                            LucideIcon { anchors.horizontalCenter:parent.horizontalCenter; width:18; height:18; name:modelData.icon; color:current?Theme.accent:Theme.textDim }
                            Text {
                                id: categoryLabel
                                objectName: "paletteCategoryLabel"
                                width:parent.width; horizontalAlignment:Text.AlignHCenter
                                text:modelData.name; font.pixelSize:10; color:current?Theme.text:Theme.textDim
                                elide:Text.ElideRight
                            }
                        }
                        ToolTip.visible:railHover.hovered && categoryLabel.truncated
                        ToolTip.text:modelData.name
                        HoverHandler {
                            id:railHover
                            onHoveredChanged: {
                                if (root.layout!=="rail") return;
                                if (hovered && !root.searching) { hoverIntent.pending=modelData.id; hoverIntent.restart(); }
                                else if (hoverIntent.pending===modelData.id) hoverIntent.stop();
                            }
                        }
                        TapHandler { onTapped:root.railActivated(modelData.id) }
                    }
                }
            }
        }
        Rectangle { visible:root.railVisible; Layout.preferredWidth:1; Layout.fillHeight:true; color:Theme.borderSoft }
        ColumnLayout {
            id:contentColumn
            visible:root.layout!=="flyout"||root.flyoutOpen
            Layout.fillWidth:true; Layout.fillHeight:true; spacing:0; clip:true
            BwTextField {
                id:searchField
                Layout.fillWidth:true; Layout.margins:8; Layout.bottomMargin:4
                placeholderText:qsTr("Search blocks"); text:root.searchText
                onTextChanged:root.searchText=text
                rightPadding:30
                LucideIcon {
                    anchors.right:parent.right; anchors.rightMargin:8; anchors.verticalCenter:parent.verticalCenter
                    width:14; height:14; name:root.searching?"x":"search"; color:Theme.textDim
                    MouseArea { anchors.fill:parent; anchors.margins:-4; enabled:root.searching; onClicked:root.searchText="" }
                }
            }
            Flow {
                visible:root.layout==="chips"
                Layout.fillWidth:true; Layout.leftMargin:8; Layout.rightMargin:8; Layout.bottomMargin:6; spacing:6
                Repeater {
                    model:root.categories
                    delegate:Rectangle {
                        required property var modelData
                        readonly property bool current:!root.searching&&root.category===modelData.id
                        height:26; radius:13; width:chipLabel.implicitWidth+20
                        color:current?Theme.panelRaised:"transparent"
                        border.color:current?Theme.accent:Theme.borderSoft
                        Text { id:chipLabel; anchors.centerIn:parent; text:modelData.name; font.pixelSize:12; color:current?Theme.text:Theme.textDim }
                        HoverHandler { cursorShape:Qt.PointingHandCursor }
                        TapHandler { onTapped:{root.searchText="";root.category=modelData.id;} }
                    }
                }
            }
            Rectangle {
                visible:root.layout==="tiles"&&!root.searching&&root.category!=="all"
                Layout.fillWidth:true; Layout.leftMargin:8; Layout.rightMargin:8; Layout.bottomMargin:6; implicitHeight:Math.max(30,Theme.minimumTarget); radius:6
                color:backHover.hovered?Theme.field:"transparent"
                Row {
                    anchors.verticalCenter:parent.verticalCenter; anchors.left:parent.left; anchors.leftMargin:6; spacing:6
                    LucideIcon { width:16; height:16; name:"chevron-left"; color:Theme.textDim; anchors.verticalCenter:parent.verticalCenter }
                    Text {
                        anchors.verticalCenter:parent.verticalCenter; color:Theme.text; font.pixelSize:13; font.weight:Font.Medium
                        text:{ for(const c of root.categories) if(c.id===root.category) return c.name; return ""; }
                    }
                }
                Text { anchors.right:parent.right; anchors.rightMargin:8; anchors.verticalCenter:parent.verticalCenter; text:qsTr("All categories"); color:Theme.textDim; font.pixelSize:11 }
                HoverHandler { id:backHover; cursorShape:Qt.PointingHandCursor }
                TapHandler { onTapped:root.category="all" }
            }
            ScrollView {
                id:paletteScroll
                // Back at the top whenever the category changes (e.g. returning to the tile grid)
                Connections { target:root; function onCategoryChanged(){ paletteScroll.contentItem.contentY=0; } }
                Layout.fillWidth:true; Layout.fillHeight:true; clip:true
                ScrollBar.horizontal.policy: ScrollBar.AsNeeded
                // Flickable must not also handle the wheel, or it overrides the faster pan below
                Binding { target:paletteScroll.contentItem; property:"interactive"; value:!paletteWheel.active }
                WheelHandler { id:paletteWheel; target:null; function touchpad(e){return paletteSmooth.isTouchpad(e);} function pan(ad,pd,tp){if(!tp)return ad/120*Qt.styleHints.wheelScrollLines*24;const d=ad!==0?ad/12:pd;return d*Math.min(24,6+1.5*Math.abs(d));} acceptedDevices:PointerDevice.Mouse|PointerDevice.TouchPad; onWheel:event=>{const tp=paletteWheel.touchpad(event);paletteSmooth.scrollBy(0,paletteWheel.pan(event.angleDelta.y,event.pixelDelta.y,tp),!tp);event.accepted=true;} }
                SmoothScroll { id:paletteSmooth; flickable:paletteScroll.contentItem }
                Column {
                    width:Math.max(root.contentWidth+16, childrenRect.width); spacing:8; padding:8
                    // Tiles: top-level category grid
                    Grid {
                        visible:root.layout==="tiles"&&!root.searching&&root.category==="all"
                        columns:2; spacing:8
                        Repeater {
                            model:root.layout==="tiles"?root.categories.slice(1):[]
                            delegate:Rectangle {
                                required property var modelData
                                width:Math.floor((root.contentWidth-8)/2); height:74; radius:8
                                color:tileHover.hovered?Theme.panelRaised:Theme.field; border.color:Theme.borderSoft
                                Column {
                                    anchors.centerIn:parent; spacing:3
                                    LucideIcon { anchors.horizontalCenter:parent.horizontalCenter; width:22; height:22; name:modelData.icon; color:Theme.text }
                                    Text { anchors.horizontalCenter:parent.horizontalCenter; text:modelData.name; color:Theme.text; font.pixelSize:12 }
                                    Text { anchors.horizontalCenter:parent.horizontalCenter; text:qsTr("%n block(s)","",root.categoryCount(modelData.id)); color:Theme.textDim; font.pixelSize:10 }
                                }
                                HoverHandler { id:tileHover; cursorShape:Qt.PointingHandCursor }
                                TapHandler { onTapped:root.category=modelData.id }
                            }
                        }
                    }
                    // Accordion: one header per category, one section open at a time
                    Column {
                        visible:root.layout==="accordion"&&!root.searching
                        spacing:4
                        Repeater {
                            model:root.layout==="accordion"?root.categories.slice(1):[]
                            delegate:Column {
                                id:section
                                required property var modelData
                                readonly property bool expanded:root.category===modelData.id
                                spacing:6
                                Rectangle {
                                    width:root.contentWidth; height:34; radius:6
                                    color:headerHover.hovered?Theme.field:"transparent"
                                    Row {
                                        anchors.verticalCenter:parent.verticalCenter; anchors.left:parent.left; anchors.leftMargin:6; spacing:8
                                        LucideIcon { width:15; height:15; name:section.expanded?"chevron-down":"chevron-right"; color:Theme.textDim; anchors.verticalCenter:parent.verticalCenter }
                                        Text { text:section.modelData.name; color:section.expanded?Theme.text:Theme.textDim; font.pixelSize:13; font.weight:section.expanded?Font.Medium:Font.Normal; anchors.verticalCenter:parent.verticalCenter }
                                    }
                                    Text { anchors.right:parent.right; anchors.rightMargin:8; anchors.verticalCenter:parent.verticalCenter; text:root.categoryCount(section.modelData.id); color:Theme.textDim; font.pixelSize:11 }
                                    HoverHandler { id:headerHover; cursorShape:Qt.PointingHandCursor }
                                    TapHandler { onTapped:root.category=section.expanded?"all":section.modelData.id }
                                }
                                Loader {
                                    id:sectionList
                                    active:section.expanded
                                    sourceComponent:blockListComponent
                                    Binding { target:sectionList.item; property:"only"; value:section.modelData.id; when:sectionList.status===Loader.Ready }
                                }
                            }
                        }
                    }
                    // Everything else: one flat list (a category, all, or search results)
                    Loader {
                        id:flatList
                        active:root.searching||(root.layout!=="accordion"&&!(root.layout==="tiles"&&root.category==="all"))
                        sourceComponent:blockListComponent
                        Binding { target:flatList.item; property:"only"; value:root.searching||root.category==="all"?"":root.category; when:flatList.status===Loader.Ready }
                    }
                    Item { width:1;height:8 }
                }
            }
        }
    }
    // Delete target, shown only while something is dragged from the canvas.
    Rectangle {
        anchors.fill: parent; z: 4; radius: root.radius
        color: root.trashArmed ? Qt.alpha(Theme.dangerBackground, 0.96) : Qt.alpha(Theme.panel, 0.96)
        opacity: root.trashActive ? 1 : 0; visible: opacity > 0
        Behavior on opacity { enabled: Motion.enabled; NumberAnimation { duration: Motion.quick } }
        Shape {
            id: trashOutline
            anchors.fill: parent; anchors.margins: 10
            ShapePath {
                strokeColor: root.trashArmed ? Theme.danger : Theme.border; strokeWidth: 1.5
                strokeStyle: ShapePath.DashLine; dashPattern: [4, 3]
                fillColor: root.trashArmed ? "#1fff4848" : "transparent"
                startX: 6; startY: 0
                PathLine { x: trashOutline.width - 6; y: 0 }
                PathQuad { x: trashOutline.width; y: 6; controlX: trashOutline.width; controlY: 0 }
                PathLine { x: trashOutline.width; y: trashOutline.height - 6 }
                PathQuad { x: trashOutline.width - 6; y: trashOutline.height; controlX: trashOutline.width; controlY: trashOutline.height }
                PathLine { x: 6; y: trashOutline.height }
                PathQuad { x: 0; y: trashOutline.height - 6; controlX: 0; controlY: trashOutline.height }
                PathLine { x: 0; y: 6 }
                PathQuad { x: 6; y: 0; controlX: 0; controlY: 0 }
            }
        }
        Column {
            anchors.centerIn: parent; spacing: 8
            LucideIcon { anchors.horizontalCenter: parent.horizontalCenter; name: "trash"; width: 30; height: 30; color: root.trashArmed ? Theme.danger : Theme.text }
            Text { anchors.horizontalCenter: parent.horizontalCenter; text: root.trashArmed ? qsTr("Release to delete") : qsTr("Drop here to delete"); color: root.trashArmed ? Theme.danger : Theme.text; font.pixelSize: 14 }
        }
    }

    // Drag the right edge to resize the panel.
    Rectangle {
        anchors.top: parent.top; anchors.bottom: parent.bottom; anchors.right: parent.right
        width: 6; z: 5; visible: root.layout !== "flyout" || root.flyoutShown
        color: Theme.accent; opacity: resizeArea.pressed || resizeArea.containsMouse ? 0.5 : 0
        MouseArea {
            id: resizeArea
            anchors.fill: parent; hoverEnabled: true; preventStealing: true; cursorShape: Qt.SizeHorCursor
            property real startWidth: 0
            property real startX: 0
            onPressed: mouse => { startWidth = root.width; startX = mapToItem(null, mouse.x, 0).x; }
            onPositionChanged: mouse => { if (pressed) root.resizeRequested(startWidth + mapToItem(null, mouse.x, 0).x - startX); }
        }
    }
}
