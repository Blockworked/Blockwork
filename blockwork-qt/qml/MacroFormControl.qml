import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

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
    readonly property var options: kind === "list" && properties.data !== undefined
        ? array(properties.data).filter(item => typeof item === "string")
        : properties.options ?? node.options ?? []

    function array(raw) {
        try {
            const result = JSON.parse(raw);
            return Array.isArray(result) ? result : [];
        } catch (_) {
            return [];
        }
    }

    sourceComponent: {
        switch (kind) {
        case "row": case "column": case "grid": case "group": return layout;
        case "scroll": return scrolling;
        case "tabs": return tabs;
        case "spacer": return spacer;
        case "button": return button;
        case "check": return check;
        case "note": case "heading": return reportText;
        default: return field;
        }
    }

    component Caption: Text {
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: Theme.text
        font.pixelSize: 13
        Layout.fillWidth: true
    }

    Component {
        id: layout
        ColumnLayout {
            spacing: 8
            Caption {
                visible: text.length > 0
                text: root.caption
                font.bold: root.kind === "group"
            }
            MacroFormLayout {
                Layout.fillWidth: true
                form: root.form
                nodes: root.childrenModel
                columns: root.kind === "row" ? Math.max(1, nodes.filter(c => root.form.properties(c).visible !== false).length)
                    : root.kind === "grid" ? (root.node.columns || 2) : 1
            }
        }
    }

    Component {
        id: scrolling
        ScrollView {
            id: viewport
            implicitHeight: Math.min(240, body.implicitHeight)
            contentWidth: availableWidth
            clip: true
            MacroFormLayout {
                id: body
                width: viewport.availableWidth
                form: root.form
                nodes: root.childrenModel
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
                        objectName: "promptFormTab_" + modelData.id
                        text: root.form.properties(modelData).label ?? modelData.label ?? ""
                        enabled: root.form.properties(modelData).enabled ?? true
                        implicitHeight: 34
                        HoverHandler {
                            cursorShape: tab.enabled && !tab.checked ? Qt.PointingHandCursor : Qt.ArrowCursor
                        }
                        background: Rectangle {
                            radius: Theme.radius
                            color: tab.checked ? Theme.panel : tab.enabled && tab.hovered ? Theme.hover : "transparent"
                            border.color: tab.visualFocus ? Theme.accent : tab.checked ? Theme.border : "transparent"
                        }
                        contentItem: Text {
                            text: tab.text
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                            horizontalAlignment: Text.AlignHCenter
                            color: tab.enabled && (tab.checked || tab.hovered) ? Theme.text : Theme.textDim
                            font.pixelSize: 13
                            font.weight: tab.checked ? Font.DemiBold : Font.Normal
                        }
                    }
                }
            }
            MacroFormLayout {
                Layout.fillWidth: true
                form: root.form
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
        Caption {
            objectName: "promptFormLabel_" + root.nodeIndex
            text: root.caption
            font.bold: root.kind === "heading"
            font.pixelSize: root.kind === "heading" ? 22 : 14
        }
    }

    Component {
        id: button
        StitchButton {
            objectName: "promptFormButton_" + root.node.label
            text: root.caption
            primary: !!root.node.close
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
            Caption {
                visible: root.kind !== "progress" && text.length > 0
                text: root.caption
            }
            Loader {
                Layout.fillWidth: true
                sourceComponent: {
                    switch (root.kind) {
                    case "text": return lineEdit;
                    case "choice": return choice;
                    case "slider": return slider;
                    case "progress": return progressIndicator;
                    case "table": return table;
                    case "chart": return chart;
                    case "list": return list;
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
            rows: root.array(root.value).filter(Array.isArray)
            Accessible.name: root.caption
        }
    }

    Component {
        id: list
        ListView {
            implicitHeight: Math.min(count * 34, 210)
            clip: true
            currentIndex: root.options.indexOf(String(root.value ?? ""))
            model: root.options
            Accessible.name: root.caption
            ScrollBar.vertical: ScrollBar {}
            delegate: ItemDelegate {
                id: entry
                required property string modelData
                width: ListView.view.width
                height: 34
                highlighted: modelData === root.value
                text: modelData
                contentItem: Text {
                    text: entry.text
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                    color: Theme.text
                }
                onClicked: root.form.setFormValue(root.node, modelData)
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
