import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// The current macro's settings: title, speed, run conditions, permissions.
AppDialog {
    id: macroSettings; title: qsTr("Macro settings"); standardButtons: Dialog.Close
    // The EditorPage this dialog edits: its macro and invoke().
    required property Item page
    readonly property var settings: page.macro && page.macro.settings ? page.macro.settings : ({})
    function saveConditions(days) {
        page.invoke("set_macro_conditions", { onlyInWindow: condWindow.text, power: powerBox.options[powerBox.currentIndex].value,
            activeFrom: condFrom.text, activeUntil: condUntil.text, activeDays: days === undefined ? (settings.active_days || 0) : days });
    }
    Flickable {
    id: macroSettingsFlick
    implicitWidth: 380; implicitHeight: Math.min(settingsColumn.implicitHeight, page.height - 190)
    WheelPan { flickable: macroSettingsFlick }
    contentHeight: settingsColumn.implicitHeight; clip: true; boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
    Column { id: settingsColumn; spacing: 14; width: 360
        Text { text: qsTr("Controls behavior specific to this macro."); color: Theme.textDim }
        Text { text: qsTr("Title"); color: Theme.textDim; font.pixelSize: 13 }
        BwTextField { width: parent.width; text: page.macro ? page.macro.name : ""; onEditingFinished: page.invoke("set_title", { title: text }) }
        Text { text: qsTr("Speed"); color: Theme.textDim; font.pixelSize: 13 }
        RowLayout { width: parent.width; spacing: 10
            Slider {
                id: speed; from: .1; to: 10; value: savedSpeed; Layout.fillWidth: true; implicitHeight: 30
                Accessible.name: qsTr("Playback speed")
                readonly property real savedSpeed: Math.max(from, Math.min(to, page.macro ? (page.macro.speed_multiplier ?? 1) : 1))
                property bool readyForEdits: false
                Component.onCompleted: readyForEdits = true
                onValueChanged: {
                    if (readyForEdits && page.macro && value !== savedSpeed)
                        page.invoke("set_macro_speed_multiplier", { multiplier: value });
                }
                HoverHandler { cursorShape: Qt.PointingHandCursor }
                background: Rectangle { x: speed.leftPadding; y: speed.topPadding + speed.availableHeight / 2 - height / 2; width: speed.availableWidth; height: 5; radius: 3; color: Theme.border; Rectangle { width: speed.visualPosition * parent.width; height: parent.height; radius: 3; color: Theme.accent } }
                handle: Rectangle { x: speed.leftPadding + speed.visualPosition * (speed.availableWidth - width); y: speed.topPadding + speed.availableHeight / 2 - height / 2; width: 16; height: 16; radius: 8; color: "white"; border.width: 2; border.color: Theme.accent }
            }
            BwTextField { text: Number(speed.value).toFixed(2); Layout.preferredWidth: 62; horizontalAlignment: Text.AlignRight; onEditingFinished: page.invoke("set_macro_speed_multiplier", { multiplier: Number(text) }) }
            Text { text: "×"; color: Theme.textDim }
        }
        BwSwitch { Accessible.name: qsTr("Listen for triggers while Blockwork is open"); checked: page.macro && page.macro.settings ? page.macro.settings.always_listen : false; onToggled: checked => page.invoke("set_macro_always_listen", { enabled: checked }) }
        Text { text: qsTr("Time limit"); color: Theme.textDim; font.pixelSize: 13 }
        RowLayout { width: parent.width; spacing: 10
            Text { text: qsTr("Stop a run after"); color: Theme.text; font.pixelSize: 13 }
            BwTextField {
                Layout.preferredWidth: 70; horizontalAlignment: Text.AlignRight
                validator: IntValidator { bottom: 0; top: 86400 }
                text: page.macro && page.macro.settings ? String(page.macro.settings.max_run_secs) : "0"
                onEditingFinished: page.invoke("set_macro_max_run_secs", { seconds: Number(text) || 0 })
            }
            Text { Layout.fillWidth: true; text: qsTr("seconds (0 = no limit)"); color: Theme.textDim; font.pixelSize: 13 }
        }
        Text { text: qsTr("Loop limit"); color: Theme.textDim; font.pixelSize: 13 }
        RowLayout { width: parent.width; spacing: 10
            Text { text: qsTr("Stop a loop after"); color: Theme.text; font.pixelSize: 13 }
            BwTextField {
                Layout.preferredWidth: 90; horizontalAlignment: Text.AlignRight
                validator: IntValidator { bottom: 0; top: 2000000000 }
                text: page.macro && page.macro.settings ? String(page.macro.settings.max_loop_iterations || 0) : "0"
                onEditingFinished: page.invoke("set_macro_max_loop_iterations", { iterations: Number(text) || 0 })
            }
            Text { Layout.fillWidth: true; text: qsTr("repeats (0 = no limit)"); color: Theme.textDim; font.pixelSize: 13 }
        }
        Text { text: qsTr("Conditions"); color: Theme.textDim; font.pixelSize: 13 }
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Triggers and hotkeys only start this macro when all of these hold. The Run button ignores them.")
        }
        RowLayout { width: parent.width; spacing: 8
            Text { text: qsTr("While"); color: Theme.text; font.pixelSize: 13 }
            BwTextField {
                id: condWindow; Layout.fillWidth: true; placeholderText: qsTr("any window")
                text: macroSettings.settings.only_in_window || ""
                onEditingFinished: macroSettings.saveConditions()
            }
            Text { text: qsTr("is focused"); color: Theme.text; font.pixelSize: 13 }
        }
        RowLayout { width: parent.width; spacing: 8
            Text { text: qsTr("On"); color: Theme.text; font.pixelSize: 13 }
            BwComboBox {
                id: powerBox; Layout.fillWidth: true; textRole: "text"
                readonly property var options: [{ value: "Any", text: qsTr("battery or AC power") }, { value: "PluggedIn", text: qsTr("AC power only") }, { value: "OnBattery", text: qsTr("battery only") }]
                model: options
                currentIndex: Math.max(0, options.findIndex(o => o.value === macroSettings.settings.power))
                onActivated: macroSettings.saveConditions()
            }
        }
        RowLayout { width: parent.width; spacing: 8
            Text { text: qsTr("Between"); color: Theme.text; font.pixelSize: 13 }
            BwTextField {
                id: condFrom; Layout.preferredWidth: 70; horizontalAlignment: Text.AlignHCenter; placeholderText: "09:00"
                text: macroSettings.settings.active_from || ""
                onEditingFinished: if (!!text === !!condUntil.text) macroSettings.saveConditions()
            }
            Text { text: qsTr("and"); color: Theme.text; font.pixelSize: 13 }
            BwTextField {
                id: condUntil; Layout.preferredWidth: 70; horizontalAlignment: Text.AlignHCenter; placeholderText: "17:00"
                text: macroSettings.settings.active_until || ""
                onEditingFinished: if (!!text === !!condFrom.text) macroSettings.saveConditions()
            }
            Text { Layout.fillWidth: true; text: qsTr("(empty = all day)"); color: Theme.textDim; font.pixelSize: 12 }
        }
        Row { spacing: 6
            Text { text: qsTr("On"); color: Theme.text; font.pixelSize: 13; anchors.verticalCenter: parent.verticalCenter; rightPadding: 2 }
            Repeater {
                model: ["S", "M", "T", "W", "T", "F", "S"]
                delegate: Rectangle {
                    required property string modelData
                    required property int index
                    readonly property int days: macroSettings.settings.active_days || 0
                    // No days picked means every day.
                    readonly property bool on: days === 0 || (days & (1 << index)) !== 0
                    width: 30; height: 30; radius: 15
                    color: on ? Theme.accent : Theme.panelRaised; border.color: Theme.border
                    Accessible.name: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][index]
                    Text { anchors.centerIn: parent; text: parent.modelData; color: parent.on ? "white" : Theme.textDim; font.pixelSize: 12; font.weight: Font.DemiBold }
                    MouseArea {
                        anchors.fill: parent; cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            const next = (parent.days === 0 ? 0x7f : parent.days) ^ (1 << parent.index);
                            if (next === 0) return; // keep at least one day
                            macroSettings.saveConditions(next === 0x7f ? 0 : next);
                        }
                    }
                }
            }
        }
        Text { text: qsTr("Schedules"); color: Theme.textDim; font.pixelSize: 13 }
        BwSwitch {
            Accessible.name: qsTr("Catch up on missed schedules")
            checked: !!macroSettings.settings.catch_up_missed
            onToggled: checked => page.invoke("set_macro_catch_up", { enabled: checked })
        }
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: qsTr("Runs a schedule or \"once at\" trigger once if its time passed while Blockwork was closed or the computer was asleep (up to a week back).")
        }
        Text { text: qsTr("Permissions"); color: Theme.textDim; font.pixelSize: 13 }
        BwSwitch {
            Accessible.name: qsTr("Ask before risky actions")
            checked: page.permissions !== null
            onToggled: checked => page.invoke("set_macro_restricted", { restricted: checked })
        }
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
            text: page.permissions === null
                ? qsTr("This macro may do anything its blocks say. Imported macros ask first until you allow them.")
                : qsTr("Ticked abilities are allowed. The rest show a dialog the first time the macro tries them.")
        }
        Repeater {
            model: page.permissions === null ? [] : page.guardedAbilities
            delegate: BwCheckBox {
                required property var modelData
                text: modelData.label
                checked: page.permissions.indexOf(modelData.kind) >= 0
                onToggled: page.invoke("set_macro_permission", { kind: modelData.kind, allowed: checked })
            }
        }
    }
    }
}
