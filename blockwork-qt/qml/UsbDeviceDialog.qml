import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// Picks the USB device a 'when USB device' block or reporter matches.
AppDialog {
    objectName: "usbDeviceScreen"
    id: usbDialog; title: qsTr("Choose a USB device"); standardButtons: Dialog.Cancel
    // The EditorPage this dialog edits: its macro and invoke().
    required property Item page
    property var request: null
    property var devices: []
    property string error: ""
    property bool loaded: false
    // What the block matches now, if it's plain text. A reporter's request
    // carries its slot's value; a trigger's, the whole instruction.
    readonly property string current: {
        const p = !request ? null : request.kind === "value" ? request.value : request.instruction ? request.instruction.pattern : null;
        return p && p.kind === "Text" ? p.value.trim() : "";
    }
    readonly property bool currentPluggedIn: current === "" || devices.some(d => matches(d, current))
    function matches(device, pattern) {
        const p = pattern.toLowerCase();
        return device.name.toLowerCase().indexOf(p) >= 0 || device.ids.indexOf(p) >= 0;
    }
    function openFor(req) {
        request = req; devices = []; error = ""; loaded = false; customPattern.text = "";
        refresh();
        open();
    }
    // Devices come and go while it's open, so ask again every few seconds.
    function refresh() {
        page.daemonCall("list_usb_devices", {}, (list, error) => {
            loaded = true;
            if (list) { devices = list; usbDialog.error = ""; } else { devices = []; usbDialog.error = error || ""; }
        });
    }
    function use(pattern) {
        if (request.kind === "value") page.invoke("edit_value_field", { location: request.location, text: pattern });
        else page.setBlockUsb(request, pattern);
        close();
    }
    Timer { interval: 2000; repeat: true; running: usbDialog.visible; onTriggered: usbDialog.refresh() }
    Column { width: 460; spacing: 10
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Pick a device that's plugged in. The block matches it by its vendor:product id, so it keeps working on any USB port.") }
        Rectangle {
            visible: usbDialog.current !== "" && usbDialog.loaded && !usbDialog.currentPluggedIn
            width: 460; height: 56; radius: Theme.radius; color: Theme.panelRaised; border.color: Theme.warning
            RowLayout { anchors.fill: parent; anchors.margins: 10; spacing: 10
                Column { Layout.fillWidth: true; spacing: 2
                    Text { width: parent.width; text: usbDialog.current; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                    Text { text: qsTr("Chosen now, but not plugged in"); color: Theme.warning; font.pixelSize: 11 }
                }
                StitchButton { text: qsTr("Keep"); onClicked: usbDialog.close() }
            }
        }
        Text { visible: !usbDialog.loaded; text: qsTr("Looking for USB devices..."); color: Theme.textDim; font.pixelSize: 12 }
        Text { visible: usbDialog.error !== ""; width: parent.width; wrapMode: Text.WordWrap; text: usbDialog.error; color: Theme.danger; font.pixelSize: 12 }
        Text { visible: usbDialog.loaded && usbDialog.error === "" && usbDialog.devices.length === 0; width: parent.width; wrapMode: Text.WordWrap
            text: qsTr("No USB devices are plugged in. Plug one in and it shows up here."); color: Theme.textDim; font.pixelSize: 12 }
        ScrollView {
            visible: usbDialog.devices.length > 0
            id: usbScroll
            width: 460; height: Math.min(usbList.implicitHeight, 300); clip: true
            contentWidth: availableWidth
            WheelPan { flickable: usbScroll.contentItem }
            Column { id: usbList; width: 460; spacing: 8
                Repeater {
                    model: usbDialog.devices
                    delegate: Rectangle {
                        required property var modelData
                        readonly property bool chosen: usbDialog.current !== "" && usbDialog.matches(modelData, usbDialog.current)
                        width: 460; height: 56; radius: Theme.radius; color: Theme.panelRaised; border.color: chosen ? Theme.accent : Theme.border
                        RowLayout { anchors.fill: parent; anchors.margins: 10; spacing: 10
                            Column { Layout.fillWidth: true; spacing: 2
                                Text { width: parent.width; text: modelData.name; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                Text { width: parent.width; color: Theme.textDim; font.pixelSize: 11; elide: Text.ElideRight
                                    text: modelData.maker ? qsTr("%1 · id %2").arg(modelData.maker).arg(modelData.ids) : qsTr("id %1").arg(modelData.ids) }
                            }
                            StitchButton { text: chosen ? qsTr("Chosen") : qsTr("Use"); onClicked: usbDialog.use(modelData.ids) }
                        }
                    }
                }
            }
        }
        RowLayout { width: parent.width; spacing: 8
            StitchButton { text: qsTr("Any USB device"); enabled: usbDialog.current !== ""; onClicked: usbDialog.use("") }
            Item { Layout.fillWidth: true }
            StitchButton { text: qsTr("Refresh"); iconName: "refresh-cw"; onClicked: usbDialog.refresh() }
        }
        Text { text: qsTr("Or part of a device's name or id"); color: Theme.text; font.pixelSize: 12 }
        RowLayout { width: parent.width; spacing: 8
            StitchTextField { id: customPattern; Layout.fillWidth: true; placeholderText: "046d:c52b"; onAccepted: useCustom.clicked() }
            StitchButton { id: useCustom; text: qsTr("Use"); enabled: customPattern.text.trim().length > 0; onClicked: usbDialog.use(customPattern.text.trim()) }
        }
    }
}
