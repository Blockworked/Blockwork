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
        if (definition && definition.read_only) return;
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
        readonly property var shown: (root.library.packs || []).filter(p => (!hideInstalled.checked || !p.installed)
            && (!query || (p.name + " " + p.description + " " + p.author + " " + p.blocks.join(" ")).toLowerCase().indexOf(query) >= 0))
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
            BwCheckBox { id: hideInstalled; text: qsTr("Hide installed") }
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
                WheelPan { flickable: packList }
                delegate: Rectangle {
                    id: packRow
                    required property var modelData
                    readonly property bool fetching: modelData.key !== "" && root.library.fetching === modelData.key
                    readonly property bool installed: modelData.installed !== ""
                    width: packList.width; radius: 8
                    implicitHeight: Math.max(packCol.implicitHeight, packActions.implicitHeight) + 20
                    color: Theme.panelRaised; border.color: Theme.borderSoft
                    ColumnLayout {
                        id: packCol
                        anchors.left: parent.left; anchors.right: packActions.left; anchors.top: parent.top
                        anchors.margins: 10; anchors.rightMargin: 12; spacing: 3
                        Text {
                            Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 14; font.weight: Font.DemiBold
                            text: packRow.modelData.name + (packRow.modelData.version ? "  " + packRow.modelData.version : "")
                        }
                        Row {
                            id: byRow
                            readonly property string author: packRow.modelData.author
                            visible: author !== ""
                            Layout.fillWidth: true; spacing: 4
                            Text {
                                width: Math.min(implicitWidth, Math.max(0, byRow.width - (verifiedBadge.visible ? verifiedBadge.width + byRow.spacing : 0)))
                                anchors.verticalCenter: parent.verticalCenter
                                elide: Text.ElideRight; color: Theme.textDim; font.pixelSize: 11
                                text: qsTr("by %1").arg(byRow.author)
                            }
                            // Same badge the community library site shows next to the team's packs.
                            Rectangle {
                                id: verifiedBadge
                                anchors.verticalCenter: parent.verticalCenter
                                visible: packRow.modelData.official === true
                                implicitWidth: 13; implicitHeight: 13; radius: 6.5; color: Theme.accent
                                Accessible.role: Accessible.Graphic; Accessible.name: verifiedTip.text
                                LucideIcon { anchors.centerIn: parent; width: 9; height: 9; strokeWidth: 3.5; name: "check"; color: "#ffffff" }
                                HoverHandler { id: badgeHover }
                                ToolTip { id: verifiedTip; visible: badgeHover.hovered; delay: 300; text: qsTr("Verified official pack") }
                            }
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
                        Text {
                            visible: packRow.installed
                            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
                            text: !packRow.modelData.available ? qsTr("Installed - unavailable from the library")
                                : qsTr("Installed %1").arg(packRow.modelData.installed)
                        }
                    }
                    Column {
                        id: packActions
                        anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        BwButton {
                            visible: packRow.modelData.available && (!packRow.installed || packRow.modelData.update)
                            enabled: !!root.macro && !packRow.fetching && root.library.fetching === ""
                            primary: true; iconName: "download"
                            text: packRow.fetching ? qsTr("Downloading...") : packRow.installed ? qsTr("Update") : qsTr("Add")
                            onClicked: root.invoke("review_block_pack", { key: packRow.modelData.key })
                        }
                        BwButton {
                            visible: packRow.installed
                            text: qsTr("Settings"); iconName: "settings"
                            onClicked: root.showPackData(packRow.modelData.id)
                        }
                        BwButton {
                            visible: packRow.installed
                            text: qsTr("Remove"); iconName: "trash-2"
                            onClicked: {
                                removeDialog.packId = packRow.modelData.id;
                                removeDialog.packName = packRow.modelData.name;
                                removeDialog.macroId = root.macro.id;
                                removeDialog.open();
                            }
                        }
                    }
                }
            }
        }
    }

    function showPackData(packId) {
        packDataDialog.packId = packId;
        packDataDialog.inspect = false;
        packDataDialog.open();
    }

    AppDialog {
        id: packDataDialog
        objectName: "packDataDialog"
        property string packId: ""
        property bool inspect: false
        readonly property var data: (root.macro && root.macro.pack_data || []).find(p => p.id === packId) || ({ name: "", variables: [] })
        readonly property var settings: data.variables.filter(v => v.role === "setting")
        title: data.name
        standardButtons: Dialog.Close
        width: Math.min(504, parent.width - 32)
        contentItem: ColumnLayout {
            spacing: 10
            Repeater {
                objectName: "packSettingRows"
                model: packDataDialog.settings
                delegate: ColumnLayout {
                    required property var modelData
                    Layout.fillWidth: true; spacing: 4
                    Text { text: modelData.label; color: Theme.text; font.pixelSize: 13 }
                    BwCheckBox {
                        visible: modelData.value.kind === "Bool"
                        text: qsTr("Enabled"); checked: modelData.value.value === true
                        onToggled: root.invoke("set_pack_setting", {packId: packDataDialog.packId, name: modelData.name, value: {kind: "Bool", value: checked}})
                    }
                    BwTextField {
                        objectName: "packSettingValue"
                        Layout.fillWidth: true
                        visible: modelData.value.kind !== "Bool"
                        text: String(modelData.value.value)
                        onEditingFinished: {
                            if (text === String(modelData.value.value)) return;
                            const value = modelData.value.kind === "Number" ? Number(text) : text;
                            if (modelData.value.kind === "Number" && (!text.trim() || !isFinite(value))) {
                                text = String(modelData.value.value); return;
                            }
                            root.invoke("set_pack_setting", {packId: packDataDialog.packId, name: modelData.name, value: {kind: modelData.value.kind, value: value}});
                        }
                    }
                }
            }
            Text {
                visible: !packDataDialog.settings.length
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("This pack has no settings. Use its block inputs to configure it.")
            }
            BwButton {
                objectName: "packInspectButton"
                text: qsTr("Inspect variables"); flat: true
                iconName: packDataDialog.inspect ? "chevron-up" : "chevron-down"
                onClicked: packDataDialog.inspect = !packDataDialog.inspect
            }
            Text {
                visible: packDataDialog.inspect
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 11
                text: qsTr("Values from the last completed run.")
            }
            ScrollView {
                visible: packDataDialog.inspect
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(260, inspector.implicitHeight)
                clip: true
                Column {
                    id: inspector
                    width: parent.width; spacing: 6
                    Repeater {
                        model: packDataDialog.data.variables
                        delegate: Text {
                            required property var modelData
                            width: inspector.width; wrapMode: Text.WrapAnywhere
                            text: modelData.label + " = " + String(modelData.value.value)
                            color: Theme.textDim; font.pixelSize: 12
                        }
                    }
                    Text {
                        visible: !packDataDialog.data.variables.length
                        text: qsTr("No private variables"); color: Theme.textDim; font.pixelSize: 12
                    }
                }
            }
        }
    }

    AppDialog {
        id: removeDialog
        property string packId: ""
        property string packName: ""
        property string macroId: ""
        title: qsTr("Remove %1?").arg(packName)
        standardButtons: Dialog.Ok | Dialog.Cancel
        acceptText: qsTr("Remove pack")
        Text {
            width: 420; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
            text: qsTr("Removes this pack's blocks and their uses from the macro. Blocks shared with another installed pack are kept. Variables and lists are kept. You can undo this change.")
        }
        onAccepted: {
            if (root.macro && root.macro.id === macroId)
                root.daemonCall("remove_block_pack", { packId: packId }, () => {});
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
        objectName: "packReviewDialog"
        readonly property var r: root.review || ({ blocks: [], replaces: [], capabilities: [], notes: [] })
        title: qsTr("Add \"%1\"?").arg(r.name || "")
        standardButtons: Dialog.Ok | Dialog.Cancel
        acceptText: qsTr("Add blocks")
        property bool detailsExpanded: false
        onAboutToShow: detailsExpanded = false
        Column {
            width: 470; spacing: 10
            Text {
                visible: (reviewDialog.r.author || "") !== ""
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("by %1").arg(reviewDialog.r.author || "")
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: reviewDialog.r.blocks.length === 1
                    ? qsTr("1 block will be added to \"%1\".").arg(reviewDialog.r.macro_name || "")
                    : qsTr("%1 blocks will be added to \"%2\".").arg(reviewDialog.r.blocks.length).arg(reviewDialog.r.macro_name || "")
            }
            Text {
                visible: reviewDialog.r.replaces.length > 0
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 12
                text: qsTr("Replaces %n existing block definition(s). Blocks that use them keep working.", "", reviewDialog.r.replaces.length)
            }
            Repeater {
                model: reviewDialog.r.capabilities.filter(c => c.risky)
                delegate: Text {
                    required property var modelData
                    width: parent.width; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 13
                    text: root.macroFiles.capabilityLabel(modelData)
                }
            }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Only add blocks from people you trust.")
            }
            Repeater {
                model: reviewDialog.r.notes
                delegate: Text {
                    required property string modelData
                    width: 470; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                    text: "• " + modelData
                }
            }
            BwButton {
                objectName: "packReviewDetailsButton"
                flat: true
                text: reviewDialog.detailsExpanded ? qsTr("Hide details") : qsTr("Show details")
                iconName: reviewDialog.detailsExpanded ? "chevron-up" : "chevron-down"
                onClicked: reviewDialog.detailsExpanded = !reviewDialog.detailsExpanded
            }
            ScrollView {
                id: reviewDetailsScroll
                objectName: "packReviewDetails"
                width: parent.width
                height: Math.min(260, reviewDetailsContent.implicitHeight)
                visible: reviewDialog.detailsExpanded
                clip: true; contentWidth: availableWidth
                Column {
                    id: reviewDetailsContent
                    width: reviewDetailsScroll.availableWidth; spacing: 10
                    Text {
                        width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                        text: [reviewDialog.r.version ? qsTr("Version %1").arg(reviewDialog.r.version) : "",
                               reviewDialog.r.author ? qsTr("by %1").arg(reviewDialog.r.author) : "",
                               qsTr("from %1").arg(reviewDialog.r.source)].filter(s => s).join(" - ")
                    }
                    Row {
                        width: parent.width; spacing: 6
                        LucideIcon {
                            anchors.verticalCenter: parent.verticalCenter; width: 14; height: 14
                            name: reviewDialog.r.verified ? "check" : "info"; color: reviewDialog.r.verified ? "#4cc38a" : Theme.textDim
                        }
                        Text {
                            width: parent.width - 20; wrapMode: Text.WordWrap
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
                            width: parent.width; elide: Text.ElideRight; color: Theme.textDim; font.pixelSize: 12
                            text: "• " + modelData
                        }
                    }
                    Text {
                        width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                        text: reviewDialog.r.capabilities.length ? qsTr("These blocks can:") : qsTr("These blocks only use blocks that stay inside Blockwork, like waits and variables.")
                    }
                    Repeater {
                        model: reviewDialog.r.capabilities
                        delegate: Row {
                            width: parent.width
                            required property var modelData
                            spacing: 8
                            LucideIcon {
                                anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16
                                name: modelData.risky ? "triangle-alert" : "check"; color: modelData.risky ? Theme.warning : Theme.textDim
                            }
                            Text { width: parent.width - 24; wrapMode: Text.WordWrap; anchors.verticalCenter: parent.verticalCenter; text: root.macroFiles.capabilityLabel(modelData); color: modelData.risky ? Theme.warning : Theme.text; font.pixelSize: 13 }
                        }
                    }
                    Text {
                        visible: reviewDialog.r.capabilities.some(c => c.guarded)
                        width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                        text: qsTr("Blocks use this macro's permissions.")
                    }
                }
            }
        }
        onAccepted: root.invoke("confirm_block_pack")
        onRejected: root.invoke("cancel_block_pack")
    }

    AppDialog {
        id: shareDialog
        objectName: "sharePackDialog"
        title: qsTr("Share blocks as a pack")
        width: Math.min(504, parent.width - 32)
        standardButtons: Dialog.Ok | Dialog.Cancel
        property var picked: []
        property var variableCandidates: []
        property var variableRoles: Object.create(null)
        property bool showVariables: false
        onPickedChanged: {
            if (!picked.length) { variableCandidates = []; return; }
            const selection = JSON.stringify(picked);
            root.daemonCall("pack_variable_candidates", {blockIds: picked}, names => {
                if (JSON.stringify(picked) === selection) variableCandidates = names;
            });
        }
        readonly property var defs: root.macro ? (root.macro.block_defs || []).filter(def => !def.read_only) : []
        acceptText: qsTr("Save pack...")
        onOpened: {
            variableRoles = Object.create(null);
            showVariables = false;
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
            BwButton {
                visible: shareDialog.variableCandidates.length > 0
                flat: true; text: qsTr("Pack variables")
                iconName: shareDialog.showVariables ? "chevron-up" : "chevron-down"
                onClicked: shareDialog.showVariables = !shareDialog.showVariables
            }
            ColumnLayout {
                visible: shareDialog.showVariables
                Layout.fillWidth: true
                Text {
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                    text: qsTr("Keep working variables internal. Outputs become reporter blocks; settings appear in the pack's settings. Use block inputs for per-call options.")
                }
                ScrollView {
                    id: authorVariablesScroll
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(240, authorVariablesContent.implicitHeight)
                    clip: true; contentWidth: availableWidth
                    ColumnLayout {
                        id: authorVariablesContent
                        width: authorVariablesScroll.availableWidth
                        Repeater {
                            objectName: "packVariableRoleRows"
                            model: shareDialog.variableCandidates
                            delegate: RowLayout {
                                required property string modelData
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: modelData; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 12 }
                                BwComboBox {
                                    objectName: "packVariableRole"
                                    model: [qsTr("Internal"), qsTr("Output"), qsTr("Setting")]
                                    currentIndex: ["internal", "output", "setting"].indexOf(shareDialog.variableRoles[modelData] || "internal")
                                    onActivated: {
                                        const roles = Object.assign(Object.create(null), shareDialog.variableRoles);
                                        roles[modelData] = ["internal", "output", "setting"][currentIndex];
                                        shareDialog.variableRoles = roles;
                                    }
                                }
                            }
                        }
                    }
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
            author: packAuthor.text, version: packVersion.text,
            variableRoles: shareDialog.variableCandidates.map(name => ({name: name, role: shareDialog.variableRoles[name] || "internal"})), path: root.localPath(selectedFile)
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
