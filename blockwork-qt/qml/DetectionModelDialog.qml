import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// Picks the detection model an object-detection block runs: a bundled one,
// an imported .onnx file, or a detector URL on this computer.
AppDialog {
    objectName: "detectionModelScreen"
    id: modelDialog; title: qsTr("Choose a detection model"); standardButtons: Dialog.Cancel
    required property Item page
    property var request: null
    property var models: []
    property bool busy: false
    signal failed(string message)
    function openFor(req) {
        request = req; models = []; busy = false; localUrl.text = "";
        page.daemonCall("list_models", {}, (list, error) => { if (list) models = list; else failed(error); });
        open();
    }
    function use(id) { page.setBlockDetector(request, id); close(); }
    function download(id) {
        busy = true;
        page.daemonCall("download_model", { id: id }, (done, error) => {
            busy = false;
            if (done) use(id); else failed(error);
        });
    }
    Column { width: 460; spacing: 10
        Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Models run inside Blockwork. Pick one, add your own .onnx file, or point the block at a detector running on this computer.") }
        Repeater {
            model: modelDialog.models
            delegate: Rectangle {
                required property var modelData
                width: 460; height: 56; radius: Theme.radius; color: Theme.panelRaised; border.color: Theme.border
                RowLayout { anchors.fill: parent; anchors.margins: 10; spacing: 10
                    Column { Layout.fillWidth: true; spacing: 2
                        Text { text: modelData.id; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
                        Text { width: parent.width; text: modelData.about; color: Theme.textDim; font.pixelSize: 11; elide: Text.ElideRight }
                    }
                    StitchButton {
                        enabled: !modelDialog.busy && (modelData.installed || modelData.downloadable)
                        text: modelData.installed ? qsTr("Use") : modelData.downloadable ? qsTr("Download %1 MB").arg(modelData.megabytes) : qsTr("Not published yet")
                        onClicked: modelData.installed ? modelDialog.use(modelData.id) : modelDialog.download(modelData.id)
                    }
                }
            }
        }
        Text { visible: modelDialog.busy; text: qsTr("Downloading..."); color: Theme.textDim; font.pixelSize: 12 }
        StitchButton { text: qsTr("Add a .onnx file..."); iconName: "folder-open"; enabled: !modelDialog.busy; onClicked: modelFile.open() }
        Text { text: qsTr("Or a detector on this computer"); color: Theme.text; font.pixelSize: 12 }
        RowLayout { width: parent.width; spacing: 8
            StitchTextField { id: localUrl; Layout.fillWidth: true; placeholderText: modelDialog.page.catalog.defaultDetector; onAccepted: useUrl.clicked() }
            StitchButton { id: useUrl; text: qsTr("Use"); enabled: localUrl.text.trim().length > 0; onClicked: modelDialog.use(localUrl.text.trim()) }
        }
    }
    FileDialog {
        id: modelFile; title: qsTr("Add a detection model"); fileMode: FileDialog.OpenFile
        currentFolder: StandardPaths.writableLocation(StandardPaths.HomeLocation)
        nameFilters: [qsTr("ONNX models (*.onnx)"), qsTr("All files (*)")]
        onAccepted: {
            const text = decodeURIComponent(selectedFile.toString());
            const path = Qt.platform.os === "windows" ? text.replace(/^file:\/\/\//, "") : text.replace(/^file:\/\//, "");
            modelDialog.page.daemonCall("import_model", { path: path }, (id, error) => { if (id) modelDialog.use(id); else modelDialog.failed(error); });
        }
    }
}
