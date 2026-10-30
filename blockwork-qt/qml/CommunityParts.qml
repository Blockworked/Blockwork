import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// Small pieces the Block library's pages share.
QtObject {
    // A short label in a rounded box.
    component Pill: Rectangle {
        property alias text: label.text
        property bool accent: false
        property bool warn: false
        implicitWidth: label.implicitWidth + 14; implicitHeight: 20; radius: 10
        color: accent ? Theme.selection : warn ? Theme.warningBackground : "transparent"
        border.color: accent ? Theme.accent : warn ? Theme.warningBorder : Theme.border
        Text {
            id: label; anchors.centerIn: parent
            color: parent.accent ? Theme.accent : parent.warn ? Theme.warning : Theme.textDim
            font.pixelSize: 11
        }
    }

    // A word that does something when clicked, like a link on a page.
    component LinkText: Text {
        signal clicked()
        color: Theme.accent; font.pixelSize: 12
        font.underline: linkHover.hovered
        HoverHandler { id: linkHover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: parent.clicked() }
        Accessible.role: Accessible.Link
    }

    // The team's check mark, as the community site shows it.
    component VerifiedBadge: Rectangle {
        implicitWidth: 13; implicitHeight: 13; radius: 6.5; color: Theme.accent
        Accessible.role: Accessible.Graphic; Accessible.name: tip.text
        LucideIcon { anchors.centerIn: parent; width: 9; height: 9; strokeWidth: 3.5; name: "check"; color: "#ffffff" }
        HoverHandler { id: badgeHover }
        ToolTip { id: tip; visible: badgeHover.hovered; delay: 300; text: qsTr("Verified official pack") }
    }

    // "by Author" and a few facts, spaced apart rather than joined.
    component Facts: Flow {
        id: facts
        property string author: ""
        property bool official: false
        property var facts: []
        property int pixelSize: 12
        signal authorClicked()
        spacing: 18
        Row {
            visible: facts.author !== ""
            spacing: 4
            Text { text: qsTr("by %1").arg(facts.author); color: Theme.textDim; font.pixelSize: facts.pixelSize; anchors.verticalCenter: parent.verticalCenter }
            VerifiedBadge { visible: facts.official; anchors.verticalCenter: parent.verticalCenter }
        }
        Repeater {
            model: facts.facts
            delegate: Text { required property string modelData; text: modelData; color: Theme.textDim; font.pixelSize: facts.pixelSize }
        }
    }

    // What a pack's blocks can do: pills on a card, or a list on its page.
    component Capabilities: Flow {
        id: caps
        property var capabilities: []
        property var macroFiles: null
        property bool compact: false
        spacing: compact ? 6 : 4
        function label(cap) { return macroFiles ? macroFiles.capabilityLabel(cap) : cap.label; }
        Repeater {
            model: caps.compact ? caps.capabilities : []
            delegate: Rectangle {
                required property var modelData
                implicitWidth: capRow.implicitWidth + 14; implicitHeight: 22; radius: 11
                color: modelData.risky ? Theme.warningBackground : "transparent"
                border.color: modelData.risky ? Theme.warningBorder : Theme.borderSoft
                Row {
                    id: capRow; anchors.centerIn: parent; spacing: 5
                    LucideIcon { width: 12; height: 12; anchors.verticalCenter: parent.verticalCenter; name: modelData.risky ? "triangle-alert" : "check"; color: modelData.risky ? Theme.warning : Theme.textDim }
                    Text { text: caps.label(modelData); color: modelData.risky ? Theme.warning : Theme.textDim; font.pixelSize: 11; anchors.verticalCenter: parent.verticalCenter }
                }
            }
        }
        Repeater {
            model: caps.compact ? [] : caps.capabilities
            delegate: Row {
                required property var modelData
                width: caps.width; spacing: 8
                LucideIcon {
                    anchors.verticalCenter: parent.verticalCenter; width: 16; height: 16
                    name: modelData.risky ? "triangle-alert" : "check"; color: modelData.risky ? Theme.warning : Theme.textDim
                }
                Text {
                    width: parent.width - 24; wrapMode: Text.WordWrap; anchors.verticalCenter: parent.verticalCenter
                    text: caps.label(modelData); color: modelData.risky ? Theme.warning : Theme.text; font.pixelSize: 13
                }
            }
        }
        Text {
            visible: !caps.compact && caps.capabilities.length === 0
            width: caps.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 13
            text: qsTr("Its blocks only use blocks that stay inside Blockwork, like waits and variables.")
        }
    }

    // A person's picture, or their first letter.
    component Avatar: Rectangle {
        property string source: ""
        property string name: ""
        implicitWidth: 28; implicitHeight: 28; radius: width / 2
        color: Theme.panelRaised; border.color: Theme.borderSoft; clip: true
        Text {
            visible: picture.status !== Image.Ready
            anchors.centerIn: parent; color: Theme.textDim; font.pixelSize: parent.height * 0.45; font.weight: Font.DemiBold
            text: (parent.name || "?").slice(0, 1).toUpperCase()
        }
        Image {
            id: picture
            anchors.fill: parent; source: parent.source; fillMode: Image.PreserveAspectCrop
            sourceSize.width: 96; sourceSize.height: 96; asynchronous: true; smooth: true; mipmap: true
            visible: status === Image.Ready
            layer.enabled: visible
        }
    }

    // A row of tabs with a line under the current one.
    component Tabs: Row {
        id: tabs
        property var tabs: []
        property string current: ""
        signal picked(string key)
        spacing: 4
        Repeater {
            model: tabs.tabs
            delegate: Item {
                required property var modelData
                readonly property bool active: modelData.key === tabs.current
                implicitWidth: tabLabel.implicitWidth + 24; implicitHeight: 34
                Text {
                    id: tabLabel; anchors.centerIn: parent
                    text: modelData.label; font.pixelSize: 13; font.weight: active ? Font.DemiBold : Font.Normal
                    color: active ? Theme.text : tabHover.hovered ? Theme.text : Theme.textDim
                }
                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 2; radius: 1; color: Theme.accent; visible: active }
                HoverHandler { id: tabHover; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: tabs.picked(modelData.key) }
                Accessible.role: Accessible.PageTab; Accessible.name: modelData.label
            }
        }
    }

    // Sign in, or the signed-in account with its menu.
    component AccountButton: BwButton {
        id: account
        required property var host
        required property var browser
        readonly property bool signedIn: host.community.signed_in
        readonly property bool signingIn: !!(host.community.tasks.sign_in || {}).running
        objectName: "libraryAccountButton"
        iconName: "user"
        // The account's picture, when it has one.
        iconSource: signedIn ? host.community.user.avatar : ""
        text: signedIn ? host.community.user.name : signingIn ? qsTr("Signing in...") : qsTr("Sign in")
        onClicked: signedIn ? accountMenu.popup(account, account.width - accountMenu.width, account.height + 4) : host.signIn()
        BwMenu {
            id: accountMenu
            BwMenuItem { text: qsTr("My packs"); iconName: "package"; onTriggered: account.browser.showMine() }
            BwMenuItem {
                text: qsTr("My page on the website"); iconName: "external-link"
                visible: account.host.community.user.page !== ""
                onTriggered: Qt.openUrlExternally(account.host.community.user.page)
            }
            BwMenuItem { text: qsTr("Sign out"); iconName: "log-out"; onTriggered: account.host.invoke("community_sign_out") }
        }
    }
}
