import QtQuick
import QtQuick.Controls

// The Qt twin of native-ui.c for tests/native_ui_live.rs: the same names,
// roles and failure cases, exposed through Qt's AT-SPI bridge.
ApplicationWindow {
    width: 480; height: 460; visible: true
    title: "Blockwork native UI fixture"
    Component.onCompleted: Qt.application.name = "Blockwork native UI fixture"
    Column {
        anchors.fill: parent; anchors.margins: 12; spacing: 8
        Button { text: "Activate"; Accessible.name: "Activate"; onClicked: status.text = "pressed" }
        TextEdit { id: status; text: "ready"; readOnly: true; Accessible.name: "Status"; Accessible.role: Accessible.StaticText }
        TextField { text: "initial"; Accessible.name: "Input"; width: 300 }
        Slider { from: 0; to: 10; stepSize: 0.1; value: 2; Accessible.name: "Amount" }
        TextField { echoMode: TextInput.Password; text: "protected-fixture-value"; Accessible.name: "Secret"; width: 300 }
        Button { text: "Duplicate"; Accessible.name: "Duplicate" }
        Button { text: "Duplicate"; Accessible.name: "Duplicate" }
        Button { text: "Disabled"; Accessible.name: "Disabled"; enabled: false }
    }
}
