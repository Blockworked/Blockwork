import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: root
    required property var appState
    required property var invoke

    readonly property var preferences: [
        { title: qsTr("Start at system startup"), detail: qsTr("Hotkeys work as soon as you log in."), on: appState.start_at_startup === true, command: "set_start_at_startup" },
        { title: qsTr("Close to tray"), detail: qsTr("Keep hotkeys active after closing the editor."), on: appState.close_to_tray === true, command: "set_close_to_tray" },
        { title: qsTr("Record mouse movement"), detail: qsTr("Include cursor paths in recordings."), on: appState.record_mouse_movement === true, command: "toggle_record_mouse_movement" }
    ]

    Rectangle { anchors.fill: parent; color: Theme.window }

    component WelcomeSection: Rectangle {
        id: section
        property string title: ""
        default property alias rows: sectionContent.data
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        Layout.minimumWidth: 0
        Layout.alignment: Qt.AlignTop
        implicitHeight: sectionContent.implicitHeight + 32
        radius: 10; color: Theme.panel; border.color: Theme.borderSoft
        ColumnLayout {
            id: sectionContent
            x: 16; y: 16; width: parent.width - 32; spacing: 0
            Text {
                Layout.fillWidth: true; Layout.bottomMargin: 8
                text: section.title; color: Theme.text; font.pixelSize: 16; font.weight: Font.Bold
                wrapMode: Text.WordWrap
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 52; color: Theme.field; border.color: Theme.borderSoft
            TitleBarDrag { anchors.fill: parent; visible: Qt.platform.os !== "osx" }
            RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                Image {
                    visible: Qt.platform.os !== "osx"
                    source: "qrc:/icons/blockwork.png"; sourceSize: Qt.size(40, 40)
                    Layout.preferredWidth: 20; Layout.preferredHeight: 20; Layout.rightMargin: 2
                    fillMode: Image.PreserveAspectFit; smooth: true; mipmap: true
                }
                Text { text: qsTr("Welcome to Blockwork"); color: Theme.text; font.pixelSize: 17; font.weight: Font.Bold }
                Item { Layout.fillWidth: true }
                WindowControls { visible: Qt.platform.os !== "osx" }
            }
        }

        ScrollView {
            id: welcomeScroll
            Layout.fillWidth: true; Layout.fillHeight: true; clip: true
            contentWidth: availableWidth
            WheelPan { flickable: welcomeScroll.contentItem }
            Item {
                width: welcomeScroll.availableWidth
                implicitHeight: pane.implicitHeight + 48
                ColumnLayout {
                    id: pane
                    x: 28; y: 24; width: Math.max(0, parent.width - 56); spacing: 16
                    ColumnLayout {
                        Layout.fillWidth: true; spacing: 6
                        Text {
                            Layout.fillWidth: true; text: qsTr("Make it yours"); color: Theme.text
                            font.pixelSize: 22; font.weight: Font.Bold; wrapMode: Text.WordWrap
                        }
                        Text {
                            Layout.fillWidth: true; text: qsTr("You can change these any time in Settings.")
                            color: Theme.textDim; font.pixelSize: 12; wrapMode: Text.WordWrap
                        }
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columns: width >= 1120 ? 2 : 1
                        columnSpacing: 20; rowSpacing: 20
                        WelcomeSection {
                            title: qsTr("Preferences")
                            Repeater {
                                model: root.preferences
                                delegate: Item {
                                    id: preferenceRow
                                    required property var modelData
                                    required property int index
                                    Layout.fillWidth: true
                                    implicitHeight: Math.max(preferenceText.implicitHeight, preferenceSwitch.implicitHeight) + 28
                                    Rectangle {
                                        width: parent.width; height: 1; color: Theme.borderSoft
                                        visible: preferenceRow.index > 0
                                    }
                                    Column {
                                        id: preferenceText
                                        anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
                                        width: Math.max(0, parent.width - preferenceSwitch.width - 16); spacing: 4
                                        Text {
                                            width: parent.width; text: preferenceRow.modelData.title; color: Theme.text
                                            font.pixelSize: 14; font.weight: Font.DemiBold; wrapMode: Text.WordWrap
                                        }
                                        Text {
                                            width: parent.width; text: preferenceRow.modelData.detail; color: Theme.textDim
                                            font.pixelSize: 12; wrapMode: Text.WordWrap
                                        }
                                    }
                                    BwSwitch {
                                        id: preferenceSwitch
                                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                                        checked: preferenceRow.modelData.on
                                        onToggled: checked => root.invoke(preferenceRow.modelData.command, { enabled: checked })
                                    }
                                }
                            }
                        }
                        WelcomeSection {
                            title: qsTr("Start from a template")
                            Repeater {
                                model: appState.templates || []
                                delegate: ItemDelegate {
                                    id: templateRow
                                    required property var modelData
                                    required property int index
                                    Layout.fillWidth: true
                                    implicitHeight: templateContent.implicitHeight + 28
                                    leftPadding: 0; rightPadding: 0; topPadding: 14; bottomPadding: 14
                                    hoverEnabled: true
                                    Accessible.name: modelData.name
                                    background: Rectangle {
                                        color: templateRow.hovered || templateRow.down ? Theme.hover : "transparent"
                                        radius: 6
                                        Rectangle {
                                            width: parent.width; height: 1; color: Theme.borderSoft
                                            visible: templateRow.index > 0
                                        }
                                    }
                                    contentItem: RowLayout {
                                        id: templateContent
                                        spacing: 12
                                        LucideIcon {
                                            name: templateRow.modelData.icon; color: Theme.textDim
                                            Layout.preferredWidth: 18; Layout.preferredHeight: 18
                                        }
                                        ColumnLayout {
                                            Layout.fillWidth: true; Layout.minimumWidth: 0; spacing: 4
                                            Text {
                                                Layout.fillWidth: true; text: templateRow.modelData.name; color: Theme.text
                                                font.pixelSize: 14; font.weight: Font.DemiBold; wrapMode: Text.WordWrap
                                            }
                                            Text {
                                                Layout.fillWidth: true; text: templateRow.modelData.description; color: Theme.textDim
                                                font.pixelSize: 12; wrapMode: Text.WordWrap
                                            }
                                        }
                                        LucideIcon {
                                            name: "chevron-right"; color: Theme.textDim
                                            Layout.preferredWidth: 16; Layout.preferredHeight: 16
                                        }
                                    }
                                    onClicked: {
                                        root.invoke("new_macro_from_template", { template: modelData.id });
                                        root.invoke("set_onboarding_done", { done: true });
                                    }
                                }
                            }
                        }
                    }
                    BwButton {
                        Layout.alignment: Qt.AlignRight
                        text: qsTr("Get started"); primary: true
                        onClicked: root.invoke("set_onboarding_done", { done: true })
                    }
                }
            }
        }
    }
}
