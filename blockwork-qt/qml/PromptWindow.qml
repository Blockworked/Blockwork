import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// One macro dialog (`blockwork --prompt`): ask, yes/no, choose, message,
// permission, pick a file or folder, a tooltip or a menu at the mouse, or a
// window a macro built from controls. Closing the window answers "cancel".
ApplicationWindow {
    id: root
    readonly property var request: { try { return JSON.parse(bridge.requestJson); } catch (e) { return ({}); } }
    readonly property string kind: request.kind || "message"
    readonly property bool picksPath: kind === "file" || kind === "folder"
    readonly property bool isTooltip: kind === "tooltip"
    readonly property bool isMenu: kind === "menu"
    readonly property bool isForm: kind === "window"
    readonly property var controls: request.controls || []
    // A window's values by variable name, sent with every click.
    property var formValues: {
        const out = {};
        for (const c of controls) if (c.name && c.kind !== "button" && c.kind !== "note") out[c.name] = c.value;
        return out;
    }
    function setFormValue(name, value) {
        const next = Object.assign({}, formValues);
        next[name] = value;
        formValues = next;
    }
    function click(control) {
        if (control.close) answer("ok", control.label);
        else bridge.sendValues("click", control.label, JSON.stringify(formValues));
    }
    // Small frameless boxes instead of a dialog.
    readonly property bool boxed: isTooltip || isMenu
    Binding { target: Theme; property: "mode"; value: blockworkAppearance }
    Binding { target: Motion; property: "durationScale"; value: root.request.animation_speed ?? 1 }
    width: isTooltip ? Math.min(480, tipText.implicitWidth + 24) : isMenu ? 260 : 440
    height: isTooltip ? tipText.implicitHeight + 16 : isMenu ? Math.min(400, menuList.contentHeight + 8)
        : isForm ? Math.min(640, form.implicitHeight + 40)
        : Math.min(560, content.implicitHeight + 40)
    minimumWidth: boxed ? 0 : 320
    visible: true
    title: kind === "permission" ? qsTr("Blockwork permission") : (request.title || "Blockwork")
    flags: isForm ? Qt.Window : isTooltip ? (Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus | Qt.WindowTransparentForInput)
        : isMenu ? (Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint)
        : (Qt.Dialog | Qt.WindowStaysOnTopHint)
    color: Theme.panel
    palette {
        window: Theme.panel; windowText: Theme.text; base: Theme.field; text: Theme.text
        button: Theme.panelRaised; buttonText: Theme.text; highlight: Theme.accent; highlightedText: "white"
        placeholderText: Theme.textDim; mid: Theme.border; dark: Theme.borderSoft; light: Theme.panelRaised
    }

    PromptBridge { id: bridge; objectName: "promptBridge" }

    property bool answered: false
    function answer(button, text) {
        if (answered) return;
        answered = true;
        if (isForm) bridge.sendValues(button, text || "", JSON.stringify(formValues));
        else bridge.finish(button, text || "");
    }
    onClosing: answer(kind === "message" || isTooltip ? "ok" : "cancel", "")
    // Where a box goes: by the mouse, or a corner of the screen it's on.
    function placeBox() {
        if (!boxed) return;
        const place = request.default || "Mouse";
        const area = screen ? Qt.rect(screen.virtualX, screen.virtualY, screen.width, screen.height) : Qt.rect(0, 0, 1920, 1080);
        const margin = 16;
        let px = request.x ?? area.x + margin, py = request.y ?? area.y + margin;
        if (place === "TopLeft") { px = area.x + margin; py = area.y + margin; }
        else if (place === "TopRight") { px = area.x + area.width - width - margin; py = area.y + margin; }
        else if (place === "BottomLeft") { px = area.x + margin; py = area.y + area.height - height - margin; }
        else if (place === "BottomRight") { px = area.x + area.width - width - margin; py = area.y + area.height - height - margin; }
        else if (place === "Center") { px = area.x + (area.width - width) / 2; py = area.y + (area.height - height) / 2; }
        x = Math.round(Math.max(area.x, Math.min(px, area.x + area.width - width)));
        y = Math.round(Math.max(area.y, Math.min(py, area.y + area.height - height)));
    }
    onWidthChanged: placeBox()
    onHeightChanged: placeBox()
    // A menu closes once you click somewhere else.
    property bool wasActive: false
    onActiveChanged: {
        if (active) wasActive = true;
        else if (isMenu && wasActive) answer("cancel", "");
    }
    Timer {
        interval: Math.max(0, root.request.duration_ms ?? 3000)
        running: root.isTooltip && !root.answered
        onTriggered: root.answer("ok", "")
    }
    function focusInput() {
        if (kind === "text") field.forceActiveFocus();
        else if (picksPath) pathField.forceActiveFocus();
        else if (kind === "choice") choices.forceActiveFocus();
        else if (isMenu) menuList.forceActiveFocus();
    }
    // Each new question puts the keyboard back in its input.
    onRequestChanged: Qt.callLater(focusInput)
    Component.onCompleted: {
        placeBox();
        if (isTooltip) return;
        raise(); requestActivate(); focusInput();
        // Straight to the system's picker; this window stays for typing a path.
        if (picksPath) Qt.callLater(browse);
    }
    function browse() {
        if (kind === "folder") folderDialog.open(); else fileDialog.open();
    }
    // A picked file:// URL as a plain path.
    function localPath(url) {
        const text = decodeURIComponent(url.toString());
        if (!text.startsWith("file://")) return text;
        const path = text.substring(7);
        return /^\/[A-Za-z]:/.test(path) ? path.substring(1) : path;
    }
    FileDialog {
        id: fileDialog
        title: root.message || qsTr("Choose a file")
        fileMode: FileDialog.OpenFile
        onAccepted: root.answer("ok", root.localPath(selectedFile))
    }
    FolderDialog {
        id: folderDialog
        title: root.message || qsTr("Choose a folder")
        onAccepted: root.answer("ok", root.localPath(selectedFolder))
    }
    // The daemon starts dialogs in the background, where some systems
    // (Windows) won't let them take focus by asking; keep pulling until it's ours.
    Timer {
        property int tries: 0
        interval: 100
        repeat: true
        running: !root.active && !root.isTooltip && tries < 20
        onTriggered: { tries++; bridge.bringToFront(); }
    }
    // A daemon exit must not leave an orphaned question on the desktop.
    Timer {
        interval: 250; repeat: true; running: !root.answered
        onTriggered: { if (!bridge.parentAlive()) root.answer("cancel", ""); }
    }
    // Permission questions are worded here so they follow the UI language.
    readonly property string message: {
        if (kind !== "permission" || !request.ability) return request.message || "";
        const name = request.subject || "";
        const phrases = {
            commands: qsTr("\"%1\" wants to run commands and open or close apps."),
            network: qsTr("\"%1\" wants to go online."),
            browser: qsTr("\"%1\" wants to control your web browser."),
            files_write: qsTr("\"%1\" wants to write to files."),
            secrets: qsTr("\"%1\" wants to read your saved secrets."),
            screen: qsTr("\"%1\" wants to read the screen.")
        };
        return phrases[request.ability] ? phrases[request.ability].arg(name) : (request.message || "");
    }

    // [label, answer, primary] per kind, left to right.
    readonly property var buttons: ({
        text: [[qsTr("Cancel"), "cancel", false], [qsTr("OK"), "ok", true]],
        confirm: [[qsTr("No"), "no", false], [qsTr("Yes"), "yes", true]],
        choice: [[qsTr("Cancel"), "cancel", false], [qsTr("OK"), "ok", true]],
        file: [[qsTr("Cancel"), "cancel", false], [qsTr("OK"), "ok", true]],
        folder: [[qsTr("Cancel"), "cancel", false], [qsTr("OK"), "ok", true]],
        message: [[qsTr("OK"), "ok", true]],
        permission: [[qsTr("Deny"), "deny", false], [qsTr("Allow this time"), "once", false], [qsTr("Always allow"), "always", true]]
    })[kind] || [[qsTr("OK"), "ok", true]]
    function accept() {
        if (isForm) return;
        if (kind === "choice") { if (choices.currentIndex >= 0) answer("ok", request.options[choices.currentIndex]); }
        else if (isMenu) { if (menuList.currentIndex >= 0) answer("ok", request.options[menuList.currentIndex]); }
        else if (kind === "text") answer("ok", field.text);
        else if (picksPath) { if (pathField.text.trim() !== "") answer("ok", pathField.text.trim()); }
        else answer(buttons[buttons.length - 1][1], "");
    }
    Shortcut { sequences: ["Return", "Enter"]; onActivated: root.accept() }
    Shortcut { sequence: "Escape"; onActivated: root.close() }

    Rectangle {
        visible: root.boxed
        anchors.fill: parent
        color: Theme.panelRaised
        border.color: Theme.border
        radius: 6
        Text {
            id: tipText
            objectName: "promptTooltip"
            visible: root.isTooltip
            x: 12; y: 8
            width: Math.min(implicitWidth, 456)
            text: root.isTooltip ? root.message : ""
            wrapMode: Text.Wrap; color: Theme.text; font.pixelSize: 13
        }
        StitchListView {
            id: menuList
            objectName: "promptMenu"
            Accessible.role: Accessible.PopupMenu
            visible: root.isMenu
            anchors.fill: parent; anchors.margins: 4
            clip: true; currentIndex: 0
            model: root.isMenu ? (root.request.options || []) : []
            delegate: Rectangle {
                required property string modelData
                required property int index
                objectName: "promptMenuItem_" + index
                Accessible.role: Accessible.MenuItem
                Accessible.name: modelData
                Accessible.onPressAction: root.answer("ok", modelData)
                width: menuList.width; height: 30; radius: 4
                color: ListView.isCurrentItem ? Theme.selection : menuHover.hovered ? Theme.panel : "transparent"
                HoverHandler { id: menuHover; onHoveredChanged: if (hovered) menuList.currentIndex = index }
                Text { anchors.verticalCenter: parent.verticalCenter; x: 10; width: parent.width - 20; text: modelData; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 13 }
                TapHandler { onTapped: root.answer("ok", modelData) }
            }
            Keys.onUpPressed: decrementCurrentIndex()
            Keys.onDownPressed: incrementCurrentIndex()
        }
    }

    // A window a macro built: its controls in order, buttons along the bottom.
    ColumnLayout {
        id: form
        objectName: "promptForm"
        visible: root.isForm
        anchors.fill: parent; anchors.margins: 20
        spacing: 12
        Repeater {
            model: root.isForm ? root.controls.filter(c => c.kind !== "button") : []
            delegate: ColumnLayout {
                id: row
                required property var modelData
                required property int index
                Layout.fillWidth: true
                spacing: 5
                Text {
                    visible: row.modelData.kind === "note" || row.modelData.kind === "text" || row.modelData.kind === "choice"
                    Layout.fillWidth: true
                    text: row.modelData.label
                    textFormat: Text.PlainText; wrapMode: Text.WordWrap
                    color: Theme.text; font.pixelSize: row.modelData.kind === "note" ? 14 : 13
                }
                StitchTextField {
                    objectName: "promptFormText_" + row.index
                    visible: row.modelData.kind === "text"
                    Accessible.name: row.modelData.label
                    Layout.fillWidth: true
                    text: row.modelData.value === null || row.modelData.value === undefined ? "" : String(row.modelData.value)
                    onTextEdited: root.setFormValue(row.modelData.name, text)
                }
                StitchCheckBox {
                    objectName: "promptFormCheck_" + row.index
                    visible: row.modelData.kind === "check"
                    text: row.modelData.label
                    checked: row.modelData.value === true
                    onToggled: root.setFormValue(row.modelData.name, checked)
                }
                StitchComboBox {
                    objectName: "promptFormChoice_" + row.index
                    visible: row.modelData.kind === "choice"
                    Accessible.name: row.modelData.label
                    Layout.fillWidth: true
                    model: row.modelData.options || []
                    currentIndex: Math.max(0, (row.modelData.options || []).indexOf(String(row.modelData.value)))
                    onActivated: index => root.setFormValue(row.modelData.name, model[index])
                    Component.onCompleted: if (row.modelData.kind === "choice" && count > 0) root.setFormValue(row.modelData.name, model[currentIndex])
                }
            }
        }
        Item { Layout.fillHeight: true }
        Flow {
            Layout.fillWidth: true
            layoutDirection: Qt.RightToLeft
            spacing: 8
            Repeater {
                // Right to left, so the first button ends up leftmost.
                model: root.isForm ? root.controls.filter(c => c.kind === "button").reverse() : []
                delegate: StitchButton {
                    required property var modelData
                    required property int index
                    objectName: "promptFormButton_" + modelData.label
                    text: modelData.label
                    primary: !!modelData.close
                    onClicked: root.click(modelData)
                }
            }
        }
    }

    ColumnLayout {
        id: content
        visible: !root.boxed && !root.isForm
        anchors.fill: parent; anchors.margins: 20
        spacing: 14
        RowLayout {
            spacing: 12
            LucideIcon {
                Layout.alignment: Qt.AlignTop
                width: 22; height: 22
                name: root.kind === "permission" ? "lock" : root.kind === "message" ? "message-square" : "info"
                color: root.kind === "permission" ? Theme.warning : Theme.accent
            }
            Text {
                Layout.fillWidth: true
                text: root.message
                wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 14
            }
        }
        Text {
            visible: root.kind === "permission"
            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Macros you import ask before running commands, going online, writing files, reading secrets or reading the screen. Change what this macro may do in its Macro settings.")
        }
        StitchTextField {
            id: field
            objectName: "promptText"
            Accessible.name: root.message
            visible: root.kind === "text"
            Layout.fillWidth: true
            text: root.request.default || ""
            onAccepted: root.accept()
        }
        RowLayout {
            visible: root.picksPath
            Layout.fillWidth: true
            spacing: 8
            StitchTextField {
                id: pathField
                objectName: "promptPath"
                Accessible.name: root.message
                Layout.fillWidth: true
                placeholderText: root.kind === "folder" ? qsTr("Folder path") : qsTr("File path")
                onAccepted: root.accept()
            }
            StitchButton {
                objectName: "promptBrowse"
                text: qsTr("Browse...")
                onClicked: root.browse()
            }
        }
        StitchListView {
            id: choices
            objectName: "promptChoices"
            Accessible.role: Accessible.List
            Accessible.name: root.message
            visible: root.kind === "choice"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(240, contentHeight)
            clip: true; currentIndex: 0
            model: root.request.options || []

            delegate: Rectangle {
                required property string modelData
                required property int index
                objectName: "promptChoice_" + index
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData
                Accessible.selected: ListView.isCurrentItem
                Accessible.onPressAction: choices.currentIndex = index
                width: choices.width; height: 34; radius: 6
                color: ListView.isCurrentItem ? Theme.selection : hover.hovered ? Theme.panelRaised : "transparent"
                Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                HoverHandler { id: hover }
                Text { anchors.verticalCenter: parent.verticalCenter; x: 10; width: parent.width - 20; text: modelData; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 13 }
                TapHandler { onTapped: choices.currentIndex = index; onDoubleTapped: root.answer("ok", modelData) }
            }
            Keys.onUpPressed: decrementCurrentIndex()
            Keys.onDownPressed: incrementCurrentIndex()
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Item { Layout.fillWidth: true }
            Repeater {
                model: root.buttons
                delegate: StitchButton {
                    required property var modelData
                    objectName: "promptButton_" + modelData[1]
                    text: modelData[0]; primary: modelData[2]
                    onClicked: modelData[1] === "ok" ? root.accept() : root.answer(modelData[1], "")
                }
            }
        }
    }
}
