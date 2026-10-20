import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// The community block library: browse packs, review one before its blocks
// join the open macro, and share this macro's blocks as a pack.
Item {
    id: root
    required property var appState
    required property var invoke
    required property var appBridge
    required property var macroFiles

    readonly property var library: appState.block_library || ({ loaded: false, loading: false, packs: [], sources: [], fetching: "", error: "" })
    readonly property var macro: appState.current_macro || null

    function browse() {
        if (!library.loaded && !library.loading) invoke("refresh_block_library");
        browseDialog.open();
    }
    function addFromFile() { packFileDialog.open(); }
    function share(definition) {
        shareDialog.picked = definition ? [definition.id] : [];
        shareDialog.open();
    }
    function blockLabel(def) {
        return (def.pieces || []).map(p => p.kind === "Label" ? p.text : "(" + p.name + ")").join(" ");
    }
    function localPath(url) {
        const text = decodeURIComponent(url.toString());
        return Qt.platform.os === "windows" ? text.replace(/^file:\/\/\//, "") : text.replace(/^file:\/\//, "");
    }

    property int callSeq: 0
    property var callbacks: ({})
    function daemonCall(command, args, done) {
        const key = "library" + (++callSeq);
        callbacks[key] = done;
        root.appBridge.callCommand(key, command, JSON.stringify(args || {}));
    }
    Connections {
        target: root.appBridge
        function onReplySerialChanged() {
            const key = root.appBridge.replyKey, done = root.callbacks[key];
            if (!done) return;
            delete root.callbacks[key];
            if (root.appBridge.replyError) failed.show(root.appBridge.replyError);
            else done(JSON.parse(root.appBridge.replyJson || "null"));
        }
    }

    AppDialog {
        id: browseDialog
        title: qsTr("Block library")
        readonly property string query: search.text.trim().toLowerCase()
        readonly property var shown: (root.library.packs || []).filter(p => !query
            || (p.name + " " + p.description + " " + p.author + " " + p.blocks.join(" ")).toLowerCase().indexOf(query) >= 0)
        readonly property var brokenSources: (root.library.sources || []).filter(s => s.error)
        width: Math.min(624, parent.width - 32)
        contentItem: ColumnLayout {
            implicitWidth: 580; spacing: 10
            RowLayout {
                Layout.fillWidth: true; spacing: 8
                BwTextField { id: search; Layout.fillWidth: true; placeholderText: qsTr("Search blocks") }
                BwButton {
                    iconName: "refresh-cw"; text: qsTr("Refresh"); enabled: !root.library.loading
                    onClicked: root.invoke("refresh_block_library")
                }
                BwButton { iconName: "folder-open"; text: qsTr("From file..."); onClicked: root.addFromFile() }
            }
            Text {
                visible: !root.macro
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 12
                text: qsTr("Open a macro to add blocks to it.")
            }
            Text {
                visible: root.library.error !== ""
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 12
                text: root.library.error
            }
            Repeater {
                model: browseDialog.brokenSources
                delegate: Text {
                    required property var modelData
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 12
                    text: qsTr("Couldn't load %1: %2").arg(modelData.url).arg(modelData.error)
                }
            }
            Text {
                visible: root.library.loading
                color: Theme.textDim; font.pixelSize: 13
                text: qsTr("Loading the library...")
            }
            Text {
                visible: !root.library.loading && root.library.loaded && browseDialog.shown.length === 0
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 13
                text: (root.library.packs || []).length ? qsTr("No packs match your search.")
                    : qsTr("No packs here yet. Add a library in Settings > Block library, or add a pack from a file.")
            }
            ListView {
                id: packList
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(420, contentHeight)
                clip: true; spacing: 6
                model: browseDialog.shown
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: Rectangle {
                    id: packRow
                    required property var modelData
                    readonly property bool fetching: root.library.fetching === modelData.key
                    width: packList.width; radius: 8
                    implicitHeight: packCol.implicitHeight + 20
                    color: Theme.panelRaised; border.color: Theme.borderSoft
                    ColumnLayout {
                        id: packCol
                        anchors.left: parent.left; anchors.right: addButton.left; anchors.top: parent.top
                        anchors.margins: 10; anchors.rightMargin: 12; spacing: 3
                        Text {
                            Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 14; font.weight: Font.DemiBold
                            text: packRow.modelData.name + (packRow.modelData.version ? "  " + packRow.modelData.version : "")
                        }
                        Text {
                            visible: text !== ""
                            Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.textDim; font.pixelSize: 11
                            text: [packRow.modelData.author ? qsTr("by %1").arg(packRow.modelData.author) : "", packRow.modelData.source].filter(s => s).join(" - ")
                        }
                        Text {
                            visible: packRow.modelData.description !== ""
                            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 12
                            text: packRow.modelData.description
                        }
                        Text {
                            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
                            text: packRow.modelData.blocks.join("  -  ")
                        }
                    }
                    BwButton {
                        id: addButton
                        anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter
                        readonly property bool installed: packRow.modelData.installed !== ""
                        enabled: !!root.macro && !packRow.fetching && root.library.fetching === "" && (!installed || packRow.modelData.update)
                        primary: !installed || packRow.modelData.update
                        iconName: installed && !packRow.modelData.update ? "check" : "download"
                        text: packRow.fetching ? qsTr("Downloading...") : packRow.modelData.update ? qsTr("Update")
                            : installed ? qsTr("Added") : qsTr("Add")
                        onClicked: root.invoke("review_block_pack", { key: packRow.modelData.key })
                    }
                }
            }
        }
    }

    FileDialog {
        id: packFileDialog
        title: qsTr("Add blocks from a pack")
        fileMode: FileDialog.OpenFile
        currentFolder: StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        nameFilters: [qsTr("Block packs (*.blockpack *.json)"), qsTr("All files (*)")]
        onAccepted: root.daemonCall("review_block_pack_file", { path: root.localPath(selectedFile) }, () => {})
    }

    // A pack waits here until the user has seen what its blocks can do.
    readonly property var review: appState.pack_review || null
    readonly property string reviewKey: review ? JSON.stringify(review) : ""
    onReviewKeyChanged: review ? reviewDialog.open() : reviewDialog.close()
    AppDialog {
        id: reviewDialog
        readonly property var r: root.review || ({ blocks: [], replaces: [], capabilities: [], notes: [] })
        title: qsTr("Add \"%1\"?").arg(r.name || "")
        standardButtons: Dialog.Ok | Dialog.Cancel
        acceptText: qsTr("Add blocks")
        Column {
            width: 470; spacing: 10
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: [reviewDialog.r.version ? qsTr("Version %1").arg(reviewDialog.r.version) : "",
                       reviewDialog.r.author ? qsTr("by %1").arg(reviewDialog.r.author) : "",
                       qsTr("from %1").arg(reviewDialog.r.source)].filter(s => s).join(" - ")
            }
            Row {
                spacing: 6
                LucideIcon {
                    anchors.verticalCenter: parent.verticalCenter; width: 14; height: 14
                    name: reviewDialog.r.verified ? "check" : "info"; color: reviewDialog.r.verified ? "#4cc38a" : Theme.textDim
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter; color: Theme.textDim; font.pixelSize: 12
                    text: reviewDialog.r.verified ? qsTr("Matches the checksum its library lists.") : qsTr("From a file, so there's no checksum to compare.")
                }
            }
            Text {
                visible: (reviewDialog.r.description || "") !== ""
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: reviewDialog.r.description || ""
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: qsTr("Adds these blocks to \"%1\":").arg(reviewDialog.r.macro_name || "")
            }
            Repeater {
                model: reviewDialog.r.blocks
                delegate: Text {
                    required property string modelData
                    width: 470; elide: Text.ElideRight; color: Theme.textDim; font.pixelSize: 12
                    text: "• " + modelData
                }
            }
            Text {
                visible: reviewDialog.r.replaces.length > 0
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 12
                text: qsTr("This macro already has %n block(s) from an earlier version. They're replaced, along with any changes you made to them; blocks that use them keep working.", "", reviewDialog.r.replaces.length)
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: reviewDialog.r.capabilities.length ? qsTr("These blocks can:") : qsTr("These blocks only use blocks that stay inside Blockwork, like waits and variables.")
            }
            Repeater {
                model: reviewDialog.r.capabilities
                delegate: Row {
                    required property var modelData
                    spacing: 8
                    LucideIcon {
                        anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16
                        name: modelData.risky ? "triangle-alert" : "check"; color: modelData.risky ? Theme.warning : Theme.textDim
                    }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: root.macroFiles.capabilityLabel(modelData); color: modelData.risky ? Theme.warning : Theme.text; font.pixelSize: 13 }
                }
            }
            Text {
                visible: reviewDialog.r.capabilities.some(c => c.guarded)
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Blocks run with the permissions of the macro you add them to, so only add these from people you trust.")
            }
            Repeater {
                model: reviewDialog.r.notes
                delegate: Text {
                    required property string modelData
                    width: 470; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                    text: "• " + modelData
                }
            }
        }
        onAccepted: root.invoke("confirm_block_pack")
        onRejected: root.invoke("cancel_block_pack")
    }

    AppDialog {
        id: shareDialog
        title: qsTr("Share blocks as a pack")
        width: Math.min(504, parent.width - 32)
        standardButtons: Dialog.Ok | Dialog.Cancel
        property var picked: []
        readonly property var defs: root.macro ? root.macro.block_defs || [] : []
        acceptText: qsTr("Save pack...")
        onOpened: {
            packName.text = picked.length === 1 ? root.blockLabel(defs.find(d => d.id === picked[0]) || {}) : (root.macro ? root.macro.name : "");
            packName.forceActiveFocus();
        }
        contentItem: ColumnLayout {
            implicitWidth: 460; spacing: 8
            Text { text: qsTr("Name"); color: Theme.textDim; font.pixelSize: 12 }
            BwTextField { id: packName; Layout.fillWidth: true }
            Text { text: qsTr("What the blocks do"); color: Theme.textDim; font.pixelSize: 12 }
            BwTextField { id: packDescription; Layout.fillWidth: true }
            RowLayout {
                Layout.fillWidth: true; spacing: 8
                ColumnLayout {
                    Layout.fillWidth: true; spacing: 4
                    Text { text: qsTr("Author"); color: Theme.textDim; font.pixelSize: 12 }
                    BwTextField { id: packAuthor; Layout.fillWidth: true }
                }
                ColumnLayout {
                    spacing: 4
                    Text { text: qsTr("Version"); color: Theme.textDim; font.pixelSize: 12 }
                    BwTextField { id: packVersion; Layout.preferredWidth: 110; text: "1.0.0" }
                }
            }
            Text { topPadding: 4; text: qsTr("Blocks"); color: Theme.textDim; font.pixelSize: 12 }
            Repeater {
                model: shareDialog.defs
                delegate: BwCheckBox {
                    required property var modelData
                    text: root.blockLabel(modelData)
                    checked: shareDialog.picked.indexOf(modelData.id) >= 0
                    onToggled: shareDialog.picked = checked ? shareDialog.picked.concat([modelData.id]) : shareDialog.picked.filter(id => id !== modelData.id)
                }
            }
            Text {
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Blocks these use come along, with the variables and lists they need. Secret values are left out. Keep the same name for later versions so they replace the old blocks.")
            }
        }
        onAccepted: {
            if (!picked.length) { failed.show(qsTr("Pick at least one block to share.")); return; }
            const folder = StandardPaths.writableLocation(StandardPaths.DocumentsLocation).toString();
            packSaveDialog.currentFolder = folder;
            packSaveDialog.selectedFile = folder + "/" + encodeURIComponent((packName.text.trim() || "blocks").replace(/[\\/:*?"<>|]/g, "_") + ".blockpack");
            packSaveDialog.open();
        }
    }
    FileDialog {
        id: packSaveDialog
        title: qsTr("Save block pack")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "blockpack"
        nameFilters: [qsTr("Block packs (*.blockpack)")]
        onAccepted: root.daemonCall("export_block_pack", {
            blockIds: shareDialog.picked, name: packName.text, description: packDescription.text,
            author: packAuthor.text, version: packVersion.text, path: root.localPath(selectedFile)
        }, count => {
            shared.message = qsTr("Saved %n block(s). Others can add them with My Blocks > Browse > From file, or you can submit the pack to the community library, where it's reviewed before it's listed.", "", count);
            shared.open();
        })
    }
    AppDialog {
        id: shared
        title: qsTr("Pack saved"); standardButtons: Dialog.Ok | Dialog.Close
        acceptText: qsTr("Submit to community library...")
        property string message: ""
        // The library site takes uploads after signing in.
        onAccepted: Qt.openUrlExternally("https://community.blockworked.com/submit")
        Text { width: 400; wrapMode: Text.WordWrap; text: shared.message; color: Theme.text; font.pixelSize: 13 }
    }
    AppDialog {
        id: failed
        title: qsTr("Something went wrong"); standardButtons: Dialog.Ok
        property string message: ""
        function show(text) { message = text; open(); }
        Text { width: 420; wrapMode: Text.WordWrap; text: failed.message; color: Theme.text; font.pixelSize: 13 }
    }
}
