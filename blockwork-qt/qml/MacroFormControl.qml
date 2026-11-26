import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Loader {
    id: root
    required property var form
    required property var node
    required property int nodeIndex
    readonly property string kind: node.kind
    readonly property var properties: form.properties(node)
    readonly property string caption: properties.label ?? node.label ?? ""
    readonly property var value: form.controlValue(node)
    readonly property var childrenModel: node.children || []
    readonly property string iconName: String(properties.icon ?? node.icon ?? "")
    readonly property string tone: String(properties.tone ?? node.tone ?? "")
    readonly property string variant: String(node.variant ?? "")
    readonly property string style: String(node.style ?? "")
    readonly property var options: kind === "choice" ? (properties.options ?? node.options ?? []) : []
    readonly property string rawData: String(properties.data ?? node.data ?? "")
    readonly property var items: kind === "list" ? listItems(rawData) : []
    readonly property var tableRows: kind === "table-input" ? parseTableRows(rawData) : []
    readonly property color toneColor: {
        switch (tone) {
        case "muted": return Theme.textDim;
        case "accent": return Theme.accent;
        case "success": return Theme.running;
        case "warning": return Theme.warning;
        case "danger": return Theme.danger;
        }
        return kind === "caption" ? Theme.textDim : Theme.text;
    }

    function array(raw) {
        try {
            const result = JSON.parse(raw);
            return Array.isArray(result) ? result : [];
        } catch (_) {
            return [];
        }
    }
    // A list's options: plain strings, or {label, value, subtitle, icon}.
    function listItems(raw) {
        return array(raw).map(entry => typeof entry === "string"
            ? {label: entry, value: entry, subtitle: "", icon: ""}
            : {label: String(entry.label ?? entry.value ?? ""), value: String(entry.value ?? entry.label ?? ""),
               subtitle: String(entry.subtitle ?? entry.detail ?? ""), icon: String(entry.icon ?? "")})
            .filter(item => item.label !== "" || item.value !== "");
    }
    function parseTableRows(raw) {
        return array(raw).filter(row => Array.isArray(row)).map(row => row.map(cell => String(cell ?? "")));
    }

    sourceComponent: {
        switch (kind) {
        case "row": case "column": case "grid": case "group": return layout;
        case "split": return split;
        case "scroll": return scrolling;
        case "tabs": return tabs;
        case "spacer": return spacer;
        case "button": return button;
        case "check": return check;
        case "note": case "heading": case "subheading": case "body": case "caption": return reportText;
        case "divider": return divider;
        case "icon": return standaloneIcon;
        default: return field;
        }
    }

    component Caption: Text {
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: root.toneColor
        font.pixelSize: 13
        Layout.fillWidth: true
    }

    Component {
        id: layout
        ColumnLayout {
            id: layoutColumn
            readonly property bool fillsHeight: WindowSizing.specs(String(root.properties.size ?? root.node.size ?? "")).h === "fill"
            spacing: 8
            RowLayout {
                visible: root.caption.length > 0 || root.iconName.length > 0
                Layout.fillWidth: true
                spacing: 6
                LucideIcon {
                    visible: root.iconName.length > 0
                    name: root.iconName
                    color: root.toneColor
                    width: 15; height: 15
                }
                Caption {
                    visible: text.length > 0
                    text: root.caption
                    font.bold: root.kind === "group"
                }
            }
            Rectangle {
                readonly property bool framed: root.style === "card" || root.style === "panel"
                Layout.fillWidth: true
                Layout.fillHeight: layoutColumn.fillsHeight
                Layout.preferredHeight: body.implicitHeight + (framed ? 20 : 0)
                color: framed ? (root.style === "panel" ? Theme.panelRaised : Theme.panel) : "transparent"
                border.color: root.style === "card" ? Theme.border : "transparent"
                radius: framed ? Theme.radius : 0
                MacroFormLayout {
                    id: body
                    anchors.fill: parent
                    anchors.margins: parent.framed ? 10 : 0
                    form: root.form
                    availableHeight: root.height
                    nodes: root.childrenModel
                    columns: root.kind === "row" ? Math.max(1, nodes.filter(c => root.form.properties(c).visible !== false).length)
                        : root.kind === "grid" ? (root.node.columns || 2) : 1
                }
            }
        }
    }

    Component {
        id: split
        SplitView {
            id: panes
            implicitHeight: 260
            readonly property real weightTotal: {
                let total = 0;
                for (const child of root.childrenModel) {
                    const weight = WindowSizing.weight(WindowSizing.specs(String(root.form.properties(child).size ?? child.size ?? "")).w);
                    if (weight > 0) total += weight;
                }
                return total;
            }
            handle: Rectangle {
                implicitWidth: 6
                color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : "transparent"
                Rectangle {
                    anchors.centerIn: parent
                    width: 2
                    height: parent.height
                    color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : Theme.border
                }
            }
            Repeater {
                model: root.childrenModel
                delegate: Loader {
                    id: pane
                    required property var modelData
                    required property int index
                    readonly property var axes: WindowSizing.specs(String(root.form.properties(modelData).size ?? modelData.size ?? ""))
                    readonly property real percent: WindowSizing.percent(axes.w)
                    readonly property real pixels: WindowSizing.pixels(axes.w)
                    readonly property real weight: WindowSizing.weight(axes.w)
                    SplitView.fillWidth: axes.w === "fill" || percent >= 0 || weight > 0 || axes.w === ""
                    SplitView.minimumWidth: 80
                    SplitView.preferredWidth: pixels >= 0 ? pixels
                        : percent >= 0 ? Math.max(80, panes.width * percent / 100)
                        : weight > 0 && panes.weightTotal > 0 ? Math.max(80, panes.width * weight / panes.weightTotal)
                        : implicitWidth
                    Component.onCompleted: setSource(Qt.resolvedUrl("MacroFormControl.qml"), {
                        form: root.form, node: modelData, nodeIndex: index
                    })
                    Rectangle {
                        visible: !!root.form.request.preview && root.form.selectionPath.slice(-1)[0] === modelData.id
                        anchors.fill: parent
                        anchors.margins: -3
                        color: "transparent"
                        radius: Theme.radius
                        border.color: Theme.accent
                        z: 2
                    }
                }
            }
        }
    }

    Component {
        id: scrolling
        ScrollView {
            id: viewport
            implicitHeight: Math.min(280, body.implicitHeight)
            contentWidth: availableWidth
            clip: true
            MacroFormLayout {
                id: body
                width: viewport.availableWidth
                form: root.form
                nodes: root.childrenModel
                availableHeight: viewport.availableHeight
            }
        }
    }

    Component {
        id: tabs
        ColumnLayout {
            id: tabLayout
            property var pages: []
            function syncPages() {
                const next = root.childrenModel.filter(c => root.form.properties(c).visible !== false);
                if (next.length === pages.length && next.every((page, index) => page === pages[index]))
                    return;
                const current = pages[tabBar.currentIndex];
                const nextIndex = next.indexOf(current);
                const index = nextIndex >= 0 ? nextIndex : Math.min(Math.max(0, tabBar.currentIndex), next.length - 1);
                pages = next;
                tabBar.setCurrentIndex(index);
            }
            function revealSelection() {
                if (!root.form.request.preview) return;
                const index = pages.findIndex(page => root.form.selectionPath.includes(page.id));
                if (index >= 0) tabBar.setCurrentIndex(index);
            }
            Component.onCompleted: {
                syncPages();
                revealSelection();
            }
            Connections {
                target: root.form
                ignoreUnknownSignals: true
                function onFormUpdatesChanged() { tabLayout.syncPages(); }
                function onSelectionPathChanged() { tabLayout.revealSelection(); }
            }
            spacing: 10
            TabBar {
                id: tabBar
                Layout.fillWidth: true
                padding: 3
                spacing: 3
                background: Rectangle {
                    radius: Theme.radius
                    color: Theme.panelRaised
                }
                Repeater {
                    model: pages
                    delegate: TabButton {
                        id: tab
                        required property var modelData
                        readonly property var pageProperties: root.form.properties(modelData)
                        readonly property string pageIcon: String(pageProperties.icon ?? modelData.icon ?? "")
                        objectName: "promptFormTab_" + modelData.id
                        text: pageProperties.label ?? modelData.label ?? ""
                        enabled: pageProperties.enabled ?? true
                        implicitHeight: 34
                        HoverHandler {
                            cursorShape: tab.enabled && !tab.checked ? Qt.PointingHandCursor : Qt.ArrowCursor
                        }
                        background: Rectangle {
                            radius: Theme.radius
                            color: tab.checked ? Theme.panel : tab.enabled && tab.hovered ? Theme.hover : "transparent"
                            border.color: tab.visualFocus ? Theme.accent : tab.checked ? Theme.border : "transparent"
                        }
                        contentItem: Row {
                            spacing: 6
                            anchors.centerIn: parent
                            LucideIcon {
                                visible: tab.pageIcon.length > 0
                                name: tab.pageIcon
                                width: 14; height: 14
                                anchors.verticalCenter: parent.verticalCenter
                                color: tab.enabled && (tab.checked || tab.hovered) ? Theme.text : Theme.textDim
                            }
                            Text {
                                text: tab.text
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                verticalAlignment: Text.AlignVCenter
                                color: tab.enabled && (tab.checked || tab.hovered) ? Theme.text : Theme.textDim
                                font.pixelSize: 13
                                font.weight: tab.checked ? Font.DemiBold : Font.Normal
                            }
                        }
                    }
                }
            }
            MacroFormLayout {
                Layout.fillWidth: true
                form: root.form
                availableHeight: Math.max(0, tabLayout.height - tabBar.height - tabLayout.spacing)
                nodes: {
                    const page = pages[tabBar.currentIndex];
                    if (!page) return [];
                    return page.kind === "group" ? (page.children || []) : [page];
                }
                enabled: tabBar.currentIndex >= 0 && tabBar.currentIndex < pages.length
                    && (root.form.properties(pages[tabBar.currentIndex]).enabled ?? true)
            }
        }
    }

    Component { id: spacer; Item { implicitHeight: 12 } }

    Component {
        id: reportText
        RowLayout {
            spacing: 8
            LucideIcon {
                visible: root.iconName.length > 0
                name: root.iconName
                color: root.toneColor
                width: root.kind === "heading" ? 20 : 15
                height: width
                Layout.alignment: Qt.AlignVCenter
            }
            Caption {
                objectName: "promptFormLabel_" + root.nodeIndex
                text: root.caption
                font.bold: root.kind === "heading"
                font.pixelSize: root.kind === "heading" ? 22
                    : root.kind === "subheading" ? 16
                    : root.kind === "caption" ? 12 : 14
                font.weight: root.kind === "subheading" ? Font.DemiBold : Font.Normal
            }
        }
    }

    Component {
        id: divider
        Rectangle {
            implicitHeight: 1
            color: Theme.borderSoft
        }
    }

    Component {
        id: standaloneIcon
        Item {
            implicitWidth: 20
            implicitHeight: 20
            LucideIcon {
                anchors.fill: parent
                name: String(root.value ?? "") || "circle"
                color: root.toneColor
            }
        }
    }

    Component {
        id: button
        StitchButton {
            objectName: "promptFormButton_" + root.node.label
            text: root.caption
            primary: root.variant === "primary" || (root.variant === "" && !!root.node.close)
            danger: root.variant === "danger"
            flat: root.variant === "quiet"
            iconName: root.iconName
            onClicked: root.form.click(root.node)
        }
    }

    Component {
        id: check
        StitchCheckBox {
            id: checkbox
            Binding { target: checkbox.contentItem; property: "textFormat"; value: Text.PlainText }
            objectName: "promptFormCheck_" + root.nodeIndex
            text: root.caption
            checked: root.value === true
            onToggled: root.form.setFormValue(root.node, checked)
        }
    }

    Component {
        id: field
        ColumnLayout {
            spacing: 5
            RowLayout {
                visible: root.kind !== "progress" && (root.caption.length > 0 || root.iconName.length > 0)
                Layout.fillWidth: true
                spacing: 6
                LucideIcon {
                    visible: root.iconName.length > 0
                    name: root.iconName
                    color: root.toneColor
                    width: 14; height: 14
                }
                Caption { text: root.caption }
            }
            Loader {
                Layout.fillWidth: true
                // Only the tall controls (and anything asking for h:fill) use spare height.
                Layout.fillHeight: root.kind === "multiline" || root.kind === "list"
                    || root.kind === "table" || root.kind === "table-input"
                    || WindowSizing.specs(String(root.properties.size ?? root.node.size ?? "")).h === "fill"
                    || WindowSizing.weight(WindowSizing.specs(String(root.properties.size ?? root.node.size ?? "")).h) > 0
                sourceComponent: {
                    switch (root.kind) {
                    case "text": return lineEdit;
                    case "choice": return choice;
                    case "slider": return slider;
                    case "progress": return progressIndicator;
                    case "table": return table;
                    case "chart": return chart;
                    case "list": return list;
                    case "table-input": return selectableTable;
                    case "multiline": return multiline;
                    case "file": case "folder": return pathPicker;
                    case "image": return preview;
                    default: return null;
                    }
                }
            }
        }
    }

    Component {
        id: lineEdit
        StitchTextField {
            objectName: "promptFormText_" + root.nodeIndex
            Accessible.name: root.caption
            text: String(root.value ?? "")
            onTextEdited: root.form.setFormValue(root.node, text)
        }
    }

    Component {
        id: choice
        StitchComboBox {
            objectName: "promptFormChoice_" + root.nodeIndex
            Accessible.name: root.caption
            model: root.options
            currentIndex: Math.max(0, root.options.indexOf(String(root.value ?? "")))
            onActivated: index => root.form.setFormValue(root.node, model[index])
            Component.onCompleted: {
                if (!root.form.request.live && count > 0)
                    root.form.setFormValue(root.node, model[currentIndex]);
            }
        }
    }

    Component {
        id: slider
        Slider {
            objectName: "promptFormSlider_" + root.nodeIndex
            Accessible.name: root.caption
            from: 0; to: 100; stepSize: 1
            value: Number(root.value) || 0
            onMoved: root.form.setFormValue(root.node, value)
        }
    }

    Component {
        id: progressIndicator
        ColumnLayout {
            spacing: 6
            RowLayout {
                Layout.fillWidth: true
                Caption { text: root.caption }
                Caption {
                    Layout.fillWidth: false
                    text: qsTr("%1%").arg(meter.value.toLocaleString(Qt.locale(), "f", 0))
                    color: Theme.textDim
                }
            }
            ProgressBar {
                id: meter
                Layout.fillWidth: true
                Accessible.name: root.caption
                from: 0; to: 100
                value: Math.max(0, Math.min(100, Number(root.value) || 0))
                background: Rectangle {
                    implicitWidth: 116
                    implicitHeight: 8
                    radius: height / 2
                    color: Theme.panelRaised
                }
                contentItem: Item {
                    implicitHeight: 8
                    Rectangle {
                        width: meter.position * parent.width
                        height: parent.height
                        x: meter.mirrored ? parent.width - width : 0
                        radius: Math.min(width, height) / 2
                        color: Theme.accent
                    }
                }
            }
        }
    }

    Component {
        id: table
        MacroFormTable {
            objectName: "promptFormTable_" + root.nodeIndex
            rows: root.parseTableRows(root.value)
            Accessible.name: root.caption
        }
    }

    Component {
        id: selectableTable
        MacroFormTable {
            objectName: "promptFormTable_" + root.nodeIndex
            rows: root.tableRows
            selectable: true
            sortable: true
            Accessible.name: root.caption
            selectedValue: String(root.value ?? "")
            onSelected: row => {
                const picked = {};
                const header = root.tableRows.length ? root.tableRows[0] : [];
                header.forEach((key, index) => picked[key] = row[index] ?? "");
                root.form.setFormValue(root.node, JSON.stringify(picked));
            }
        }
    }

    Component {
        id: list
        ListView {
            id: listView
            implicitHeight: Math.min(count * 34, 280)
            clip: true
            model: root.items
            Accessible.name: root.caption
            ScrollBar.vertical: ScrollBar {}
            delegate: ItemDelegate {
                id: entry
                required property var modelData
                width: ListView.view.width
                height: modelData.subtitle.length ? 48 : 34
                highlighted: modelData.value === String(root.value ?? "")
                text: modelData.label
                contentItem: RowLayout {
                    spacing: 8
                    LucideIcon {
                        visible: entry.modelData.icon.length > 0
                        name: entry.modelData.icon
                        width: 16; height: 16
                        color: Theme.textDim
                        Layout.alignment: Qt.AlignVCenter
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            Layout.fillWidth: true
                            text: entry.modelData.label
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: Theme.text
                            font.pixelSize: 13
                        }
                        Text {
                            visible: entry.modelData.subtitle.length > 0
                            Layout.fillWidth: true
                            text: entry.modelData.subtitle
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: Theme.textDim
                            font.pixelSize: 11
                        }
                    }
                }
                onClicked: root.form.setFormValue(root.node, entry.modelData.value)
            }
        }
    }

    Component {
        id: multiline
        ScrollView {
            implicitHeight: 110
            TextArea {
                Accessible.name: root.caption
                textFormat: TextEdit.PlainText
                wrapMode: TextEdit.Wrap
                text: String(root.value ?? "")
                onTextChanged: {
                    if (activeFocus && text !== String(root.value ?? ""))
                        root.form.setFormValue(root.node, text);
                }
            }
        }
    }

    Component {
        id: pathPicker
        RowLayout {
            StitchTextField {
                Layout.fillWidth: true
                Accessible.name: root.caption
                text: String(root.value ?? "")
                onTextEdited: root.form.setFormValue(root.node, text)
            }
            StitchButton {
                text: qsTr("Browse…")
                enabled: !root.form.request.preview
                onClicked: {
                    if (root.kind === "folder") folder.open();
                    else file.open();
                }
            }
            FileDialog {
                id: file
                title: root.caption
                onAccepted: root.form.setFormValue(root.node, root.form.localPath(selectedFile))
            }
            FolderDialog {
                id: folder
                title: root.caption
                onAccepted: root.form.setFormValue(root.node, root.form.localPath(selectedFolder))
            }
        }
    }

    Component {
        id: preview
        Item {
            implicitHeight: image.source.toString() ? 150 : 0
            Image {
                id: image
                anchors.fill: parent
                fillMode: Image.PreserveAspectFit
                sourceSize.width: 800
                sourceSize.height: 600
                Accessible.name: root.caption
                source: /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/=\r\n]+$/.test(String(root.value)) ? root.value : ""
            }
        }
    }

    Component {
        id: chart
        Item {
            id: bars
            readonly property var numbers: root.array(root.value).filter(n => Number.isFinite(n)).slice(0, 120)
            readonly property real maximum: numbers.reduce((acc, n) => Math.max(acc, n), 0)
            readonly property real minimum: numbers.reduce((acc, n) => Math.min(acc, n), 0)
            implicitHeight: numbers.length ? 110 : 0
            RowLayout {
                anchors.fill: parent
                spacing: 3
                Repeater {
                    model: bars.numbers
                    delegate: Item {
                        required property real modelData
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Accessible.name: String(modelData)
                        Rectangle {
                            readonly property real span: bars.maximum - bars.minimum || 1
                            width: parent.width
                            y: parent.height * (bars.maximum - Math.max(0, modelData)) / span
                            height: parent.height * Math.abs(modelData) / span
                            color: Theme.accent
                        }
                    }
                }
            }
        }
    }
}
