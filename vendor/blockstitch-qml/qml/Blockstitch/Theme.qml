pragma Singleton
import QtQuick

QtObject {
    // Empty follows the desktop, including changes made while the app is open.
    property bool touchMode: Qt.platform.os === "android"
    readonly property int minimumTarget: touchMode ? 48 : 0
    property string mode: ""
    readonly property bool dark: mode === "dark" || (mode !== "light" && Qt.styleHints.colorScheme === Qt.Dark)
    readonly property color window: dark ? "#1d1e20" : "#f3f4f6"
    readonly property color panel: dark ? "#242527" : "#ffffff"
    readonly property color panelRaised: dark ? "#303134" : "#e9ecf0"
    readonly property color canvas: dark ? "#27282b" : "#f5f6f8"
    readonly property color field: dark ? "#292a2d" : "#f0f2f5"
    readonly property color border: dark ? "#4a4b50" : "#aeb5c0"
    readonly property color borderSoft: dark ? "#36373a" : "#d7dce3"
    readonly property color text: dark ? "#dedee0" : "#202631"
    readonly property color textDim: dark ? "#96979c" : "#616b79"
    readonly property color accent: dark ? "#1597ff" : "#006fc9"
    readonly property color accentHover: dark ? "#32a5ff" : "#005eae"
    readonly property color danger: dark ? "#ff4848" : "#c62828"
    readonly property color warning: dark ? "#f5b942" : "#8a5900"
    readonly property color running: dark ? "#3ccf6e" : "#18743b"
    readonly property color block: dark ? "#3b3c40" : "#e1e5eb"
    readonly property color blockTop: dark ? "#44454a" : "#d5dbe4"
    readonly property color menu: panelRaised
    readonly property color hover: dark ? "#3b3c40" : "#dbe1e9"
    readonly property color selection: dark ? "#1d3a55" : "#d9eaff"
    readonly property color grid: dark ? "#46474d" : "#c7cdd6"
    readonly property color warningBackground: dark ? "#33291a" : "#fff3d6"
    readonly property color warningBorder: dark ? "#5a4220" : "#d0aa53"
    readonly property color successBackground: dark ? "#1c2a21" : "#e5f5e9"
    readonly property color successBorder: dark ? "#2b4a35" : "#89bd98"
    readonly property color dangerBackground: dark ? "#4b2225" : "#fde8e8"
    readonly property color dangerBorder: dark ? "#5c2a2c" : "#dda2a2"
    readonly property int radius: 6
    readonly property int spacing: 12
}
