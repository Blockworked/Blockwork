import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

BwDialog {
    id: root
    title: "Choose an App"
    width: 640
    height: Math.min(620, parent ? parent.height - 32 : 620)
    standardButtons: Dialog.NoButton

    required property var bridge
    signal selected(var app)

    property string searchText: ""
    // Set when a fetch takes longer than expected so the dialog stops
    // looking merely "loading" and offers a retry instead.
    property bool timedOut: false
    // Null-safe, always-boolean views of the bridge state. (Bare
    // `bridge && bridge.installedAppsLoading` chains evaluate to `undefined`
    // when bridge isn't set yet, which warns on assignment to bool.)
    readonly property bool appsLoading: !!(bridge && bridge.installedAppsLoading)
    readonly property string appsError: bridge ? bridge.installedAppsError : ""
    // Re-parsed automatically whenever bridge.installedAppsJson changes;
    // no Connections block needed (it produced "no signal matches" warnings).
    property var allApps: {
        try {
            return JSON.parse(bridge ? bridge.installedAppsJson : "[]") || [];
        } catch (e) {
            return [];
        }
    }
    property var filteredApps: {
        const q = searchText.trim().toLowerCase();
        if (!q) return allApps;
        return allApps.filter(a => (a.name || "").toLowerCase().indexOf(q) >= 0);
    }

    function refreshApps() {
        searchText = "";
        searchField.text = "";
        timedOut = false;
        if (!bridge) {
            console.warn("AppSelectorDialog opened without a bridge - app picker cannot load");
        } else if (typeof bridge.refreshInstalledApps === "function") {
            bridge.refreshInstalledApps();
        } else {
            console.warn("AppBridge.refreshInstalledApps is missing - running a stale blockwork binary?");
        }
    }

    function choose(app) {
        selected(app);
        root.close();
    }

    onOpened: refreshApps()

    // If the daemon hasn't answered after 15s, say so and offer a retry
    // instead of spinning forever.
    Timer {
        interval: 15000
        running: root.opened && root.appsLoading
        onTriggered: root.timedOut = true
    }

    contentItem: ColumnLayout {
        spacing: 10

        BwTextField {
            id: searchField
            Layout.fillWidth: true
            placeholderText: "Search apps…"
            onTextEdited: root.searchText = text
            Component.onCompleted: forceActiveFocus()
        }

        Text {
            visible: root.appsLoading
            text: "Loading installed apps…"
            color: Theme.textDim
            font.pixelSize: 13
        }
        Text {
            visible: !root.appsLoading && root.appsError.length > 0
            text: root.appsError
            color: Theme.danger
            font.pixelSize: 12
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
        Text {
            visible: root.timedOut && root.appsLoading
            text: "Taking longer than expected — the background service may be busy. Check the console for [blockwork-qt] lines, then retry."
            color: Theme.warning
            font.pixelSize: 12
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
        Text {
            visible: !root.appsLoading && root.appsError.length === 0 && root.filteredApps.length === 0
            text: "No apps found."
            color: Theme.textDim
            font.pixelSize: 13
        }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: 380
            clip: true
            ScrollBar.vertical.policy: ScrollBar.AsNeeded
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            Flow {
                width: parent.width
                spacing: 8
                Repeater {
                    model: root.filteredApps
                    delegate: Rectangle {
                        required property var modelData
                        required property int index
                        width: 144
                        height: 104
                        radius: Theme.radius
                        color: hover.hovered ? "#3b3c40" : Theme.panelRaised
                        border.color: hover.hovered ? Theme.accent : Theme.border
                        Column {
                            anchors.centerIn: parent
                            spacing: 6
                            width: parent.width - 16
                            Image {
                                visible: !!modelData.icon
                                source: modelData.icon || ""
                                width: 32
                                height: 32
                                anchors.horizontalCenter: parent.horizontalCenter
                                fillMode: Image.PreserveAspectFit
                                smooth: true
                            }
                            LucideIcon {
                                visible: !modelData.icon
                                name: "app-window"
                                color: Theme.textDim
                                width: 28
                                height: 28
                                anchors.horizontalCenter: parent.horizontalCenter
                            }
                            Text {
                                text: modelData.name || ""
                                color: Theme.text
                                font.pixelSize: 12
                                width: parent.width
                                horizontalAlignment: Text.AlignHCenter
                                elide: Text.ElideRight
                                maximumLineCount: 2
                                wrapMode: Text.Wrap
                            }
                        }
                        HoverHandler {
                            id: hover
                            cursorShape: Qt.PointingHandCursor
                        }
                        TapHandler {
                            onTapped: root.choose(modelData)
                        }
                        Accessible.name: modelData.name || "app"
                        Accessible.role: Accessible.Button
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            BwButton {
                visible: root.timedOut || root.appsError.length > 0
                text: "Retry"
                onClicked: root.refreshApps()
            }
            BwButton { text: "Cancel"; onClicked: root.reject() }
        }
    }
}
