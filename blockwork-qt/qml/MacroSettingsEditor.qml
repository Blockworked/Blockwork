pragma Translator: "EditorPage"
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

ColumnLayout {
    id: editor
    required property var macro
    required property var invoke
    required property var permissions
    required property var guardedAbilities
    property bool showTitle: true
    property bool androidHost: false
    readonly property var settings: macro && macro.settings ? macro.settings : ({})
    function reset() { tabs.currentIndex = 0; settingsFlick.contentY = 0; }
    function saveConditions(days, from, until) {
        invoke("set_macro_conditions", { onlyInWindow: condWindow.text, power: powerBox.options[powerBox.currentIndex].value,
            activeFrom: from === undefined ? (settings.active_from || "") : from,
            activeUntil: until === undefined ? (settings.active_until || "") : until, activeDays: days === undefined ? (settings.active_days || 0) : days });
    }

    component ConditionTime: StitchButton {
        id: timeButton
        property string timeValue: ""
        property int defaultHour: 9
        signal chosen(int hour, int minute)
        iconName: "clock"
        text: Schedules.timeLabel(timeValue)
        implicitHeight: 34
        onClicked: {
            const time = Schedules.parseTime(timeValue) || { hour: defaultHour, minute: 0 };
            picker.openAt(timeButton, time.hour, time.minute);
        }
        TimePicker {
            id: picker
            objectName: timeButton.objectName + "Picker"
            use12h: Schedules.use12h; amText: Schedules.amText; pmText: Schedules.pmText
            onPicked: (h, m) => timeButton.chosen(h, m)
        }
    }

    component Help: Text {
        width: parent.width
        wrapMode: Text.WordWrap
        color: Theme.textDim
        font.pixelSize: 12
    }
    component Heading: Text {
        color: Theme.text
        font.pixelSize: 14
        font.weight: Font.DemiBold
    }
    component FieldLabel: Text {
        Layout.fillWidth: true
        Layout.preferredWidth: 125
        Layout.minimumWidth: parent.columns === 2 ? 125 : 0
        Layout.maximumWidth: parent.columns === 2 ? 125 : Infinity
        text: ""
        wrapMode: Text.WordWrap
        color: Theme.textDim
        font.pixelSize: 13
    }
    component Form: GridLayout {
        width: parent.width
        columns: width < 440 ? 1 : 2
        columnSpacing: 16
        rowSpacing: 12
    }
    component Divider: Rectangle {
        width: parent.width; height: 1; color: Theme.borderSoft
    }

    spacing: 16
    TabBar {
        id: tabs
        objectName: "macroSettingsTabs"
        Layout.fillWidth: true
        background: Rectangle { color: Theme.panelRaised; radius: 7 }
        Repeater {
            model: [qsTr("General"), qsTr("Triggers"), qsTr("Permissions")]
            delegate: TabButton {
                id: tab
                required property string modelData
                text: modelData
                implicitHeight: 36
                HoverHandler { cursorShape: tab.checked ? Qt.ArrowCursor : Qt.PointingHandCursor }
                contentItem: Text {
                    text: tab.text; color: tab.checked ? Theme.text : Theme.textDim
                    font.pixelSize: 13; font.weight: tab.checked ? Font.DemiBold : Font.Normal
                    horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }
                background: Rectangle {
                    radius: 6; color: tab.checked ? Theme.field : tab.hovered ? Theme.hover : "transparent"
                    border.color: tab.checked ? Theme.border : "transparent"
                }
            }
        }
        onCurrentIndexChanged: settingsFlick.contentY = 0
    }
    Flickable {
        id: settingsFlick
        objectName: "macroSettingsFlick"
        Layout.fillWidth: true; Layout.fillHeight: true
        contentHeight: settingsColumn.implicitHeight
        clip: true; boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        WheelPan { flickable: settingsFlick }
        Column {
            id: settingsColumn
            width: settingsFlick.width - 16
            spacing: 0
            Column {
                width: parent.width; spacing: 16; visible: tabs.currentIndex === 0
                Form {
                    visible: editor.showTitle
                    FieldLabel { text: qsTr("Title") }
                    StitchTextField {
                        objectName: "macroTitle"
                        Layout.fillWidth: true; Layout.minimumWidth: 0
                        Accessible.name: qsTr("Title")
                        text: editor.macro ? editor.macro.name : ""
                        onEditingFinished: editor.invoke("set_title", { title: text })
                    }
                }
                Divider { visible: editor.showTitle }
                Heading { text: qsTr("Playback") }
                Column {
                    width: parent.width; spacing: 6
                    Help { text: qsTr("Speed") }
                    RowLayout { width: parent.width; spacing: 10
                        StitchSlider {
                            id: speed; objectName: "macroSpeedSlider"; from: .1; to: 10; value: savedSpeed; Layout.fillWidth: true; implicitHeight: 30
                            Accessible.name: qsTr("Playback speed")
                            readonly property real savedSpeed: Math.max(from, Math.min(to, editor.macro ? (editor.macro.speed_multiplier ?? 1) : 1))
                            onSavedSpeedChanged: value = savedSpeed
                            onMoved: editor.invoke("set_macro_speed_multiplier", { multiplier: value })
                        }
                        StitchTextField { Accessible.name: qsTr("Playback speed multiplier"); text: Number(speed.value).toFixed(2); Layout.preferredWidth: 76; horizontalAlignment: Text.AlignRight; onEditingFinished: editor.invoke("set_macro_speed_multiplier", { multiplier: Number(text) }) }
                        Text { text: "×"; color: Theme.textDim }
                    }
                }
                Form {
                    FieldLabel { text: qsTr("Time limit") }
                    RowLayout {
                        Layout.fillWidth: true; spacing: 8
                        StitchTextField {
                            objectName: "runLimit"
                            Layout.preferredWidth: 90
                            Accessible.name: qsTr("Time limit in seconds")
                            horizontalAlignment: Text.AlignRight
                            validator: IntValidator { bottom: 0; top: 86400 }
                            text: String(editor.settings.max_run_secs || 0)
                            onEditingFinished: editor.invoke("set_macro_max_run_secs", { seconds: Number(text) || 0 })
                        }
                        Text { text: qsTr("seconds"); color: Theme.textDim; font.pixelSize: 13 }
                        Item { Layout.fillWidth: true }
                    }
                    FieldLabel { text: qsTr("Loop limit") }
                    RowLayout {
                        Layout.fillWidth: true; spacing: 8
                        StitchTextField {
                            objectName: "loopLimit"
                            Layout.preferredWidth: 90
                            Accessible.name: qsTr("Loop limit in repeats")
                            horizontalAlignment: Text.AlignRight
                            validator: IntValidator { bottom: 0; top: 2000000000 }
                            text: String(editor.settings.max_loop_iterations || 0)
                            onEditingFinished: editor.invoke("set_macro_max_loop_iterations", { iterations: Number(text) || 0 })
                        }
                        Text { text: qsTr("repeats"); color: Theme.textDim; font.pixelSize: 13 }
                        Item { Layout.fillWidth: true }
                    }
                }
                Help { text: qsTr("Limits stop a run or loop after the amount above. Set either to 0 for no limit.") }
            }
            Column {
                width: parent.width; spacing: 12; visible: tabs.currentIndex === 1
                Column {
                    width: parent.width; spacing: 4
                    StitchSwitch {
                        objectName: "alwaysListen"
                        Accessible.name: qsTr("Listen for triggers")
                        checked: !!editor.settings.always_listen
                        onToggled: checked => editor.invoke("set_macro_always_listen", { enabled: checked })
                    }
                    Help { text: qsTr("Keep this macro's triggers active while Blockwork is open.") }
                }
                Divider {}
                Heading { text: qsTr("Conditions") }
                Help { text: qsTr("Triggers and hotkeys only start this macro when all of these hold. The Run button ignores them.") }
                Form {
                    FieldLabel { text: qsTr("Focused window") }
                    StitchTextField {
                        id: condWindow; Layout.fillWidth: true; Layout.minimumWidth: 0
                        Accessible.name: qsTr("Focused window")
                        placeholderText: qsTr("Any window")
                        text: editor.settings.only_in_window || ""
                        onEditingFinished: editor.saveConditions()
                    }
                    FieldLabel { text: qsTr("Power source") }
                    StitchComboBox {
                        id: powerBox; Layout.fillWidth: true; Layout.minimumWidth: 0; textRole: "text"
                        Accessible.name: qsTr("Power source")
                        readonly property var options: [{ value: "Any", text: qsTr("Battery or AC power") }, { value: "PluggedIn", text: qsTr("AC power only") }, { value: "OnBattery", text: qsTr("Battery only") }]
                        model: options
                        currentIndex: Math.max(0, options.findIndex(o => o.value === editor.settings.power))
                        onActivated: editor.saveConditions()
                    }
                    FieldLabel { text: qsTr("Time of day") }
                    ColumnLayout {
                        Layout.fillWidth: true; spacing: 8
                        StitchComboBox {
                            id: timeMode
                            objectName: "conditionTimeMode"
                            Layout.fillWidth: true; Layout.minimumWidth: 0
                            Accessible.name: qsTr("Time of day")
                            model: [qsTr("Any time"), qsTr("Time range")]
                            currentIndex: editor.settings.active_from || editor.settings.active_until ? 1 : 0
                            onActivated: index => {
                                if (index === 0) editor.saveConditions(undefined, "", "");
                                else editor.saveConditions(undefined, editor.settings.active_from || "09:00", editor.settings.active_until || "17:00");
                            }
                        }
                        RowLayout {
                            visible: timeMode.currentIndex === 1
                            spacing: 8
                            ConditionTime {
                                objectName: "conditionStart"
                                Accessible.name: qsTr("Start time")
                                timeValue: editor.settings.active_from || "09:00"
                                defaultHour: 9
                                onChosen: (h, m) => editor.saveConditions(undefined, Schedules.stamp(h, m), editor.settings.active_until || "17:00")
                            }
                            Text { text: qsTr("to"); color: Theme.textDim; font.pixelSize: 13 }
                            ConditionTime {
                                objectName: "conditionEnd"
                                Accessible.name: qsTr("End time")
                                timeValue: editor.settings.active_until || "17:00"
                                defaultHour: 17
                                onChosen: (h, m) => editor.saveConditions(undefined, editor.settings.active_from || "09:00", Schedules.stamp(h, m))
                            }
                        }

                    }
                    FieldLabel { text: qsTr("Days") }
                    Row {
                        spacing: 3
                        Repeater {
                            model: [qsTr("Su"), qsTr("Mo"), qsTr("Tu"), qsTr("We"), qsTr("Th"), qsTr("Fr"), qsTr("Sa")]
                            delegate: StitchButton {
                                required property string modelData
                                required property int index
                                readonly property int days: editor.settings.active_days || 0
                                readonly property bool selected: days === 0 || (days & (1 << index)) !== 0
                                implicitWidth: 31; implicitHeight: 32; leftPadding: 0; rightPadding: 0
                                text: modelData; checkable: true; checked: selected; primary: selected
                                Accessible.name: [qsTr("Sunday"), qsTr("Monday"), qsTr("Tuesday"), qsTr("Wednesday"), qsTr("Thursday"), qsTr("Friday"), qsTr("Saturday")][index]
                                onClicked: {
                                    const next = (days === 0 ? 0x7f : days) ^ (1 << index);
                                    if (next !== 0) editor.saveConditions(next === 0x7f ? 0 : next);
                                    checked = Qt.binding(() => selected);
                                }
                            }
                        }
                    }
                }
                Divider {}
                Column {
                    width: parent.width; spacing: 4
                    StitchSwitch {
                        objectName: "catchUp"
                        Accessible.name: qsTr("Catch up on missed schedules")
                        checked: !!editor.settings.catch_up_missed
                        onToggled: checked => editor.invoke("set_macro_catch_up", { enabled: checked })
                    }
                    Help { text: qsTr("Runs a schedule or \"once at\" trigger once if its time passed while Blockwork was closed or the computer was asleep (up to a week back).") }
                    StitchSwitch {
                        visible: editor.androidHost
                        Accessible.name: qsTr("Exact time on Android")
                        checked: !!editor.settings.android_exact_time
                        onToggled: checked => editor.invoke("set_macro_android_exact_time", {enabled: checked})
                    }
                    Help { visible: editor.androidHost; text: qsTr("Exact schedules require Android exact-alarm access. Without access they stay blocked. Best-effort alarms may be delayed.") }
                }
            }
            Column {
                width: parent.width; spacing: 12; visible: tabs.currentIndex === 2
                StitchSwitch {
                    Accessible.name: qsTr("Ask before risky actions")
                    checked: editor.permissions !== null
                    onToggled: checked => editor.invoke("set_macro_restricted", { restricted: checked })
                }
                Text {
                    width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                    text: editor.permissions === null
                        ? qsTr("This macro may do anything its blocks say. Imported macros ask first until you allow them.")
                        : qsTr("Ticked abilities are allowed. The rest show a dialog the first time the macro tries them.")
                }
                Repeater {
                    model: editor.permissions === null ? [] : editor.guardedAbilities
                    delegate: StitchCheckBox {
                        id: ability
                        required property var modelData
                        width: parent.width
                        implicitHeight: Math.max(28, contentItem.implicitHeight + topPadding + bottomPadding)
                        text: modelData.label
                        Accessible.name: text
                        contentItem: Text {
                            leftPadding: ability.indicator.width + ability.spacing
                            text: ability.text; font: ability.font; color: Theme.textDim
                            wrapMode: Text.WordWrap; verticalAlignment: Text.AlignVCenter
                        }
                        checked: editor.permissions.indexOf(modelData.kind) >= 0
                        onToggled: editor.invoke("set_macro_permission", { kind: modelData.kind, allowed: checked })
                    }
                }
            }
        }
    }
}
