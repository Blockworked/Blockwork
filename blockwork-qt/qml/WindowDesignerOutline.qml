import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

ColumnLayout {
    id: root
    required property var designer
    objectName: "windowDesignerOutline"
    spacing: 8
    property var collapsed: Object.create(null)
    readonly property var entries: designer.draft
        ? [{block: designer.draft, path: []}].concat(designer.outline(designer.draft.body, [], collapsed))
        : []
    readonly property int selectedIndex: entries.findIndex(row => (row.path.length ? row.block.id : "") === designer.selectedId)
    readonly property int siblingIndex: designer.selected ? designer.selected.path.slice(-1)[0] : -1
    readonly property string addTarget: !designer.selected ? qsTr("Inside Window")
        : designer.isContainer(designer.block) ? qsTr("Inside %1").arg(designer.componentName(designer.block))
        : qsTr("After %1").arg(designer.componentName(designer.block))

    function toggle(row) {
        const id = row.block.id;
        const next = Object.assign(Object.create(null), collapsed);
        if (next[id]) delete next[id]; else next[id] = true;
        if (next[id] && designer.selected && designer.selected.path.length > row.path.length
            && row.path.every((index, depth) => index === designer.selected.path[depth]))
            designer.selectedId = id;
        collapsed = next;
    }
    function revealSelection() {
        if (designer.selected) {
            const ancestors = designer.selected.path.slice(0, -1);
            const next = Object.assign(Object.create(null), collapsed);
            for (let i = 1; i <= ancestors.length; ++i)
                delete next[designer.at(designer.draft, ancestors.slice(0, i)).id];
            if (Object.keys(next).length !== Object.keys(collapsed).length) collapsed = next;
        }
        Qt.callLater(positionSelection);
    }
    function positionSelection() {
        if (selectedIndex >= 0) tree.positionViewAtIndex(selectedIndex, ListView.Contain);
    }
    function select(index) {
        if (index < 0 || index >= entries.length) return;
        const row = entries[index];
        designer.selectedId = row.path.length ? row.block.id : "";
    }
    Connections {
        target: root.designer
        function onOpened() {
            root.collapsed = Object.create(null);
            tree.forceActiveFocus();
        }
        function onSelectedIdChanged() { root.revealSelection(); }
    }

    component MenuLabel: Text {
        color: Theme.textDim
        textFormat: Text.PlainText
        font.pixelSize: 12
        elide: Text.ElideRight
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 4
        Text {
            Layout.fillWidth: true
            text: qsTr("Components")
            color: Theme.text
            font.pixelSize: 14
            font.bold: true
        }
        StitchButton {
            objectName: "designerMoveTo"
            flat: true
            iconName: "folder-tree"
            implicitWidth: 28
            implicitHeight: 28
            enabled: root.designer.moveChoices.length > 0
            ToolTip.text: qsTr("Move to another container")
            ToolTip.visible: hovered
            onClicked: picker.pick(true)
        }
        StitchButton {
            id: more
            objectName: "designerComponentActions"
            flat: true
            iconName: "ellipsis"
            implicitWidth: 28
            implicitHeight: 28
            enabled: !!root.designer.selected
            ToolTip.text: qsTr("Component actions")
            ToolTip.visible: hovered
            onClicked: actions.popup(more, more.width - actions.width, more.height)
        }
    }
    StitchButton {
        id: add
        objectName: "designerAddComponent"
        Layout.fillWidth: true
        implicitHeight: 32
        iconName: "plus"
        text: root.designer.addParent?.layout === "tabs" ? qsTr("Add tab") : qsTr("Add component")
        onClicked: picker.pick(false)
    }
    MenuLabel {
        Layout.fillWidth: true
        text: root.addTarget
        id: targetLabel
        ToolTip {
            visible: targetLabel.truncated && targetHover.hovered
            contentItem: MenuLabel { text: targetLabel.text }
        }
        HoverHandler { id: targetHover }
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        color: Theme.field
        border.color: Theme.borderSoft
        radius: Theme.radius
        ListView {
            id: tree
            objectName: "windowDesignerTree"
            anchors.fill: parent
            anchors.margins: 4
            model: root.entries
            clip: true
            keyNavigationEnabled: false
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { id: treeScroll }
            Keys.onPressed: event => {
                const row = root.entries[root.selectedIndex];
                if (!row) return;
                switch (event.key) {
                case Qt.Key_Up:
                case Qt.Key_Down: {
                    const direction = event.key === Qt.Key_Up ? -1 : 1;
                    if (event.modifiers & Qt.ControlModifier) root.designer.reposition(direction);
                    else root.select(root.selectedIndex + direction);
                    break;
                }
                case Qt.Key_Left:
                    if (row.path.length && root.designer.isContainer(row.block) && !root.collapsed[row.block.id])
                        root.toggle(row);
                    else if (row.path.length)
                        root.designer.selectedId = row.path.length > 1
                            ? root.designer.at(root.designer.draft, row.path.slice(0, -1)).id : "";
                    break;
                case Qt.Key_Right:
                    if (!row.path.length) root.select(1);
                    else if (root.designer.isContainer(row.block)) {
                        if (root.collapsed[row.block.id]) root.toggle(row);
                        else if (row.block.body.length) root.select(root.selectedIndex + 1);
                    }
                    break;
                case Qt.Key_Delete: root.designer.reposition(0); break;
                case Qt.Key_Menu:
                    if (!root.designer.selected) return;
                    actions.popup();
                    break;
                default: return;
                }
                event.accepted = true;
                Qt.callLater(root.positionSelection);
            }
            delegate: ItemDelegate {
                id: entry
                required property var modelData
                readonly property var componentBlock: modelData.block
                readonly property bool windowRow: modelData.path.length === 0
                readonly property bool container: !windowRow && root.designer.isContainer(componentBlock)
                readonly property string componentType: windowRow ? "" : root.designer.partName(componentBlock)
                readonly property string componentId: windowRow ? "" : componentBlock.id
                objectName: windowRow ? "designerWindowRow" : "designerComponent_" + componentId
                width: tree.width - (treeScroll.visible && treeScroll.size < 1 ? treeScroll.width : 0)
                height: 29
                leftPadding: 4 + 13 * Math.min(7, Math.max(0, modelData.path.length - 1))
                rightPadding: 6
                topPadding: 0
                bottomPadding: 0
                focusPolicy: Qt.NoFocus
                highlighted: root.designer.selectedId === componentId
                text: windowRow ? qsTr("Window") : root.designer.componentName(componentBlock)
                Accessible.name: componentType && componentType !== text ? qsTr("%1, %2").arg(text).arg(componentType) : text
                contentItem: RowLayout {
                    spacing: 5
                    Item {
                        Layout.preferredWidth: 18
                        Layout.fillHeight: true
                        StitchButton {
                            anchors.fill: parent
                            visible: entry.container
                            objectName: "designerDisclosure_" + entry.componentId
                            focusPolicy: Qt.NoFocus
                            flat: true
                            iconName: root.collapsed[entry.componentId] ? "chevron-right" : "chevron-down"
                            ToolTip.text: root.collapsed[entry.componentId] ? qsTr("Expand") : qsTr("Collapse")
                            onClicked: {
                                tree.forceActiveFocus();
                                root.toggle(entry.modelData);
                            }
                        }
                    }
                    LucideIcon {
                        Layout.preferredWidth: 14
                        Layout.preferredHeight: 14
                        name: entry.windowRow ? "app-window" : root.designer.partFor(entry.componentBlock)?.icon ?? "square"
                        color: entry.highlighted ? Theme.accent : Theme.textDim
                    }
                    Text {
                        id: caption
                        Layout.fillWidth: true
                        text: entry.text
                        textFormat: Text.PlainText
                        color: Theme.text
                        font.pixelSize: 13
                        font.weight: entry.highlighted || entry.windowRow ? Font.DemiBold : Font.Normal
                        elide: Text.ElideRight
                    }
                    LucideIcon {
                        visible: entry.componentBlock.label?.kind === "Var" || entry.componentBlock.text?.kind === "Var"
                            || entry.componentBlock.data?.kind === "Var" || !!entry.componentBlock.name
                        Layout.preferredWidth: 12
                        Layout.preferredHeight: 12
                        name: "variable"
                        color: Theme.textDim
                    }
                }
                ToolTip {
                    visible: entry.hovered && !entry.windowRow && (caption.truncated || entry.componentType !== entry.text)
                    delay: 500
                    contentItem: MenuLabel { text: entry.Accessible.name }
                }
                background: Rectangle {
                    radius: Theme.radius
                    color: entry.highlighted ? Theme.selection : entry.hovered ? Theme.hover : "transparent"
                    border.color: entry.highlighted && tree.activeFocus ? Theme.accent : "transparent"
                }
                onClicked: {
                    tree.forceActiveFocus();
                    root.designer.selectedId = componentId;
                }
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    enabled: !entry.windowRow
                    onTapped: {
                        root.designer.selectedId = entry.componentId;
                        actions.popup();
                    }
                }
            }
        }
    }

    StitchMenu {
        id: actions
        objectName: "designerActionsMenu"
        property bool openMovePicker: false
        onClosed: {
            if (!openMovePicker) return;
            openMovePicker = false;
            picker.pick(true);
        }
        StitchMenuItem {
            iconName: "chevron-up"
            text: qsTr("Move up")
            enabled: root.siblingIndex > 0
            onTriggered: { root.designer.reposition(-1); root.revealSelection(); }
        }
        StitchMenuItem {
            iconName: "chevron-down"
            text: qsTr("Move down")
            enabled: root.siblingIndex >= 0 && root.siblingIndex + 1 < root.designer.selected.siblings
            onTriggered: { root.designer.reposition(1); root.revealSelection(); }
        }
        MenuSeparator {}
        StitchMenuItem {
            iconName: "trash"
            text: qsTr("Remove")
            danger: true
            enabled: !!root.designer.selected
            onTriggered: root.designer.reposition(0)
        }
    }

    Popup {
        id: picker
        objectName: "designerComponentPicker"
        parent: root
        width: Math.min(310, root.designer.width - 48)
        height: Math.max(130, Math.min(370, root.height - y))
        y: add.y + add.height + 4
        padding: 10
        focus: true
        modal: false
        property bool moving: false
        readonly property var choices: (moving ? root.designer.moveChoices : root.designer.addChoices)
            .filter(item => item.name.toLocaleLowerCase().includes(search.text.trim().toLocaleLowerCase()))

        function pick(move) {
            moving = move;
            search.text = "";
            choicesList.currentIndex = 0;
            open();
        }
        function activate(index) {
            if (index < 0 || index >= choices.length) return;
            const item = choices[index];
            if (moving) root.designer.moveInto(item.id); else root.designer.add(item);
            close();
            root.revealSelection();
            tree.forceActiveFocus();
        }
        onOpened: search.forceActiveFocus()
        background: Rectangle { color: Theme.panelRaised; border.color: Theme.border; radius: Theme.radius }
        contentItem: ColumnLayout {
            spacing: 6
            StitchTextField {
                id: search
                objectName: "designerComponentSearch"
                Layout.fillWidth: true
                placeholderText: picker.moving ? qsTr("Find a destination…") : qsTr("Find a component…")
                onTextEdited: choicesList.currentIndex = 0
                Keys.onUpPressed: choicesList.decrementCurrentIndex()
                Keys.onDownPressed: choicesList.incrementCurrentIndex()
                onAccepted: picker.activate(choicesList.currentIndex)
            }
            MenuLabel {
                Layout.fillWidth: true
                text: picker.moving ? qsTr("Move %1 into").arg(root.designer.block ? root.designer.componentName(root.designer.block) : "") : root.addTarget
            }
            ListView {
                id: choicesList
                Layout.fillWidth: true
                Layout.fillHeight: true
                model: picker.choices
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                delegate: ColumnLayout {
                    id: choice
                    required property var modelData
                    required property int index
                    width: choicesList.width
                    spacing: 2
                    MenuLabel {
                        Layout.fillWidth: true
                        Layout.leftMargin: 6
                        visible: !!choice.modelData.group && (choice.index === 0 || picker.choices[choice.index - 1]?.group !== choice.modelData.group)
                        text: choice.modelData.group ?? ""
                    }
                    ItemDelegate {
                        id: option
                        Layout.fillWidth: true
                        implicitHeight: 30
                        leftPadding: 7 + Math.min(choice.modelData.depth ?? 0, 5) * 12
                        topPadding: 0
                        bottomPadding: 0
                        highlighted: choice.index === choicesList.currentIndex
                        text: choice.modelData.name
                        contentItem: RowLayout {
                            spacing: 7
                            LucideIcon {
                                name: choice.modelData.icon
                                color: Theme.textDim
                                Layout.preferredWidth: 14
                                Layout.preferredHeight: 14
                            }
                            Text {
                                Layout.fillWidth: true
                                text: option.text
                                textFormat: Text.PlainText
                                color: Theme.text
                                font.pixelSize: 13
                                elide: Text.ElideRight
                            }
                        }
                        background: Rectangle {
                            radius: Theme.radius
                            color: option.highlighted || option.hovered ? Theme.selection : "transparent"
                        }
                        onClicked: picker.activate(choice.index)
                    }
                }
                MenuLabel {
                    anchors.centerIn: parent
                    visible: !choicesList.count
                    text: qsTr("No matches")
                }
            }
        }
    }
}
