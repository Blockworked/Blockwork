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
    readonly property var community: appState.community || ({ site: "", signed_in: false, user: { name: "", avatar: "", page: "" }, sign_in_url: "", revision: 0, tasks: {} })
    readonly property int communityRevision: community.revision

    function browse() {
        if (!library.loaded && !library.loading) invoke("refresh_block_library");
        browseDialog.showList();
        browseDialog.open();
    }
    function confirmRemove(packId, packName) {
        removeDialog.packId = packId;
        removeDialog.packName = packName;
        removeDialog.macroId = root.macro ? root.macro.id : "";
        removeDialog.open();
    }

    // Signing in: the browser opens the site, which hands this computer a code.
    // `then` runs once the sign-in lands.
    property var afterSignIn: null
    function signIn(then) {
        afterSignIn = then || null;
        daemonCall("community_sign_in", {}, result => {
            Qt.openUrlExternally(result.url);
            signInDialog.open();
        });
    }
    readonly property var signInTask: community.tasks.sign_in || ({ running: false, error: "", message: "", done: 0 })
    readonly property int signInDone: signInTask.done
    onSignInDoneChanged: {
        const then = afterSignIn;
        afterSignIn = null;
        if (signInTask.error) { signInDialog.close(); failed.show(signInTask.error); return; }
        signInDialog.close();
        if (then && community.signed_in) then();
    }
    // Runs `action` now if signed in, or after signing in.
    function whenSignedIn(action) {
        if (community.signed_in) action(); else signIn(action);
    }

    // Sharing to the community library
    function nextVersion(version) {
        const m = String(version || "").match(/^(.*?)(\d+)([^\d]*)$/);
        return m ? m[1] + (Number(m[2]) + 1) + m[3] : "1.0.0";
    }
    function publishNew() {
        whenSignedIn(() => {
            if (!root.macro) { failed.show(qsTr("Open the macro your blocks are in first.")); return; }
            shareDialog.publishing = true;
            shareDialog.saved = null;
            // Start with every block the macro made, so a one-block macro is one click.
            shareDialog.picked = shareDialog.defs.map(d => d.id);
            shareDialog.open();
        });
    }
    // An update to one of the account's packs, from this macro's blocks.
    function publishUpdate(pack) {
        whenSignedIn(() => {
            if (!root.macro) { failed.show(qsTr("Open the macro your blocks are in first.")); return; }
            const latest = pack.versions[0] || { version: "", blocks: [] };
            const exported = (root.macro.pack_exports || []).find(p => p.info.id === pack.id);
            const defs = shareDialog.defs;
            const ids = exported ? exported.block_ids.filter(id => defs.some(d => d.id === id))
                : defs.filter(d => (latest.blocks || []).indexOf(root.blockLabel(d)) >= 0).map(d => d.id);
            shareDialog.publishing = true;
            shareDialog.saved = {
                info: { id: pack.id, name: exported ? exported.info.name : pack.name, description: exported ? exported.info.description : pack.description,
                        author: "", version: root.nextVersion(latest.version || (exported ? exported.info.version : "")) },
                block_ids: ids, variable_roles: exported ? exported.variable_roles : []
            };
            shareDialog.picked = ids;
            shareDialog.open();
        });
    }
    function publishFromFile() {
        whenSignedIn(() => publishFileDialog.open());
    }
    function sendFile(path) {
        whenSignedIn(() => {
            daemonCall("community_publish_file", { path: path }, () => {
                browseDialog.showMine();
                browseDialog.open();
            });
        });
    }
    function addFromFile() { packFileDialog.open(); }
    function share(definition) {
        if (definition && definition.read_only) return;
        shareDialog.publishing = false;
        const saved = ((root.macro && root.macro.pack_exports) || []).find(pack => definition && pack.block_ids.indexOf(definition.id) >= 0);
        shareDialog.saved = saved || null;
        shareDialog.picked = saved ? saved.block_ids.filter(id => shareDialog.defs.some(d => d.id === id)) : definition ? [definition.id] : [];
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
    // `fail` takes the error instead of the error dialog.
    function daemonCall(command, args, done, fail) {
        const key = "library" + (++callSeq);
        callbacks[key] = { done: done, fail: fail };
        root.appBridge.callCommand(key, command, JSON.stringify(args || {}));
    }
    Connections {
        target: root.appBridge
        function onReplySerialChanged() {
            const key = root.appBridge.replyKey, call = root.callbacks[key];
            if (!call) return;
            delete root.callbacks[key];
            if (root.appBridge.replyError) {
                if (call.fail) call.fail(root.appBridge.replyError);
                else failed.show(root.appBridge.replyError);
            } else call.done(JSON.parse(root.appBridge.replyJson || "null"));
        }
    }

    BlockLibraryBrowser { id: browseDialog; host: root }

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
        title: publishing ? (saved ? qsTr("Publish an update to %1").arg(saved.info.name) : qsTr("Share a pack with the community"))
            : qsTr("Share blocks as a pack")
        // Sends the pack to the community library instead of saving a file.
        property bool publishing: false
        width: Math.min(624, parent.width - 32)
        height: Math.min(740, parent.height - 32)
        standardButtons: Dialog.Ok | Dialog.Cancel
        property var saved: null
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
        acceptText: publishing ? qsTr("Send for review") : qsTr("Save pack...")
        onOpened: {
            const roles = Object.create(null);
            if (saved) (saved.variable_roles || []).forEach(r => roles[r.name] = r.role);
            variableRoles = roles;
            showVariables = false;
            packName.text = saved ? saved.info.name : picked.length === 1
                ? (defs.find(d => d.id === picked[0]) || {pieces: []}).pieces.filter(p => p.kind === "Label").map(p => p.text).join(" ")
                : root.macro ? root.macro.name : "";
            packDescription.text = saved ? saved.info.description : "";
            packAuthor.text = saved ? saved.info.author : "";
            packVersion.text = saved ? saved.info.version : "1.0.0";
            packName.forceActiveFocus();
        }
        contentItem: ScrollView {
            id: shareScroll
            objectName: "sharePackScroll"
            clip: true; contentWidth: availableWidth
            ColumnLayout {
                width: shareScroll.availableWidth; spacing: 8
                Text { text: qsTr("Name"); color: Theme.textDim; font.pixelSize: 12 }
                BwTextField { id: packName; objectName: "sharePackName"; Layout.fillWidth: true; Layout.minimumWidth: 0 }
                Text { text: qsTr("What the blocks do"); color: Theme.textDim; font.pixelSize: 12 }
                BwTextField { id: packDescription; objectName: "sharePackDescription"; Layout.fillWidth: true; Layout.minimumWidth: 0 }
                RowLayout {
                    Layout.fillWidth: true; spacing: 8
                    ColumnLayout {
                        visible: !shareDialog.publishing
                        Layout.fillWidth: true; spacing: 4
                        Text { text: qsTr("Author"); color: Theme.textDim; font.pixelSize: 12 }
                        BwTextField { id: packAuthor; objectName: "sharePackAuthor"; Layout.fillWidth: true; Layout.minimumWidth: 0 }
                    }
                    Text {
                        visible: shareDialog.publishing
                        Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                        text: qsTr("Shared as %1. Your name goes in front of each block's ID.").arg(root.community.user.name)
                    }
                    ColumnLayout {
                        spacing: 4
                        Text { text: qsTr("Version"); color: Theme.textDim; font.pixelSize: 12 }
                        BwTextField { id: packVersion; objectName: "sharePackVersion"; Layout.preferredWidth: 110; text: "1.0.0" }
                    }
                }
                Text { topPadding: 4; text: qsTr("Blocks"); color: Theme.textDim; font.pixelSize: 12 }
                Text {
                    visible: shareDialog.defs.length === 0
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 13
                    text: shareDialog.saved
                        ? qsTr("This macro has no blocks of its own. Open the macro you made %1's blocks in to publish an update.").arg(shareDialog.saved.info.name)
                        : qsTr("This macro has no blocks of its own yet. Make one with Make a Block in the Custom category first.")
                }
                Repeater {
                    objectName: "sharePackBlockRows"
                    model: shareDialog.defs
                    delegate: RowLayout {
                        id: blockRow
                        required property var modelData
                        Layout.fillWidth: true
                        BwCheckBox {
                            objectName: "sharePackBlockCheck"
                            checked: shareDialog.picked.indexOf(blockRow.modelData.id) >= 0
                            onToggled: shareDialog.picked = checked ? shareDialog.picked.concat([blockRow.modelData.id]) : shareDialog.picked.filter(id => id !== blockRow.modelData.id)
                        }
                        Flickable {
                            objectName: "sharePackBlockViewport"
                            Layout.fillWidth: true; Layout.minimumWidth: 0
                            Layout.preferredHeight: preview.height + 8
                            contentWidth: preview.width; contentHeight: height
                            clip: true; flickableDirection: Flickable.HorizontalFlick
                            Loader {
                                id: preview
                                objectName: "sharePackBlockPreview"
                                readonly property bool reporter: blockRow.modelData.shape === "ReturnsValue" || blockRow.modelData.shape === "ReturnsBool"
                                readonly property var args: (blockRow.modelData.pieces || []).filter(p => p.kind === "Input").map(p => p.default_value || {kind: "Number", value: 0})
                                width: item ? item.implicitWidth : 0
                                height: item ? item.implicitHeight : 0
                                y: 4
                                sourceComponent: reporter ? valuePreview : instructionPreview
                                Component {
                                    id: instructionPreview
                                    InstructionBlock {
                                        locked: true; paletteMode: true
                                        blockDefinitions: shareDialog.defs
                                        instruction: ({type: (blockRow.modelData.pieces || []).some(p => p.kind === "Branch") ? "BranchCallBlock" : "CallBlock",
                                            block_id: blockRow.modelData.id, args: preview.args, branches: []})
                                    }
                                }
                                Component {
                                    id: valuePreview
                                    ValueChip {
                                        editable: false; paletteMode: true
                                        blockDefinitions: shareDialog.defs
                                        forceBoolean: blockRow.modelData.shape === "ReturnsBool"
                                        valueData: ({kind: "Call", block_id: blockRow.modelData.id, args: preview.args})
                                    }
                                }
                                enabled: false
                            }
                        }
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
        }
        onAccepted: {
            if (!picked.length) { failed.show(qsTr("Pick at least one block to share.")); return; }
            if (publishing) {
                root.daemonCall("community_publish", {
                    blockIds: picked, id: saved ? saved.info.id : "", name: packName.text, description: packDescription.text,
                    version: packVersion.text,
                    variableRoles: variableCandidates.map(name => ({name: name, role: variableRoles[name] || "internal"}))
                }, () => { browseDialog.showMine(); browseDialog.open(); });
                return;
            }
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
            blockIds: shareDialog.picked, id: shareDialog.saved ? shareDialog.saved.info.id : "", name: packName.text, description: packDescription.text,
            author: packAuthor.text, version: packVersion.text,
            variableRoles: shareDialog.variableCandidates.map(name => ({name: name, role: shareDialog.variableRoles[name] || "internal"})), path: root.localPath(selectedFile)
        }, count => {
            shared.message = qsTr("Saved %n block(s). Others can add them with My Blocks > Browse > From file, or you can send the pack to the community library, where it's reviewed before it's listed.", "", count);
            shared.path = root.localPath(selectedFile);
            shared.open();
        })
    }
    AppDialog {
        id: shared
        title: qsTr("Pack saved"); standardButtons: Dialog.Ok | Dialog.Close
        acceptText: qsTr("Send to the community library")
        property string message: ""
        property string path: ""
        onAccepted: root.sendFile(path)
        Text { width: 400; wrapMode: Text.WordWrap; text: shared.message; color: Theme.text; font.pixelSize: 13 }
    }
    FileDialog {
        id: publishFileDialog
        title: qsTr("Send a pack to the community library")
        fileMode: FileDialog.OpenFile
        currentFolder: StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        nameFilters: [qsTr("Block packs (*.blockpack *.json)"), qsTr("All files (*)")]
        onAccepted: root.sendFile(root.localPath(selectedFile))
    }
    AppDialog {
        id: signInDialog
        objectName: "communitySignInDialog"
        title: qsTr("Sign in with your browser")
        standardButtons: Dialog.Cancel
        onRejected: { root.afterSignIn = null; root.invoke("community_cancel_sign_in"); }
        Column {
            width: 420; spacing: 12
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: qsTr("Your browser opened the community library. Sign in there with GitHub or Google, then come back here.")
            }
            BwButton {
                flat: true; iconName: "external-link"; text: qsTr("Open the page again")
                enabled: root.community.sign_in_url !== ""
                onClicked: Qt.openUrlExternally(root.community.sign_in_url)
            }
        }
    }
    AppDialog {
        id: failed
        title: qsTr("Something went wrong"); standardButtons: Dialog.Ok
        property string message: ""
        function show(text) { message = text; open(); }
        Text { width: 420; wrapMode: Text.WordWrap; text: failed.message; color: Theme.text; font.pixelSize: 13 }
    }
}
