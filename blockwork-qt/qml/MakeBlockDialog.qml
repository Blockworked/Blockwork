import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import com.blockworked.Blockstitch 1.0

BwDialog {
    id: root
    title: "Make a Block"
    width: 720
    height: Math.min(760, parent ? parent.height - 32 : 760)
    standardButtons: Dialog.NoButton

    signal createRequested(var pieces, string shape, string color)

    property int selectedPiece: 0
    property string blockShape: "Normal"
    property string blockColor: "#4C97FF"
    property string validationError: ""
    readonly property bool returnsValue: blockShape === "ReturnsValue" || blockShape === "ReturnsBool"
    readonly property bool hasBranches: branchCount() > 0
    readonly property var colorPresets: [
        "#4C97FF", "#9966FF", "#C65BCF", "#FFBF00", "#FFAB19", "#5BA9D0",
        "#59C059", "#FF8C1A", "#FF5B1F", "#FF6680", "#19B88E", "#FF4D4F",
        "#FF7F7F", "#FFB77B", "#FFF28A", "#8BF77A", "#78F0B0", "#70D5E8",
        "#7DB5F5", "#8080F5", "#C667E8", "#F27AED", "#B3B3B3"
    ]

    ListModel { id: pieces }
    // One entry per branch piece: its model index, name and the separator label shown below it.
    property var branchInfo: []
    function refreshBranches() {
        const out = [];
        for (let i = 0; i < pieces.count; ++i) {
            if (pieces.get(i).pieceKind !== "Branch") continue;
            let sep = "";
            for (let j = i + 1; j < pieces.count && pieces.get(j).pieceKind !== "Branch"; ++j)
                if (pieces.get(j).pieceKind === "Label") { sep = pieces.get(j).pieceName; break; }
            out.push({ index: i, name: pieces.get(i).pieceName, sep: sep, sepIndex: -1 });
        }
        branchInfo = out;
    }
    Connections {
        target: pieces
        function onCountChanged() { root.refreshBranches(); }
        function onDataChanged() { root.refreshBranches(); }
        function onRowsMoved() { root.refreshBranches(); }
    }

    function uuid() {
        return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, c => {
            const r = Math.random() * 16 | 0;
            return (c === "x" ? r : (r & 3 | 8)).toString(16);
        });
    }
    function resetForm() {
        pieces.clear();
        pieces.append({ pieceKind: "Label", pieceName: "block name", valueType: "Any" });
        selectedPiece = 0; blockShape = "Normal"; blockColor = "#4C97FF"; validationError = "";
        pieceEditor.text = "block name";
    }
    function branchCount() {
        let count = 0;
        for (let i = 0; i < pieces.count; ++i) if (pieces.get(i).pieceKind === "Branch") count++;
        return count;
    }
    function firstBranchIndex() {
        for (let i = 0; i < pieces.count; ++i) if (pieces.get(i).pieceKind === "Branch") return i;
        return -1;
    }
    function nextName(prefix) {
        let n = 1;
        while (true) {
            const candidate = prefix + n;
            let used = false;
            for (let i = 0; i < pieces.count; ++i)
                if (pieces.get(i).pieceKind !== "Label" && pieces.get(i).pieceName === candidate) used = true;
            if (!used) return candidate;
            n++;
        }
    }
    function addPiece(kind, valueType) {
        if (kind === "Branch" && hasBranches)
            pieces.append({ pieceKind: "Label", pieceName: "", valueType: "Any" });
        const name = kind === "Label" ? "label" : nextName(kind === "Branch" ? "branch" : "value");
        const insertAt = kind === "Label" && hasBranches ? firstBranchIndex() : pieces.count;
        pieces.insert(insertAt, { pieceKind: kind, pieceName: name, valueType: valueType || "Any" });
        selectedPiece = insertAt;
        syncEditor();
        pieceEditor.forceActiveFocus(); pieceEditor.selectAll();
    }
    function syncEditor() {
        pieceEditor.text = pieces.count && selectedPiece >= 0 && selectedPiece < pieces.count
                ? pieces.get(selectedPiece).pieceName : "";
    }
    function selectPiece(index) { selectedPiece = index; syncEditor(); }
    function commitPiece() {
        if (pieces.count && selectedPiece >= 0 && selectedPiece < pieces.count)
            pieces.setProperty(selectedPiece, "pieceName", pieceEditor.text);
    }
    function moveSelected(delta) {
        const target = selectedPiece + delta;
        if (target < 0 || target >= pieces.count) return;
        commitPiece(); pieces.move(selectedPiece, target, 1); selectedPiece = target; syncEditor();
    }
    function removeSelected() {
        if (!pieces.count) return;
        pieces.remove(selectedPiece);
        selectedPiece = Math.max(0, Math.min(selectedPiece, pieces.count - 1)); syncEditor();
    }
    function resultPieces() {
        const out = [];
        for (let i = 0; i < pieces.count; ++i) {
            const p = pieces.get(i); const name = p.pieceName.trim();
            if (p.pieceKind === "Label") out.push({ kind: "Label", id: uuid(), text: name });
            else if (p.pieceKind === "Branch") out.push({ kind: "Branch", id: uuid(), name: name });
            else out.push({ kind: "Input", id: uuid(), name: name, value_type: p.valueType });
        }
        return out;
    }
    function submit() {
        commitPiece();
        let hasLabel = false; const names = ({});
        for (let i = 0; i < pieces.count; ++i) {
            const p = pieces.get(i); const text = p.pieceName.trim();
            if (p.pieceKind === "Label") { if (text.length) hasLabel = true; continue; }
            if (!text.length) { validationError = "Every input and branch needs a name"; return; }
            if (names[text]) { validationError = "Input and branch names must be unique"; return; }
            names[text] = true;
        }
        if (!hasLabel) { validationError = "Give the block a name"; return; }
        createRequested(resultPieces(), blockShape, blockColor); accept();
    }
    onOpened: resetForm()

    contentItem: ColumnLayout {
        spacing: 12

        Rectangle {
            Layout.fillWidth: true; Layout.fillHeight: true; Layout.preferredHeight: 260; Layout.minimumHeight: 170
            radius: Theme.radius; color: Theme.canvas; border.color: Theme.border; clip: true
            Canvas {
                anchors.fill: parent
                onPaint: {
                    const c = getContext("2d"); c.reset(); c.fillStyle = "#45464b";
                    for (let x = 11; x < width; x += 22) for (let y = 11; y < height; y += 22) {
                        c.beginPath(); c.arc(x, y, 1.1, 0, Math.PI * 2); c.fill();
                    }
                }
            }

            Column {
                anchors.centerIn: parent; spacing: 8
                scale: Math.min(1, (parent.height - 16) / implicitHeight)
                Row {
                    anchors.horizontalCenter: parent.horizontalCenter; spacing: 2
                    BwButton { iconName: "arrow-left"; text: ""; implicitWidth: 28; implicitHeight: 27; enabled: root.selectedPiece > 0; onClicked: root.moveSelected(-1) }
                    BwButton { iconName: "trash"; text: ""; danger: true; implicitWidth: 28; implicitHeight: 27; enabled: pieces.count > 0; onClicked: root.removeSelected() }
                    BwButton { iconName: "chevron-down"; text: ""; implicitWidth: 28; implicitHeight: 27; enabled: root.selectedPiece < pieces.count - 1; onClicked: root.moveSelected(1) }
                }

                Item {
                    visible: !root.hasBranches
                    width: Math.max(230, previewRow.implicitWidth + 64); height: visible ? 62 : 0
                    BlockSurface { anchors.fill: parent; shape: root.blockShape === "Ending" ? "cap" : "stack"; fill: root.blockColor }
                    Row {
                        id: previewRow; x: 42; y: 15; spacing: 4
                        LucideIcon { name: "blocks"; color: Theme.textDim; width: 16; height: 16; anchors.verticalCenter: parent.verticalCenter }
                        Repeater {
                            model: pieces
                            delegate: Rectangle {
                                required property int index
                                required property string pieceKind
                                required property string pieceName
                                required property string valueType
                                visible: pieceKind !== "Branch"
                                height: 29; width: visible ? pieceText.implicitWidth + (pieceKind === "Label" ? 12 : 22) : 0
                                radius: valueType === "Bool" ? 14 : 5
                                color: valueType === "Bool" ? "transparent" : pieceKind === "Label" ? "transparent" : Theme.field
                                border.width: root.selectedPiece === index ? 2 : (pieceKind === "Label" ? 0 : 1)
                                border.color: root.selectedPiece === index ? Theme.accent : Theme.border
                                Canvas {
                                    anchors.fill: parent; visible: valueType === "Bool"
                                    onPaint: {
                                        const c = getContext("2d"); c.reset(); c.beginPath(); const n = Math.min(height * .32, width / 2);
                                        c.moveTo(n, .5); c.lineTo(width - n, .5); c.lineTo(width - .5, height / 2);
                                        c.lineTo(width - n, height - .5); c.lineTo(n, height - .5); c.lineTo(.5, height / 2); c.closePath();
                                        c.fillStyle = Theme.field; c.fill(); c.strokeStyle = root.selectedPiece === index ? Theme.accent : Theme.border; c.lineWidth = root.selectedPiece === index ? 2 : 1; c.stroke();
                                    }
                                }
                                Text { id: pieceText; anchors.centerIn: parent; text: pieceKind === "Label" ? (pieceName || "Add label") : pieceName; color: pieceKind === "Label" ? Theme.text : Theme.accent; font.pixelSize: 13; font.weight: pieceKind === "Label" ? Font.Normal : Font.DemiBold }
                                TapHandler { onTapped: root.selectPiece(index); onDoubleTapped: { root.selectPiece(index); pieceEditor.forceActiveFocus(); pieceEditor.selectAll(); } }
                            }
                        }
                    }
                }

                Item {
                    id: wrapPreview
                    visible: root.hasBranches
                    readonly property real mouthHeight: 46
                    width: Math.max(270, branchHead.implicitWidth + 70)
                    height: visible ? 50 + root.branchInfo.length * mouthHeight + Math.max(0, root.branchInfo.length - 1) * 34 + 26 + 8 : 0
                    BlockSurface {
                        anchors.fill: parent; shape: "wrap"; fill: root.blockColor
                        headHeight: 50; midHeight: 34; footHeight: 26; spine: 20
                        mouthHeights: root.branchInfo.map(() => wrapPreview.mouthHeight)
                    }
                    Row {
                        id: branchHead; x: 18; y: 12; spacing: 5
                        LucideIcon { name: "blocks"; color: Theme.textDim; width: 16; height: 16; anchors.verticalCenter: parent.verticalCenter }
                        Repeater {
                            model: pieces
                            delegate: Rectangle {
                                required property int index; required property string pieceKind; required property string pieceName; required property string valueType
                                visible: index < root.firstBranchIndex() || pieceKind === "Input"
                                width: visible ? label.implicitWidth + (pieceKind === "Label" ? 8 : 18) : 0; height: 27; radius: valueType === "Bool" ? 13 : 5
                                color: pieceKind === "Input" ? Theme.field : "transparent"
                                border.width: root.selectedPiece === index ? 2 : (pieceKind === "Input" ? 1 : 0)
                                border.color: root.selectedPiece === index ? Theme.accent : Theme.border
                                Text { id: label; anchors.centerIn: parent; text: pieceName || "Add label"; color: pieceKind === "Input" ? Theme.accent : Theme.text; font.pixelSize: 12 }
                                TapHandler { onTapped: root.selectPiece(index) }
                            }
                        }
                    }
                    Repeater {
                        model: root.branchInfo
                        delegate: Item {
                            required property var modelData; required property int index
                            readonly property real rowTop: 50 + index * (wrapPreview.mouthHeight + 34)
                            Rectangle {
                                x: 30; y: parent.rowTop + 9; width: branchName.implicitWidth + 18; height: 28; radius: 5
                                color: Qt.darker(root.blockColor, 1.25)
                                border.width: root.selectedPiece === modelData.index ? 2 : 1
                                border.color: root.selectedPiece === modelData.index ? Theme.accent : Theme.border
                                Text { id: branchName; anchors.centerIn: parent; text: modelData.name; color: Theme.text; font.pixelSize: 12; font.weight: Font.DemiBold }
                                TapHandler { onTapped: root.selectPiece(modelData.index) }
                            }
                            Text {
                                visible: index < root.branchInfo.length - 1
                                x: 28; y: parent.rowTop + wrapPreview.mouthHeight + 9
                                text: modelData.sep; color: Theme.textDim; font.pixelSize: 12
                            }
                        }
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true; spacing: 8
            Text { text: "Selected piece"; color: Theme.textDim; font.pixelSize: 12 }
            BwTextField { id: pieceEditor; Layout.fillWidth: true; placeholderText: "Piece name"; onTextEdited: root.commitPiece(); onEditingFinished: root.commitPiece() }
        }

        Grid {
            Layout.alignment: Qt.AlignHCenter; columns: 12; spacing: 7
            Repeater {
                model: root.colorPresets
                delegate: Rectangle {
                    required property string modelData
                    width: 30; height: 30; radius: 15; color: modelData
                    border.width: root.blockColor === modelData ? 3 : 1
                    border.color: root.blockColor === modelData ? Theme.text : Theme.border
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: root.blockColor = modelData }
                }
            }
            Rectangle {
                width:30;height:30;radius:15;color:Theme.panelRaised;border.width:root.colorPresets.indexOf(root.blockColor)<0?3:1;border.color:root.colorPresets.indexOf(root.blockColor)<0?Theme.text:Theme.border
                LucideIcon{anchors.centerIn:parent;width:15;height:15;name:"pipette";color:Theme.text}
                HoverHandler{cursorShape:Qt.PointingHandCursor}
                TapHandler{onTapped:customColor.open()}
            }
        }

        GridLayout {
            Layout.fillWidth: true; columns: 4; columnSpacing: 8
            Repeater {
                model: [
                    { title: "Add an input", subtitle: "number or text", sample: "123", kind: "Input", valueType: "Any" },
                    { title: "Add an input", subtitle: "boolean", sample: "◇", kind: "Input", valueType: "Bool" },
                    { title: "Add an input", subtitle: "branch", sample: "⌞", kind: "Branch", valueType: "Any" },
                    { title: "Add a label", subtitle: "", sample: "Abc", kind: "Label", valueType: "Any" }
                ]
                delegate: Button {
                    required property var modelData
                    Layout.fillWidth: true; implicitHeight: 58
                    onClicked: root.addPiece(modelData.kind, modelData.valueType)
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    contentItem: Row {
                        spacing: 8
                        Text { text: modelData.sample; color: modelData.kind === "Input" || modelData.kind === "Branch" ? Theme.accent : Theme.text; font.pixelSize: modelData.kind === "Branch" ? 26 : 13; font.weight: Font.DemiBold; width: 32; anchors.verticalCenter: parent.verticalCenter; horizontalAlignment: Text.AlignHCenter }
                        Column { anchors.verticalCenter: parent.verticalCenter
                            Text { text: modelData.title; color: Theme.text; font.pixelSize: 12; font.weight: Font.DemiBold }
                            Text { visible: modelData.subtitle.length > 0; text: modelData.subtitle; color: Theme.textDim; font.pixelSize: 10 }
                        }
                    }
                    background: Rectangle { radius: Theme.radius; color: parent.hovered ? "#3b3c40" : Theme.panelRaised; border.color: parent.hovered ? Theme.accent : Theme.border }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true; spacing: 8
            BwButton { Layout.fillWidth: true; text: root.returnsValue ? "Return Text or Number" : "Normal block"; primary: root.blockShape === (root.returnsValue ? "ReturnsValue" : "Normal"); onClicked: root.blockShape = root.returnsValue ? "ReturnsValue" : "Normal" }
            BwButton { Layout.fillWidth: true; text: root.returnsValue ? "Return a Boolean" : "Ending block"; primary: root.blockShape === (root.returnsValue ? "ReturnsBool" : "Ending"); onClicked: root.blockShape = root.returnsValue ? "ReturnsBool" : "Ending" }
        }

        BwCheckBox {
            text: "Returns a value"; checked: root.returnsValue
            onToggled: root.blockShape = checked ? "ReturnsValue" : "Normal"
        }
        Text { visible: root.validationError.length > 0; text: root.validationError; color: Theme.danger; font.pixelSize: 12 }
        Rectangle { Layout.fillWidth: true; height: 1; color: Theme.borderSoft }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            BwButton { text: "Cancel"; onClicked: root.reject() }
            BwButton { text: "OK"; primary: true; onClicked: root.submit() }
        }
    }
    ColorDialog{id:customColor;title:"Choose a block color";selectedColor:root.blockColor;onAccepted:root.blockColor=String(selectedColor)}
}
