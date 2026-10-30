import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// The signed-in account's packs: each one's review status and versions,
// sharing an update from the open macro, and sharing a new pack.
ColumnLayout {
    id: mine
    required property var browser
    readonly property var host: browser.host
    readonly property var community: host.community
    readonly property var publish: community.tasks.publish || ({ running: false, error: "", message: "", done: 0 })
    property var result: ({ loading: true, packs: [], error: "" })

    function load(refresh) {
        if (!community.signed_in) return;
        host.daemonCall("community_my_packs", { refresh: !!refresh }, result => mine.result = result, () => {});
    }
    readonly property bool signedIn: community.signed_in
    onSignedInChanged: if (signedIn && browser.page === "mine") load(true)
    readonly property int publishDone: publish.done
    onPublishDoneChanged: load(true)

    function statusText(v) {
        return v.status === "approved" ? qsTr("Listed") : v.status === "pending" ? qsTr("Waiting for review") : qsTr("Not listed");
    }

    spacing: 10

    RowLayout {
        Layout.fillWidth: true; spacing: 8
        BwButton { flat: true; iconName: "arrow-left"; text: qsTr("All packs"); onClicked: mine.browser.showList() }
        Item { Layout.fillWidth: true }
        CommunityParts.AccountButton { host: mine.host; browser: mine.browser }
    }

    // Not signed in (signed out from another window, or the token expired).
    ColumnLayout {
        visible: !mine.signedIn
        Layout.fillWidth: true; spacing: 10
        Text {
            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 14
            text: qsTr("Sign in to share packs with the community library and publish updates to them. Your browser opens to sign in with GitHub or Google.")
        }
        BwButton { primary: true; iconName: "user"; text: qsTr("Sign in with your browser"); onClicked: mine.host.signIn() }
    }

    ColumnLayout {
        visible: mine.signedIn
        Layout.fillWidth: true; Layout.fillHeight: true; spacing: 10

        RowLayout {
            Layout.fillWidth: true; spacing: 10
            CommunityParts.Avatar { width: 40; height: 40; source: mine.community.user.avatar; name: mine.community.user.name }
            ColumnLayout {
                Layout.fillWidth: true; spacing: 2
                Text { text: mine.community.user.name; color: Theme.text; font.pixelSize: 16; font.weight: Font.DemiBold }
            }
            BwButton {
                objectName: "shareNewPack"
                primary: true; iconName: "upload"; text: qsTr("Share a new pack...")
                enabled: !mine.publish.running
                onClicked: mine.host.publishNew()
            }
            BwButton {
                iconName: "folder-open"; text: qsTr("Send a pack file...")
                enabled: !mine.publish.running
                onClicked: mine.host.publishFromFile()
            }
        }

        // How the last send went.
        Rectangle {
            visible: mine.publish.running || mine.publish.error !== "" || mine.publish.message !== ""
            Layout.fillWidth: true
            implicitHeight: publishText.implicitHeight + 18; radius: 6
            color: mine.publish.error ? Theme.dangerBackground : mine.publish.running ? Theme.panelRaised : Theme.successBackground
            border.color: mine.publish.error ? Theme.dangerBorder : mine.publish.running ? Theme.borderSoft : Theme.successBorder
            Text {
                id: publishText
                anchors.left: parent.left; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; anchors.margins: 10
                wrapMode: Text.WordWrap; font.pixelSize: 13
                color: mine.publish.error ? Theme.danger : Theme.text
                text: mine.publish.running ? qsTr("Sending your pack...") : mine.publish.error || mine.publish.message
            }
        }
        Text {
            visible: !mine.browser.macro
            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 12
            text: qsTr("Open the macro your blocks are in to share them or publish an update.")
        }
        Text {
            visible: mine.result.error !== ""
            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.danger; font.pixelSize: 12
            text: mine.result.error
        }
        Text {
            visible: mine.result.loading && mine.result.packs.length === 0
            color: Theme.textDim; font.pixelSize: 13
            text: qsTr("Loading your packs...")
        }
        Text {
            visible: !mine.result.loading && !mine.result.error && mine.result.packs.length === 0
            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 13
            text: qsTr("You haven't shared any packs yet. Pick some of this macro's blocks with Share a new pack.")
        }

        ListView {
            id: list
            objectName: "myPacksList"
            Layout.fillWidth: true; Layout.fillHeight: true
            clip: true; spacing: 8
            model: mine.result.packs
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            WheelPan { flickable: list }
            delegate: Rectangle {
                id: card
                required property var modelData
                readonly property var latest: modelData.versions[0] || ({ status: "", reason: "", version: "" })
                readonly property var listed: (mine.browser.library.packs || []).find(p => p.id === modelData.id && p.community && p.key) || null
                property bool showVersions: false
                width: list.width - 12
                implicitHeight: col.implicitHeight + 24
                radius: 8; color: Theme.panelRaised; border.color: Theme.borderSoft
                ColumnLayout {
                    id: col
                    anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
                    anchors.margins: 12; spacing: 6
                    RowLayout {
                        Layout.fillWidth: true; spacing: 8
                        ColumnLayout {
                            Layout.fillWidth: true; spacing: 3
                            RowLayout {
                                spacing: 8
                                Text { text: card.modelData.name; color: Theme.text; font.pixelSize: 15; font.weight: Font.DemiBold }
                                CommunityParts.Pill {
                                    text: mine.statusText(card.latest) + (card.latest.version ? " " + card.latest.version : "")
                                    accent: card.latest.status === "approved"
                                    warn: card.latest.status === "pending"
                                }
                                CommunityParts.Pill { visible: !card.modelData.listed; warn: true; text: qsTr("Taken down") }
                            }
                            CommunityParts.Facts {
                                Layout.fillWidth: true
                                facts: [
                                    qsTr("ID %1").arg(card.modelData.id),
                                    card.modelData.live ? qsTr("listed version %1").arg(card.modelData.live) : qsTr("not listed yet"),
                                    qsTr("last sent %1").arg(mine.browser.date(card.latest.submitted))
                                ]
                            }
                        }
                        Row {
                            spacing: 6
                            Layout.alignment: Qt.AlignTop
                            BwButton {
                                objectName: "publishUpdate"
                                visible: card.modelData.listed
                                primary: true; iconName: "upload"; text: qsTr("Publish update...")
                                enabled: !mine.publish.running && !!mine.browser.macro
                                onClicked: mine.host.publishUpdate(card.modelData)
                            }
                            BwButton {
                                visible: !!card.listed
                                text: qsTr("View"); iconName: "package"
                                onClicked: mine.browser.showPack(card.listed.key)
                            }
                        }
                    }
                    Text {
                        visible: card.latest.status === "rejected" && card.latest.reason !== ""
                        Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.warning; font.pixelSize: 12
                        text: qsTr("Not listed: %1").arg(card.latest.reason)
                    }
                    Text {
                        visible: card.modelData.description !== ""
                        Layout.fillWidth: true; wrapMode: Text.WordWrap; color: Theme.text; font.pixelSize: 13
                        text: card.modelData.description
                    }
                    CommunityParts.LinkText {
                        visible: card.modelData.versions.length > 1
                        text: card.showVersions ? qsTr("Hide earlier uploads") : qsTr("Earlier uploads (%1)").arg(card.modelData.versions.length - 1)
                        onClicked: card.showVersions = !card.showVersions
                    }
                    Repeater {
                        model: card.showVersions ? card.modelData.versions.slice(1) : []
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true; spacing: 12
                            Text { text: modelData.version || modelData.sha256.slice(0, 8); color: Theme.text; font.pixelSize: 12; font.weight: Font.DemiBold }
                            Text { text: mine.statusText(modelData); color: Theme.textDim; font.pixelSize: 12 }
                            Text { text: mine.browser.date(modelData.submitted); color: Theme.textDim; font.pixelSize: 12 }
                            Text {
                                Layout.fillWidth: true; elide: Text.ElideRight
                                visible: modelData.reason !== ""; text: modelData.reason; color: Theme.textDim; font.pixelSize: 12
                            }
                        }
                    }
                }
            }
        }
    }
}
