import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// One pack's page in the Block library: its blocks and what they can do,
// its versions, its source on a read-only canvas, and its comments.
ColumnLayout {
    id: page
    required property var browser
    readonly property var host: browser.host
    readonly property var pack: browser.currentPack || ({ key: "", id: "", name: "", version: "", description: "", author: "", blocks: [], capabilities: [], installed: "", available: false, update: false, official: false, community: false, page: "", source: "", updated: 0, created: 0 })
    readonly property bool installed: pack.installed !== ""
    readonly property bool fetching: pack.key !== "" && browser.library.fetching === pack.key

    // "overview", "source" or "comments"
    property string tab: "overview"
    // An older version's hash, for the source view, or "" for the current one.
    property string versionSha: ""
    property bool showFile: false
    // From the pack file (block_library_pack) and from the site (community_pack).
    property var view: ({ loading: true })
    property var detail: ({ loading: false, detail: null, error: "" })
    readonly property var site: detail.detail || null
    readonly property var defs: ((view.macro || {}).block_defs || []).filter(d => !d.read_only)
    readonly property var comments: site ? site.comments : []

    function load(refresh) {
        if (!pack.key) return;
        const key = pack.key, sha = versionSha;
        if (refresh || view.loading || view.key !== key || view.asked !== sha) {
            host.daemonCall("block_library_pack", { key: key, sha256: sha }, result => {
                if (browser.packKey !== key || versionSha !== sha) return;
                page.view = Object.assign({ key: key, asked: sha }, result);
                if (!result.loading) Qt.callLater(page.placeStrands);
            }, error => { page.view = { key: key, asked: sha, loading: false, error: error }; });
        }
        if (pack.community) {
            host.daemonCall("community_pack", { packId: pack.id, refresh: !!refresh }, result => {
                if (browser.packKey === key) page.detail = result;
            }, () => {});
        } else {
            detail = { loading: false, detail: null, error: "" };
        }
    }
    onVersionShaChanged: if (browser.page === "pack") load(false)
    Connections {
        target: page.browser
        function onPackKeyChanged() {
            page.view = { loading: true };
            page.detail = { loading: false, detail: null, error: "" };
            commentBox.text = "";
        }
    }

    function versionLabel(v) { return v.version || v.sha256.slice(0, 8); }
    function inputsOf(def) { return (def.pieces || []).filter(p => p.kind === "Input"); }
    function literal(value) {
        if (!value) return "";
        if (value.kind === "Text") return JSON.stringify(value.value);
        if (value.value === undefined || value.value === null) return "";
        return String(value.value);
    }
    // A comment's words as rich text, with web addresses made into links.
    function commentHtml(body) {
        const escape = t => t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
        let out = "", last = 0;
        const re = /https?:\/\/[^\s<>"']+[^\s<>"'.,;:!?)\]]/g;
        let m;
        while ((m = re.exec(body)) !== null) {
            out += escape(body.slice(last, m.index));
            out += "<a href=\"" + escape(m[0]) + "\">" + escape(m[0]) + "</a>";
            last = m.index + m[0].length;
        }
        return (out + escape(body.slice(last))).replace(/\n/g, "<br>");
    }

    // The pack's strands, stood one under the other when they share a spot,
    // and moved so the top left one starts near the view's corner.
    property var strands: []
    property point shift: Qt.point(0, 0)
    function placeStrands() {
        const mac = view.macro;
        let list = mac ? mac.strands || [] : [];
        const spots = list.map(s => s.x + "," + s.y);
        if (spots.some((spot, i) => spots.indexOf(spot) !== i)) {
            let y = 0;
            list = list.map(s => {
                const at = Object.assign({}, s, { x: 0, y: y });
                y += canvas.contentHeightFor(s.instructions) + 48;
                return at;
            });
        }
        let minX = Infinity, minY = Infinity;
        list.forEach(s => { minX = Math.min(minX, s.x); minY = Math.min(minY, s.y); });
        const dx = isFinite(minX) ? -canvas.width / 2 + 32 - minX : 0;
        const dy = isFinite(minY) ? -canvas.height / 2 + 32 - minY : 0;
        shift = Qt.point(dx, dy);
        strands = list.map(s => Object.assign({}, s, { x: s.x + dx, y: s.y + dy }));
        canvas.centerOnOrigin();
    }
    function moved(list) {
        return (list || []).map(o => Object.assign({}, o, { x: o.x + shift.x, y: o.y + shift.y }));
    }

    spacing: 10

    RowLayout {
        Layout.fillWidth: true; spacing: 8
        BwButton { objectName: "libraryBackButton"; flat: true; iconName: "arrow-left"; text: qsTr("All packs"); onClicked: page.browser.showList() }
        Item { Layout.fillWidth: true }
        CommunityParts.AccountButton { host: page.host; browser: page.browser }
    }

    RowLayout {
        Layout.fillWidth: true; spacing: 10
        ColumnLayout {
            Layout.fillWidth: true; spacing: 4
            RowLayout {
                Layout.fillWidth: true; spacing: 10
                Text {
                    objectName: "packPageTitle"
                    Layout.maximumWidth: Math.max(120, page.width - 520)
                    elide: Text.ElideRight; color: Theme.text; font.pixelSize: 22; font.weight: Font.DemiBold
                    text: page.pack.name
                }
                Text { visible: page.pack.version !== ""; text: page.pack.version; color: Theme.textDim; font.pixelSize: 14; Layout.alignment: Qt.AlignBaseline }
                CommunityParts.Pill { visible: page.installed && !page.pack.update; text: qsTr("Installed %1").arg(page.pack.installed) }
                CommunityParts.Pill { visible: !!page.pack.update && !!page.pack.available; accent: true; text: qsTr("Update available") }
                Item { Layout.fillWidth: true }
            }
            CommunityParts.Facts {
                Layout.fillWidth: true
                author: page.pack.author; official: !!page.pack.official
                facts: [
                    page.pack.updated ? qsTr("updated %1").arg(page.browser.date(page.pack.updated)) : "",
                    page.pack.created ? qsTr("first shared %1").arg(page.browser.date(page.pack.created)) : "",
                    page.pack.source && !page.pack.community ? qsTr("from %1").arg(page.pack.source) : ""
                ].filter(f => f)
            }
        }
        Row {
            spacing: 6
            Layout.alignment: Qt.AlignTop
            BwButton {
                objectName: "packPageAdd"
                visible: page.pack.available && (!page.installed || page.pack.update)
                enabled: !!page.browser.macro && !page.fetching && page.browser.library.fetching === ""
                primary: true; iconName: "download"
                text: page.fetching ? qsTr("Downloading...") : page.installed ? qsTr("Update") : qsTr("Add to macro")
                onClicked: page.host.invoke("review_block_pack", { key: page.pack.key })
            }
            BwButton { visible: page.installed; text: qsTr("Settings"); iconName: "settings"; onClicked: page.host.showPackData(page.pack.id) }
            BwButton { visible: page.installed; text: qsTr("Remove"); iconName: "trash-2"; onClicked: page.host.confirmRemove(page.pack.id, page.pack.name) }
            BwButton {
                visible: page.pack.page !== ""
                iconName: "external-link"; Accessible.name: qsTr("Open on the website")
                ToolTip.visible: hovered; ToolTip.delay: 400; ToolTip.text: qsTr("Open on the website")
                onClicked: Qt.openUrlExternally(page.pack.page)
            }
        }
    }

    CommunityParts.Tabs {
        objectName: "packPageTabs"
        current: page.tab
        tabs: [{ key: "overview", label: qsTr("Overview") }, { key: "source", label: qsTr("Source") }]
            .concat(page.pack.community ? [{ key: "comments", label: page.site ? qsTr("Comments (%1)").arg(page.comments.length) : qsTr("Comments") }] : [])
        onPicked: key => {
            page.tab = key;
            if (key === "source") Qt.callLater(page.placeStrands);
        }
    }
    Rectangle { Layout.fillWidth: true; Layout.topMargin: -10; height: 1; color: Theme.borderSoft }

    Text {
        visible: !!page.view.error
        Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 12
        text: page.view.error || ""
    }

    StackLayout {
        Layout.fillWidth: true; Layout.fillHeight: true
        currentIndex: page.tab === "source" ? 1 : page.tab === "comments" ? 2 : 0

        // Overview
        ScrollView {
            id: overviewScroll
            clip: true; contentWidth: availableWidth
            WheelPan { flickable: overviewScroll.contentItem }
            ColumnLayout {
                width: overviewScroll.availableWidth - 12; spacing: 10
                Text {
                    visible: page.pack.description !== ""
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 15
                    text: page.pack.description
                }
                Text {
                    visible: !page.browser.macro
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 12
                    text: qsTr("Open a macro to add these blocks to it.")
                }
                Heading { text: qsTr("What it can do") }
                CommunityParts.Capabilities {
                    Layout.fillWidth: true
                    capabilities: page.view.capabilities || page.pack.capabilities || []
                    macroFiles: page.host.macroFiles
                }
                Heading {
                    text: page.browser.plural(page.defs.length || (page.pack.blocks || []).length, qsTr("%1 block"), qsTr("%1 blocks"))
                }
                Text {
                    visible: !!page.view.loading
                    color: Theme.textDim; font.pixelSize: 12
                    text: qsTr("Loading its blocks...")
                }
                Repeater {
                    objectName: "packPageBlocks"
                    model: page.defs
                    delegate: Rectangle {
                        id: blockItem
                        required property var modelData
                        readonly property var inputs: page.inputsOf(modelData)
                        Layout.fillWidth: true
                        implicitHeight: blockCol.implicitHeight + 20
                        radius: 8; color: Theme.panelRaised; border.color: Theme.borderSoft
                        ColumnLayout {
                            id: blockCol
                            anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
                            anchors.margins: 10; spacing: 6
                            Flickable {
                                Layout.fillWidth: true
                                Layout.preferredHeight: preview.implicitHeight + 6
                                contentWidth: preview.implicitWidth; contentHeight: height
                                clip: true; flickableDirection: Flickable.HorizontalFlick
                                interactive: contentWidth > width
                                CommunityBlock { id: preview; y: 3; definition: blockItem.modelData; definitions: page.defs }
                            }
                            Text {
                                visible: (blockItem.modelData.description || "") !== ""
                                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                                text: blockItem.modelData.description || ""
                            }
                            Text {
                                visible: (blockItem.modelData.identifier || "") !== ""
                                Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.textDim; font.pixelSize: 11
                                text: qsTr("ID %1").arg(blockItem.modelData.identifier || "")
                            }
                            Repeater {
                                model: blockItem.inputs
                                delegate: Text {
                                    required property var modelData
                                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                                    readonly property string start: page.literal(modelData.default_value)
                                    text: (modelData.value_type === "Bool" ? qsTr("%1: true or false") : qsTr("%1: any value")).arg(modelData.name)
                                        + (start !== "" ? "   " + qsTr("starts as %1").arg(start) : "")
                                }
                            }
                        }
                    }
                }
                CommunityParts.LinkText {
                    visible: page.defs.length > 0
                    text: qsTr("See how they work")
                    onClicked: { page.tab = "source"; Qt.callLater(page.placeStrands); }
                }
                Heading { visible: !!page.site; text: qsTr("Versions") }
                Repeater {
                    model: page.site ? page.site.versions : []
                    delegate: RowLayout {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true; spacing: 12
                        Text { text: page.versionLabel(modelData); color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
                        CommunityParts.Pill { visible: index === 0; text: qsTr("current") }
                        Text { text: page.browser.date(modelData.date); color: Theme.textDim; font.pixelSize: 12 }
                        Item { Layout.fillWidth: true }
                        CommunityParts.LinkText {
                            text: qsTr("View source")
                            onClicked: {
                                page.versionSha = index === 0 ? "" : modelData.sha256;
                                page.tab = "source";
                            }
                        }
                    }
                }
                Text {
                    visible: page.pack.community && !page.site && !!page.detail.error
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                    text: qsTr("Couldn't load its versions and comments: %1").arg(page.detail.error || "")
                }
                Item { implicitHeight: 8 }
            }
        }

        // Source
        ColumnLayout {
            spacing: 8
            RowLayout {
                Layout.fillWidth: true; spacing: 8
                CommunityParts.Tabs {
                    current: page.showFile ? "file" : "blocks"
                    tabs: [{ key: "blocks", label: qsTr("Blocks") }, { key: "file", label: qsTr("Pack file") }]
                    onPicked: key => { page.showFile = key === "file"; if (!page.showFile) Qt.callLater(page.placeStrands); }
                }
                Item { Layout.fillWidth: true }
                Text {
                    visible: page.versionSha !== ""
                    color: Theme.textDim; font.pixelSize: 12
                    text: qsTr("Showing version %1").arg(page.view.version || page.versionSha.slice(0, 8))
                }
                CommunityParts.LinkText { visible: page.versionSha !== ""; text: qsTr("Show current"); onClicked: page.versionSha = "" }
            }
            Rectangle {
                Layout.fillWidth: true; Layout.fillHeight: true
                radius: 8; color: Theme.canvas; border.color: Theme.borderSoft; clip: true
                BlockCanvas {
                    id: canvas
                    objectName: "packSourceCanvas"
                    visible: !page.showFile
                    anchors.fill: parent; anchors.margins: 1
                    locked: true
                    offerClear: false
                    strands: page.strands
                    comments: page.moved(page.view.macro ? page.view.macro.comments : [])
                    floatingValues: page.moved(page.view.macro ? page.view.macro.floating_values : [])
                    variables: page.view.macro ? page.view.macro.variables : []
                    lists: page.view.macro && page.view.macro.lists ? page.view.macro.lists : []
                    blockDefinitions: page.view.macro ? page.view.macro.block_defs : []
                    onWidthChanged: if (visible && page.view.macro) Qt.callLater(page.placeStrands)
                }
                ScrollView {
                    id: fileScroll
                    visible: page.showFile
                    anchors.fill: parent; anchors.margins: 1
                    clip: true
                    TextArea {
                        objectName: "packSourceFile"
                        readOnly: true; selectByMouse: true; wrapMode: TextEdit.NoWrap
                        text: page.view.file || ""
                        font.family: "monospace"; font.pixelSize: 12
                        color: Theme.text
                        background: null
                    }
                }
                Text {
                    anchors.centerIn: parent
                    visible: !!page.view.loading
                    color: Theme.textDim; font.pixelSize: 13
                    text: qsTr("Loading the pack...")
                }
            }
        }

        // Comments
        ScrollView {
            id: commentsScroll
            clip: true; contentWidth: availableWidth
            WheelPan { flickable: commentsScroll.contentItem }
            ColumnLayout {
                width: commentsScroll.availableWidth - 12; spacing: 12
                Text {
                    visible: !page.site && !!page.detail.loading
                    color: Theme.textDim; font.pixelSize: 13
                    text: qsTr("Loading comments...")
                }
                Text {
                    visible: !!page.site && page.comments.length === 0
                    color: Theme.textDim; font.pixelSize: 13
                    text: qsTr("No comments yet.")
                }
                Repeater {
                    objectName: "packComments"
                    model: page.comments
                    delegate: RowLayout {
                        id: comment
                        required property var modelData
                        Layout.fillWidth: true; spacing: 10
                        CommunityParts.Avatar {
                            Layout.alignment: Qt.AlignTop
                            width: 32; height: 32
                            source: comment.modelData.avatar; name: comment.modelData.author
                        }
                        ColumnLayout {
                            Layout.fillWidth: true; spacing: 4
                            RowLayout {
                                Layout.fillWidth: true; spacing: 8
                                Text { text: comment.modelData.author; color: Theme.text; font.pixelSize: 13; font.weight: Font.DemiBold }
                                CommunityParts.VerifiedBadge { visible: comment.modelData.official }
                                CommunityParts.Pill { visible: comment.modelData.pack_author; text: qsTr("author") }
                                Text { text: page.browser.date(comment.modelData.created); color: Theme.textDim; font.pixelSize: 12 }
                                Item { Layout.fillWidth: true }
                                BwButton {
                                    visible: comment.modelData.can_delete
                                    flat: true; implicitHeight: 28; iconName: "trash-2"
                                    text: qsTr("Delete")
                                    enabled: !(page.host.community.tasks.comment || {}).running
                                    onClicked: page.host.invoke("community_delete_comment", { packId: page.pack.id, id: comment.modelData.id })
                                }
                            }
                            Text {
                                Layout.fillWidth: true; wrapMode: Text.Wrap
                                color: Theme.text; font.pixelSize: 13; linkColor: Theme.accent
                                textFormat: Text.StyledText
                                text: page.commentHtml(comment.modelData.body)
                                onLinkActivated: link => Qt.openUrlExternally(link)
                                HoverHandler { cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.IBeamCursor }
                            }
                        }
                    }
                }
                Rectangle { Layout.fillWidth: true; height: 1; color: Theme.borderSoft; visible: !!page.site }
                // Writing one
                ColumnLayout {
                    visible: !!page.site && page.host.community.signed_in && page.site.can_comment
                    Layout.fillWidth: true; spacing: 6
                    TextArea {
                        id: commentBox
                        objectName: "packCommentBox"
                        Layout.fillWidth: true; Layout.preferredHeight: Math.max(90, implicitHeight)
                        wrapMode: TextEdit.Wrap
                        placeholderText: qsTr("Ask a question, share how you use it, or report a problem")
                        color: Theme.text; placeholderTextColor: Theme.textDim; selectionColor: Theme.accent
                        font.pixelSize: 13; padding: 10
                        background: Rectangle {
                            radius: 5; color: Theme.field
                            border.color: commentBox.activeFocus ? Theme.accent : Theme.border
                            border.width: commentBox.activeFocus ? 1.5 : 1
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true; spacing: 8
                        Item { Layout.fillWidth: true }
                        Text {
                            visible: !!(page.host.community.tasks.comment || {}).error
                            color: Theme.danger; font.pixelSize: 12
                            text: (page.host.community.tasks.comment || {}).error || ""
                        }
                        BwButton {
                            objectName: "packCommentPost"
                            primary: true; text: qsTr("Post comment")
                            enabled: commentBox.text.trim() !== "" && commentBox.text.length <= 2000 && !(page.host.community.tasks.comment || {}).running
                            onClicked: {
                                page.postedText = commentBox.text;
                                page.host.invoke("community_post_comment", { packId: page.pack.id, body: commentBox.text });
                            }
                        }
                    }
                }
                Text {
                    visible: !!page.site && page.host.community.signed_in && !page.site.can_comment
                    color: Theme.textDim; font.pixelSize: 13
                    text: qsTr("This account can't comment.")
                }
                RowLayout {
                    visible: !!page.site && !page.host.community.signed_in
                    spacing: 10
                    Text { text: qsTr("Sign in to comment."); color: Theme.textDim; font.pixelSize: 13 }
                    BwButton { iconName: "user"; text: qsTr("Sign in"); onClicked: page.host.signIn() }
                }
                Item { implicitHeight: 8 }
            }
        }
    }

    // Clears the box once the comment it held is posted.
    property string postedText: ""
    readonly property int commentDone: (host.community.tasks.comment || { done: 0 }).done
    onCommentDoneChanged: {
        const task = host.community.tasks.comment || {};
        if (!task.error && postedText && commentBox.text === postedText) commentBox.text = "";
        postedText = "";
    }

    component Heading: Text {
        Layout.fillWidth: true; Layout.topMargin: 6
        color: Theme.text; font.pixelSize: 15; font.weight: Font.DemiBold
    }
}
