import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// The Block library window: every pack the libraries list, with search,
// filters and sort orders; a page per pack (its blocks, what they can do,
// versions, source and comments); and the signed-in account's own packs.
AppDialog {
    id: browser
    objectName: "blockLibraryBrowser"
    // BlockLibrary.qml, for its commands and dialogs.
    required property var host
    readonly property var library: host.library
    readonly property var community: host.community
    readonly property var macro: host.macro

    title: page === "pack" ? "" : page === "mine" ? qsTr("My packs") : qsTr("Block library")
    width: Math.min(1000, parent ? parent.width - 32 : 1000)
    height: Math.min(780, parent ? parent.height - 32 : 780)

    // "list", "pack" or "mine"
    property string page: "list"
    property var packKey: ""
    function showList() { page = "list"; }
    function showPack(key, tab) {
        packKey = key;
        packPage.tab = tab || "overview";
        packPage.versionSha = "";
        packPage.showFile = false;
        page = "pack";
        packPage.load(false);
    }
    function showMine() { page = "mine"; minePage.load(true); }
    onOpened: page === "list" && requestPreviews()

    // Block definitions per pack key, drawn on the cards.
    property var previews: ({})
    function requestPreviews() {
        const keys = (library.packs || []).filter(p => p.key && !(p.key in previews)).map(p => p.key);
        if (!keys.length) return;
        host.daemonCall("block_library_previews", { keys: keys }, found => {
            if (!found || !Object.keys(found).length) return;
            previews = Object.assign({}, previews, found);
        }, () => {});
    }
    Connections {
        target: browser.host
        function onLibraryChanged() { if (browser.opened) browser.requestPreviews(); }
        function onCommunityRevisionChanged() {
            if (!browser.opened) return;
            browser.requestPreviews();
            if (browser.page === "pack") packPage.load(false);
            if (browser.page === "mine") minePage.load(false);
        }
    }

    // Filters

    readonly property var permissionFilters: [
        ["", qsTr("Any permissions")], ["none", qsTr("Needs no permissions")], ["safe", qsTr("Nothing risky")],
        ["commands", qsTr("Runs commands")], ["secrets", qsTr("Reads secrets")], ["network", qsTr("Goes online")],
        ["files_write", qsTr("Writes files")], ["files_read", qsTr("Reads files")], ["input", qsTr("Presses keys and clicks")],
        ["clipboard", qsTr("Uses the clipboard")], ["windows", qsTr("Controls windows")], ["triggers", qsTr("Starts on its own")],
        ["screen", qsTr("Reads the screen")], ["macros", qsTr("Runs other macros")], ["globals", qsTr("Uses global variables")],
        ["browser", qsTr("Controls the browser")]
    ]
    readonly property var sorts: [
        ["match", qsTr("Best match")], ["name", qsTr("Name")], ["updated", qsTr("Recently updated")],
        ["new", qsTr("Newest")], ["discussed", qsTr("Most discussed")]
    ]
    readonly property string query: search.text.trim()
    readonly property string sortKey: sortBox.currentIndex >= 0 ? sorts[sortBox.currentIndex][0] : "name"
    readonly property string usesKey: usesBox.currentIndex >= 0 ? permissionFilters[usesBox.currentIndex][0] : ""
    onQueryChanged: {
        // A search sorts by how well packs match, until another order is picked.
        if (query && !sortTouched) sortBox.currentIndex = 0;
        else if (!query && sortBox.currentIndex === 0) sortBox.currentIndex = 1;
    }
    property bool sortTouched: false

    function fold(text) { return String(text || "").normalize("NFKD").replace(/[̀-ͯ]/g, "").toLowerCase(); }
    // How well a pack matches every word, or 0 if one word is missing. Names count most.
    function score(p, terms) {
        if (!terms.length) return 1;
        const name = fold(p.name + " " + p.id), blocks = fold((p.blocks || []).join(" ")), rest = fold(p.description + " " + p.author);
        let total = 0;
        for (const t of terms) {
            const s = (name.indexOf(t) >= 0 ? 4 : 0) + (blocks.indexOf(t) >= 0 ? 2 : 0) + (rest.indexOf(t) >= 0 ? 1 : 0);
            if (!s) return 0;
            total += s;
        }
        return total;
    }
    readonly property var shown: {
        const terms = fold(query).split(/\s+/).filter(t => t);
        const uses = usesKey;
        const rows = (library.packs || []).map(p => ({ p: p, score: score(p, terms) })).filter(({ p, score }) => {
            if (!score) return false;
            if (hideInstalled.checked && p.installed) return false;
            if (officialOnly.checked && !p.official) return false;
            const caps = (p.capabilities || []);
            if (uses === "none") return caps.length === 0;
            if (uses === "safe") return !caps.some(c => c.risky);
            return !uses || caps.some(c => c.kind === uses);
        });
        const byName = (a, b) => a.p.name.localeCompare(b.p.name, undefined, { sensitivity: "base" });
        const order = {
            match: (a, b) => b.score - a.score || byName(a, b),
            name: byName,
            updated: (a, b) => (b.p.updated || 0) - (a.p.updated || 0) || byName(a, b),
            "new": (a, b) => (b.p.created || 0) - (a.p.created || 0) || byName(a, b),
            discussed: (a, b) => (b.p.comments || 0) - (a.p.comments || 0) || byName(a, b)
        }[sortKey] || byName;
        return rows.sort(order).map(r => r.p);
    }
    readonly property bool filtered: query !== "" || usesKey !== "" || officialOnly.checked || hideInstalled.checked
    function clearFilters() {
        search.text = ""; usesBox.currentIndex = 0; officialOnly.checked = false; hideInstalled.checked = false;
    }

    function date(ms) { return ms ? Qt.locale().toString(new Date(ms), "MMM d, yyyy") : ""; }
    function plural(n, one, many) { return n === 1 ? one.arg(n) : many.arg(n); }
    readonly property var currentPack: (library.packs || []).find(p => p.key === packKey) || null

    contentItem: StackLayout {
        currentIndex: browser.page === "pack" ? 1 : browser.page === "mine" ? 2 : 0

        // The list
        ColumnLayout {
            spacing: 10
            RowLayout {
                Layout.fillWidth: true; spacing: 8
                BwTextField {
                    id: search; objectName: "librarySearch"
                    Layout.fillWidth: true; placeholderText: qsTr("Search packs, blocks and authors")
                }
                BwButton {
                    iconName: "refresh-cw"; text: qsTr("Refresh"); enabled: !browser.library.loading
                    onClicked: { browser.previews = ({}); browser.host.invoke("refresh_block_library"); }
                }
                BwButton { iconName: "folder-open"; text: qsTr("From file..."); onClicked: browser.host.addFromFile() }
                CommunityParts.AccountButton { host: browser.host; browser: browser }
            }
            Flow {
                Layout.fillWidth: true; spacing: 8
                BwComboBox {
                    id: usesBox; objectName: "libraryPermissions"
                    width: 210
                    model: browser.permissionFilters.map(f => f[1])
                }
                BwComboBox {
                    id: sortBox; objectName: "librarySort"
                    width: 180; currentIndex: 1
                    model: browser.sorts.map(s => s[1])
                    onActivated: browser.sortTouched = true
                }
                BwCheckBox { id: officialOnly; text: qsTr("Official only"); height: 34 }
                BwCheckBox { id: hideInstalled; text: qsTr("Hide installed"); height: 34 }
            }
            Text {
                visible: !browser.macro
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 12
                text: qsTr("Open a macro to add blocks to it.")
            }
            Text {
                visible: browser.library.error !== ""
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 12
                text: browser.library.error
            }
            Repeater {
                model: (browser.library.sources || []).filter(s => s.error)
                delegate: Text {
                    required property var modelData
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 12
                    text: qsTr("Couldn't load %1: %2").arg(modelData.url).arg(modelData.error)
                }
            }
            RowLayout {
                Layout.fillWidth: true; spacing: 12
                Text {
                    color: Theme.textDim; font.pixelSize: 12
                    text: browser.library.loading ? qsTr("Loading the library...")
                        : browser.filtered ? qsTr("%1 of %2 packs").arg(browser.shown.length).arg((browser.library.packs || []).length)
                        : browser.plural((browser.library.packs || []).length, qsTr("%1 pack"), qsTr("%1 packs"))
                }
                CommunityParts.LinkText { visible: browser.filtered; text: qsTr("Clear filters"); onClicked: browser.clearFilters() }
                Item { Layout.fillWidth: true }
            }
            Text {
                visible: !browser.library.loading && browser.library.loaded && browser.shown.length === 0
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 13
                text: (browser.library.packs || []).length ? qsTr("No packs match that.")
                    : qsTr("No packs here yet. Add a library in Settings > Block library, or add a pack from a file.")
            }
            ListView {
                id: packList
                Layout.fillWidth: true; Layout.fillHeight: true
                clip: true; spacing: 8
                model: browser.shown
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                WheelPan { flickable: packList }
                delegate: PackCard {
                    width: packList.width - 12
                    owner: browser
                }
            }
        }

        // A pack's page
        CommunityPackPage {
            id: packPage
            browser: browser
        }

        // The account's packs
        CommunityMyPacks {
            id: minePage
            browser: browser
        }
    }

    // A pack in the list: its details, its blocks and what they can do.
    component PackCard: Rectangle {
        id: card
        required property var modelData
        required property var owner
        readonly property bool fetching: modelData.key !== "" && browser.library.fetching === modelData.key
        readonly property bool installed: modelData.installed !== ""
        readonly property var defs: ((browser.previews[modelData.key] || {}).defs || []).filter(d => !d.read_only)
        readonly property int maxBlocks: 6
        radius: 8
        implicitHeight: cardCol.implicitHeight + 24
        color: cardHover.hovered && modelData.key ? Theme.hover : Theme.panelRaised
        border.color: Theme.borderSoft
        HoverHandler { id: cardHover; cursorShape: card.modelData.key ? Qt.PointingHandCursor : Qt.ArrowCursor }
        TapHandler { enabled: card.modelData.key !== ""; onTapped: card.owner.showPack(card.modelData.key) }

        ColumnLayout {
            id: cardCol
            anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
            anchors.margins: 12; spacing: 6
            RowLayout {
                Layout.fillWidth: true; spacing: 8
                ColumnLayout {
                    Layout.fillWidth: true; spacing: 3
                    RowLayout {
                        Layout.fillWidth: true; spacing: 8
                        Text {
                            Layout.maximumWidth: Math.max(80, cardCol.width - 360)
                            elide: Text.ElideRight; color: Theme.text; font.pixelSize: 15; font.weight: Font.DemiBold
                            text: card.modelData.name
                        }
                        Text { visible: card.modelData.version !== ""; text: card.modelData.version; color: Theme.textDim; font.pixelSize: 12 }
                        CommunityParts.Pill { visible: card.installed && !card.modelData.update; text: qsTr("Installed") }
                        CommunityParts.Pill { visible: card.modelData.update && card.modelData.available; text: qsTr("Update available"); accent: true }
                        Item { Layout.fillWidth: true }
                        Row {
                            visible: card.modelData.comments > 0
                            spacing: 4
                            LucideIcon { name: "message-square"; width: 14; height: 14; color: Theme.textDim; anchors.verticalCenter: parent.verticalCenter }
                            Text { text: card.modelData.comments; color: Theme.textDim; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }
                    CommunityParts.Facts {
                        Layout.fillWidth: true
                        author: card.modelData.author; official: card.modelData.official
                        facts: [
                            card.owner.plural((card.modelData.blocks || []).length, qsTr("%1 block"), qsTr("%1 blocks")),
                            card.modelData.updated ? qsTr("updated %1").arg(card.owner.date(card.modelData.updated)) : "",
                            !card.modelData.available && card.installed ? qsTr("not in the library any more") : ""
                        ].filter(f => f)
                    }
                }
                Row {
                    spacing: 6
                    Layout.alignment: Qt.AlignTop
                    BwButton {
                        visible: card.modelData.available && (!card.installed || card.modelData.update)
                        enabled: !!card.owner.macro && !card.fetching && card.owner.library.fetching === ""
                        primary: true; iconName: "download"
                        text: card.fetching ? qsTr("Downloading...") : card.installed ? qsTr("Update") : qsTr("Add")
                        onClicked: card.owner.host.invoke("review_block_pack", { key: card.modelData.key })
                    }
                    BwButton {
                        visible: card.installed
                        iconName: "settings"; ToolTip.visible: hovered; ToolTip.text: qsTr("Settings"); ToolTip.delay: 400
                        Accessible.name: qsTr("Settings")
                        onClicked: card.owner.host.showPackData(card.modelData.id)
                    }
                    BwButton {
                        visible: card.installed
                        text: qsTr("Remove"); iconName: "trash-2"
                        onClicked: card.owner.host.confirmRemove(card.modelData.id, card.modelData.name)
                    }
                }
            }
            Text {
                visible: card.modelData.description !== ""
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                text: card.modelData.description
            }
            // The pack's blocks, as they'll look in the palette.
            Flow {
                visible: card.defs.length > 0
                Layout.fillWidth: true; spacing: 8
                Repeater {
                    model: card.defs.slice(0, card.maxBlocks)
                    delegate: CommunityBlock {
                        required property var modelData
                        definition: modelData; definitions: card.defs
                    }
                }
                Text {
                    visible: card.defs.length > card.maxBlocks
                    height: 32; verticalAlignment: Text.AlignVCenter
                    color: Theme.textDim; font.pixelSize: 12
                    text: qsTr("and %1 more").arg(card.defs.length - card.maxBlocks)
                }
            }
            // Until the pack is downloaded, its block names.
            Text {
                visible: card.defs.length === 0 && (card.modelData.blocks || []).length > 0
                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: (card.modelData.blocks || []).join("     ")
            }
            CommunityParts.Capabilities {
                Layout.fillWidth: true
                capabilities: card.modelData.capabilities || []
                macroFiles: card.owner.host.macroFiles
                compact: true
            }
        }
    }
}
