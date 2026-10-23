import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

AppDialog {
    id: root
    required property var names
    required property var callCommand
    required property string chooseTitle
    required property string createTitle
    required property string createCommand
    property string scopeText: ""
    property string newText: qsTr("New name…")
    property bool allowExpression: false
    property bool caseInsensitive: false
    property var selection: null
    property string selectedName: ""
    property string error: ""
    property bool busy: false
    property bool creating: false
    property int requestSerial: 0
    readonly property string newName: nameField.text.trim()
    readonly property bool exists: (names || []).some(n => root.normalize(n) === root.normalize(newName))
    readonly property var matches: (names || []).filter(n => n.toLowerCase().includes(search.text.toLowerCase()))
    title: creating ? createTitle : chooseTitle
    standardButtons: Dialog.NoButton

    function normalize(name) { return caseInsensitive ? name.replace(/[A-Z]/g, c => c.toLowerCase()) : name; }
    function openFor(done, current) {
        selection = done; selectedName = current || "";
        error = ""; search.text = ""; nameField.text = ""; creating = false;
        requestSerial++;
        open(); search.forceActiveFocus();
    }
    function openForCreate(done) { openFor(done, ""); beginCreate(); }
    function use(name, expression) {
        const done = selection;
        close();
        if (done) done(name, !!expression);
    }
    function beginCreate() {
        nameField.text = search.text.trim(); error = ""; creating = true;
        nameField.forceActiveFocus();
    }
    function create() {
        if (!newName.length || exists || busy) return;
        const chosen = newName, serial = requestSerial;
        busy = true; error = "";
        callCommand(createCommand, { name: chosen }, (result, failure) => {
            busy = false;
            if (serial !== requestSerial) return;
            if (failure) { error = failure; return; }
            use(chosen, false);
        });
    }
    onClosed: { selection = null; requestSerial++; }
    Column {
        width: 400; spacing: 12
        Text { visible: !root.creating && root.scopeText.length > 0; text: root.scopeText; color: Theme.textDim; font.pixelSize: 13 }
        BwTextField {
            id: search; objectName: "namePickerSearch"; width: parent.width; visible: !root.creating
            placeholderText: qsTr("Search names")
            onAccepted: if (root.matches.length === 1) root.use(root.matches[0], false)
        }
        ScrollView {
            id: scroll; visible: !root.creating
            width: parent.width; height: Math.min(240, entries.implicitHeight)
            contentWidth: availableWidth; clip: true
            WheelPan { flickable: scroll.contentItem }
            Column {
                id: entries; width: scroll.availableWidth; spacing: 2
                Repeater {
                    model: root.matches
                    delegate: ItemDelegate {
                        required property string modelData
                        width: entries.width; height: 36
                        leftPadding: 10; rightPadding: 10
                        highlighted: modelData === root.selectedName
                        contentItem: RowLayout {
                            Text { Layout.fillWidth: true; text: modelData; color: Theme.text; font.pixelSize: 13; elide: Text.ElideRight }
                            LucideIcon { visible: modelData === root.selectedName; name: "check"; width: 14; height: 14; color: Theme.accent }
                        }
                        background: Rectangle { radius: 5; color: parent.hovered ? Theme.panelRaised : parent.highlighted ? Theme.field : "transparent" }
                        onClicked: root.use(modelData, false)
                    }
                }
            }
        }
        Text { visible: !root.creating && !root.matches.length; text: root.names.length ? qsTr("No matching names.") : qsTr("No saved names yet."); color: Theme.textDim; font.pixelSize: 13 }
        Text { visible: root.creating; text: qsTr("Name"); color: Theme.text; font.pixelSize: 13 }
        BwTextField { id: nameField; objectName: "namePickerName"; width: parent.width; visible: root.creating; enabled: !root.busy; onAccepted: root.create() }
        Text { visible: root.creating && root.exists; text: qsTr("This name already exists."); color: Theme.warning; font.pixelSize: 13 }
        Text { visible: root.error.length > 0; width: parent.width; wrapMode: Text.WordWrap; text: root.error; color: Theme.danger; font.pixelSize: 13 }
    }
    footer: Item {
        implicitHeight: actions.implicitHeight + 26
        RowLayout {
            id: actions
            anchors.fill: parent; anchors.leftMargin: 18; anchors.rightMargin: 18; anchors.topMargin: 8; anchors.bottomMargin: 18
            spacing: 8
            BwButton { visible: !root.creating; flat: true; iconName: "plus"; text: root.newText; onClicked: root.beginCreate() }
            BwButton { visible: !root.creating && root.allowExpression; flat: true; text: qsTr("Use expression"); onClicked: root.use(root.selectedName, true) }
            Item { Layout.fillWidth: true }
            BwButton { text: qsTr("Cancel"); enabled: !root.busy; onClicked: root.reject() }
            BwButton { visible: root.creating; text: qsTr("Create"); primary: true; enabled: root.newName.length > 0 && !root.exists && !root.busy; onClicked: root.create() }
        }
    }
}
