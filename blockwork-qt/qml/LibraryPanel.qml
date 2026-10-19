import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// The macro library: search, folders, and per-macro actions.
Popup {
    id: root
    required property var appState
    required property var invoke
    // Ids of macros with a run in flight.
    property var runningIds: []
    signal importRequested()
    signal exportRequested(string macroId)
    signal bundleRequested(var macroIds, string name)
    signal templateRequested()
    signal deleteRequested(string macroId, string name)

    padding: 0
    width: 380
    // The parent is the 34px toolbar button, so size from the window instead.
    height: Math.min(560, (Overlay.overlay ? Overlay.overlay.height : 640) - 80)
    focus: true
    onOpened: { query.text = ""; query.forceActiveFocus(); }

    readonly property var entries: appState.library || []
    readonly property string currentId: appState.current_macro ? appState.current_macro.id : ""
    property var collapsed: ({})
    readonly property var folders: {
        const out = [];
        for (const e of entries) if (e.folder && out.indexOf(e.folder) < 0) out.push(e.folder);
        return out.sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
    }
    // Folder headers and macro rows, top-level macros first.
    readonly property var rows: {
        const q = query.text.trim().toLowerCase();
        const byName = (a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
        const hits = entries.filter(e => !q || e.name.toLowerCase().indexOf(q) >= 0 || e.folder.toLowerCase().indexOf(q) >= 0);
        const out = hits.filter(e => !e.folder).sort(byName).map(e => ({ kind: "macro", entry: e }));
        for (const folder of folders) {
            const inside = hits.filter(e => e.folder === folder).sort(byName);
            if (!inside.length) continue;
            const open = !!q || !collapsed[folder];
            out.push({ kind: "folder", name: folder, count: inside.length, open: open });
            if (open) for (const e of inside) out.push({ kind: "macro", entry: e });
        }
        return out;
    }
    function toggleFolder(name) {
        const next = Object.assign({}, collapsed);
        next[name] = !next[name];
        collapsed = next;
    }
    function openMacro(id) { root.invoke("select_macro_by_id", { id: id }); root.close(); }

    background: Rectangle { radius: 10; color: Theme.panel; border.color: Theme.border }

    contentItem: ColumnLayout {
        spacing: 0
        BwTextField {
            id: query
            Layout.fillWidth: true; Layout.margins: 10; Layout.bottomMargin: 6
            placeholderText: qsTr("Search macros")
            Keys.onReturnPressed: {
                const first = root.rows.find(r => r.kind === "macro");
                if (first) root.openMacro(first.entry.id);
            }
            Keys.onEscapePressed: root.close()
        }
        ListView {
            id: list
            Layout.fillWidth: true; Layout.fillHeight: true
            clip: true; boundsBehavior: Flickable.StopAtBounds
            model: root.rows
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Item {
                id: row
                required property var modelData
                readonly property bool isFolder: modelData.kind === "folder"
                readonly property var entry: modelData.entry || ({})
                readonly property bool current: !isFolder && entry.id === root.currentId
                width: list.width; height: isFolder ? 32 : 38
                HoverHandler { id: rowHover }
                Rectangle {
                    anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; radius: 6
                    color: row.current ? "#1d3a55" : rowHover.hovered ? Theme.panelRaised : "transparent"
                }
                RowLayout {
                    anchors.fill: parent; anchors.leftMargin: row.isFolder || !row.entry.folder ? 16 : 34; anchors.rightMargin: 12; spacing: 8
                    LucideIcon {
                        visible: row.isFolder; width: 14; height: 14; color: Theme.textDim
                        name: row.isFolder && row.modelData.open ? "chevron-down" : "chevron-right"
                    }
                    LucideIcon { name: row.isFolder ? "folder" : "blocks"; width: 15; height: 15; color: row.isFolder ? Theme.warning : Theme.textDim }
                    Text {
                        Layout.fillWidth: true; elide: Text.ElideRight; color: Theme.text; font.pixelSize: 13
                        font.weight: row.isFolder || row.current ? Font.DemiBold : Font.Normal
                        text: row.isFolder ? row.modelData.name : (row.entry.name || "Untitled")
                    }
                    Text { visible: row.isFolder; text: row.modelData.count || ""; color: Theme.textDim; font.pixelSize: 11 }
                    Rectangle {
                        visible: !row.isFolder && root.runningIds.indexOf(row.entry.id) >= 0
                        width: 8; height: 8; radius: 4; color: Theme.running
                        ToolTip.visible: runHover.hovered; ToolTip.text: qsTr("Running")
                        HoverHandler { id: runHover }
                    }
                    LucideIcon {
                        visible: !row.isFolder && !!row.entry.restricted
                        name: "lock"; width: 14; height: 14; color: Theme.textDim
                        ToolTip.visible: shieldHover.hovered; ToolTip.text: qsTr("Asks before risky actions (see its Macro settings)")
                        HoverHandler { id: shieldHover }
                    }
                    LucideIcon {
                        visible: !row.isFolder && row.entry.has_triggers && row.entry.always_listen
                        name: "zap"; width: 14; height: 14; color: Theme.accent
                        ToolTip.visible: zapHover.hovered; ToolTip.text: qsTr("Listens for its triggers even when it isn't open")
                        HoverHandler { id: zapHover }
                    }
                    BwButton {
                        id: moreButton
                        visible: rowHover.hovered || rowMenu.visible || folderMenu.visible
                        iconName: "ellipsis"; text: ""; flat: true; implicitWidth: 28; implicitHeight: 28
                        onClicked: (row.isFolder ? folderMenu : rowMenu).popup(moreButton, 0, moreButton.height)
                    }
                }
                TapHandler {
                    enabled: row.isFolder
                    onTapped: root.toggleFolder(row.modelData.name)
                }
                TapHandler {
                    enabled: !row.isFolder
                    acceptedButtons: Qt.LeftButton
                    onTapped: root.openMacro(row.entry.id)
                }
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: (row.isFolder ? folderMenu : rowMenu).popup()
                }
                BwMenu {
                    id: folderMenu
                    implicitWidth: 220
                    BwMenuItem {
                        iconName: "package"; text: qsTr("Export folder as bundle...")
                        onTriggered: {
                            const name = row.modelData.name;
                            root.close();
                            root.bundleRequested(root.entries.filter(e => e.folder === name).map(e => e.id), name);
                        }
                    }
                }
                BwMenu {
                    id: rowMenu
                    implicitWidth: 190
                    BwMenuItem { iconName: "folder-open"; text: qsTr("Open"); onTriggered: root.openMacro(row.entry.id) }
                    BwMenuItem { iconName: "copy"; text: qsTr("Duplicate"); onTriggered: { root.invoke("duplicate_macro", { id: row.entry.id }); root.close(); } }
                    BwMenuItem { iconName: "folder"; text: qsTr("Move to folder..."); onTriggered: folderDialog.openFor(row.entry) }
                    BwMenuItem { iconName: "upload"; text: qsTr("Export..."); onTriggered: { root.close(); root.exportRequested(row.entry.id); } }
                    BwMenuItem { iconName: "package"; text: qsTr("Export as bundle..."); onTriggered: { root.close(); root.bundleRequested([row.entry.id], row.entry.name); } }
                    BwMenuItem { iconName: "trash"; text: qsTr("Delete"); danger: true; onTriggered: { root.close(); root.deleteRequested(row.entry.id, row.entry.name); } }
                }
            }
            Text {
                anchors.centerIn: parent; visible: root.rows.length === 0
                text: root.entries.length ? qsTr("No macros match") : qsTr("No macros yet"); color: Theme.textDim; font.pixelSize: 13
            }
        }
        Rectangle { Layout.fillWidth: true; height: 1; color: Theme.borderSoft }
        RowLayout {
            Layout.fillWidth: true; Layout.margins: 10; spacing: 6
            BwButton { iconName: "plus"; text: qsTr("New"); implicitHeight: 32; onClicked: { root.invoke("new_macro"); root.close(); } }
            BwButton { iconName: "sparkles"; text: qsTr("Template"); implicitHeight: 32; onClicked: { root.close(); root.templateRequested(); } }
            Item { Layout.fillWidth: true }
            BwButton { iconName: "download"; text: qsTr("Import..."); implicitHeight: 32; onClicked: { root.close(); root.importRequested(); } }
        }
    }

    BwDialog {
        id: folderDialog
        property var entry: null
        title: qsTr("Move to folder")
        standardButtons: Dialog.Ok | Dialog.Cancel
        function openFor(e) { entry = e; folderName.text = e.folder || ""; open(); folderName.forceActiveFocus(); folderName.selectAll(); }
        Column {
            width: 340; spacing: 10
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                text: qsTr("Type a new folder name or pick one. Leave it empty to take \"%1\" out of its folder.").arg(folderDialog.entry ? folderDialog.entry.name : "")
            }
            BwTextField { id: folderName; width: parent.width; placeholderText: qsTr("Work"); onAccepted: folderDialog.accept() }
            Flow {
                width: parent.width; spacing: 6
                Repeater {
                    model: root.folders
                    delegate: BwButton {
                        required property string modelData
                        iconName: "folder"; text: modelData; implicitHeight: 28
                        onClicked: { folderName.text = modelData; folderDialog.accept(); }
                    }
                }
            }
        }
        onAccepted: if (entry) root.invoke("set_macro_folder", { id: entry.id, folder: folderName.text })
    }
}
