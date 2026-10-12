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
        { title: "Start at system startup", detail: "Hotkeys work as soon as you log in.", on: appState.start_at_startup === true, command: "set_start_at_startup" },
        { title: "Close to tray", detail: "Keep hotkeys active after closing the editor.", on: appState.close_to_tray === true, command: "set_close_to_tray" },
        { title: "Record mouse movement", detail: "Include cursor paths in recordings.", on: appState.record_mouse_movement === true, command: "toggle_record_mouse_movement" }
    ]

    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 56; color: "#292a2d"; border.color: "#3a3b3f"
            TitleBarDrag { anchors.fill: parent; visible: Qt.platform.os !== "osx" }
            RowLayout { anchors.fill: parent; anchors.margins: 14
                Image {
                    visible: Qt.platform.os !== "osx"
                    source: "qrc:/icons/blockwork.png"; sourceSize: Qt.size(40, 40)
                    Layout.preferredWidth: 20; Layout.preferredHeight: 20
                    fillMode: Image.PreserveAspectFit; smooth: true; mipmap: true
                }
                Text { text: "Welcome to Blockwork"; color: "#e7e7e8"; font.pixelSize: 15; font.weight: Font.Bold }
                Item { Layout.fillWidth: true }
                WindowControls { visible: Qt.platform.os !== "osx" }
            }
        }

        Item {
            Layout.fillWidth: true; Layout.fillHeight: true
            ColumnLayout {
                anchors.centerIn: parent; width: Math.min(560, parent.width - 48); spacing: 14
                Text { Layout.alignment: Qt.AlignHCenter; text: "Make it yours"; color: "#e7e7e8"; font.pixelSize: 24; font.weight: Font.DemiBold }
                Text { Layout.alignment: Qt.AlignHCenter; Layout.bottomMargin: 6; text: "You can change these any time in Settings."; color: "#9fa0a6"; font.pixelSize: 14 }
                SectionCard {
                    Layout.fillWidth: true; title: "Preferences"; iconName: "settings"
                    Repeater {
                        model: root.preferences
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true; spacing: 12
                            ColumnLayout { Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: modelData.title; color: "#e7e7e8"; font.pixelSize: 14; font.weight: Font.DemiBold }
                                Text { Layout.fillWidth: true; text: modelData.detail; color: "#9fa0a6"; font.pixelSize: 12 }
                            }
                            BwSwitch {
                                checked: modelData.on
                                onToggled: checked => root.invoke(modelData.command, { enabled: checked })
                            }
                        }
                    }
                }
                BwButton {
                    Layout.alignment: Qt.AlignHCenter; Layout.topMargin: 10
                    text: "Get started"; primary: true
                    onClicked: root.invoke("set_onboarding_done", { done: true })
                }
            }
        }
    }
}
