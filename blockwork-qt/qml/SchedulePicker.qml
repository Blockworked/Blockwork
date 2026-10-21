import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

// Builds the cron expression behind an "on schedule" block from plain
// choices: repeat every few minutes or hours, run at a time on chosen days,
// or once a month. Anything else stays editable as cron text.
AppDialog {
    id: root
    title: qsTr("Schedule")
    standardButtons: Dialog.Ok | Dialog.Cancel
    acceptText: qsTr("Use schedule")
    signal chosen(string expression)

    // "repeat", "daily", "monthly" or "custom".
    property string mode: "daily"
    property int every: 15
    property string unit: "minutes"
    property bool windowed: false
    property int fromHour: 9
    property int toHour: 17
    property int hour: 9
    property int minute: 0
    property int monthDay: 1
    // Sunday first, like cron.
    property var days: [false, true, true, true, true, true, false]

    readonly property var minuteSteps: [1, 2, 3, 4, 5, 6, 10, 12, 15, 20, 30]
    readonly property var hourSteps: [1, 2, 3, 4, 6, 8, 12]
    readonly property var modes: [
        { id: "repeat", label: qsTr("Repeat"), icon: "repeat" },
        { id: "daily", label: qsTr("At a time"), icon: "clock" },
        { id: "monthly", label: qsTr("Monthly"), icon: "layout-grid" },
        { id: "custom", label: qsTr("Custom"), icon: "square-code" }
    ]

    function daysPart() {
        const on = [];
        for (let d = 0; d < 7; ++d) if (days[d]) on.push(d);
        if (on.length === 0) return null;
        if (on.length === 7) return "*";
        if (on.join(",") === "1,2,3,4,5") return "mon-fri";
        // Monday first, so weekends read "sat,sun".
        return on.sort((a, b) => ((a + 6) % 7) - ((b + 6) % 7)).map(d => Schedules.weekdayKeys[d]).join(",");
    }
    readonly property bool noDays: (mode === "repeat" || mode === "daily") && days.indexOf(true) < 0
    readonly property string expression: {
        if (mode === "custom") return customField.text.trim();
        if (noDays) return "";
        if (mode === "monthly") return minute + " " + hour + " " + monthDay + " * *";
        if (mode === "daily") return minute + " " + hour + " * * " + daysPart();
        const span = windowed ? fromHour + "-" + (toHour - 1) : "*";
        if (unit === "minutes") return (every === 1 ? "*" : "*/" + every) + " " + span + " * * " + daysPart();
        const hours = windowed ? (every === 1 ? span : span + "/" + every) : (every === 1 ? "*" : "*/" + every);
        return "0 " + hours + " * * " + daysPart();
    }
    readonly property bool valid: Schedules.parseCron(expression) !== null && Schedules.nextRuns(expression, 1).length > 0
    readonly property string summary: valid ? (Schedules.describe(expression) || expression) : ""
    readonly property var upcoming: valid ? Schedules.nextRuns(expression, 3) : []
    onValidChanged: { const ok = standardButton(Dialog.Ok); if (ok) ok.enabled = valid; }

    // Reads `text` into the choices; anything they can't show opens as cron text.
    function openWith(text) {
        const t = String(text || "").trim();
        customField.text = t;
        const c = t.length ? Schedules.parseCron(t) : null;
        mode = "custom";
        if (!t.length) { mode = "daily"; hour = 9; minute = 0; days = [false, true, true, true, true, true, false]; }
        else if (c) {
            const [m, h, dom, mon, dow] = c.parts;
            const daySet = [0, 1, 2, 3, 4, 5, 6].map(d => c.weekday.indexOf(d) >= 0);
            const range = Schedules.rangeOf(h);
            if (dom === "*" && mon === "*") {
                const minuteStep = Schedules.stepOf(m), hourStep = Schedules.stepOf(h);
                if (minuteStep && minuteSteps.indexOf(minuteStep) >= 0 && (h === "*" || (range && range[1] < 23))) {
                    mode = "repeat"; unit = "minutes"; every = minuteStep; days = daySet;
                    windowed = !!range; if (range) { fromHour = range[0]; toHour = range[1] + 1; }
                } else if (m === "0" && hourStep && hourSteps.indexOf(hourStep) >= 0) {
                    mode = "repeat"; unit = "hours"; every = hourStep; windowed = false; days = daySet;
                } else if (m === "0" && range && range[1] < 23) {
                    mode = "repeat"; unit = "hours"; every = 1; windowed = true; fromHour = range[0]; toHour = range[1] + 1; days = daySet;
                } else if (Schedules.isNumber(m) && Schedules.isNumber(h)) {
                    mode = "daily"; hour = Number(h); minute = Number(m); days = daySet;
                }
            } else if (Schedules.isNumber(m) && Schedules.isNumber(h) && Schedules.isNumber(dom) && mon === "*" && dow === "*") {
                mode = "monthly"; hour = Number(h); minute = Number(m); monthDay = Number(dom);
            }
        }
        open();
        Qt.callLater(() => { const ok = standardButton(Dialog.Ok); if (ok) ok.enabled = valid; });
    }
    onAccepted: if (valid) chosen(expression)

    component SectionLabel: Text { color: Theme.textDim; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 0.5 }
    component Phrase: Text { color: Theme.text; font.pixelSize: 13; Layout.alignment: Qt.AlignVCenter }
    // A time that opens the time picker.
    component TimeButton: BwButton {
        id: timeButton
        property int h: 0
        property int m: 0
        signal changed(int h, int m)
        iconName: "clock"; text: Schedules.clock(h, m); implicitHeight: 32
        onClicked: picker.openAt(timeButton, h, m)
        BwTimePicker { id: picker; onPicked: (h, m) => timeButton.changed(h, m) }
    }
    component Choice: BwComboBox {
        id: choice
        property var values: []
        property var labels: values.map(String)
        property int current: 0
        signal changedTo(int value)
        model: labels; currentIndex: values.indexOf(current)
        implicitHeight: 32; font.pixelSize: 13
        implicitWidth: Math.max(64, choiceMetrics.advanceWidth + 44)
        TextMetrics { id: choiceMetrics; font: choice.font; text: choice.displayText }
        onActivated: index => changedTo(values[index])
    }

    Column {
        width: 470; spacing: 16

        // The four ways to set it; the highlight slides to the chosen one.
        Rectangle {
            width: parent.width; height: 40; radius: 8; color: Theme.field; border.color: Theme.borderSoft
            Rectangle {
                readonly property int at: root.modes.findIndex(m => m.id === root.mode)
                x: 4 + at * (parent.width - 8) / root.modes.length; y: 4
                width: (parent.width - 8) / root.modes.length; height: parent.height - 8; radius: 6
                color: Theme.accent
                Behavior on x { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }
            }
            Row {
                x: 4; y: 4
                Repeater {
                    model: root.modes
                    delegate: Item {
                        required property var modelData
                        readonly property bool on: root.mode === modelData.id
                        width: (470 - 8) / root.modes.length; height: 32
                        Row {
                            anchors.centerIn: parent; spacing: 6
                            LucideIcon { name: modelData.icon; width: 14; height: 14; color: on ? "white" : Theme.textDim; anchors.verticalCenter: parent.verticalCenter }
                            Text { text: modelData.label; color: on ? "white" : Theme.text; font.pixelSize: 13; font.weight: on ? Font.DemiBold : Font.Normal; anchors.verticalCenter: parent.verticalCenter }
                        }
                        MouseArea {
                            anchors.fill: parent; cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (modelData.id === "custom" && root.mode !== "custom" && root.valid) customField.text = root.expression;
                                root.mode = modelData.id;
                            }
                        }
                        Accessible.role: Accessible.PageTab; Accessible.name: modelData.label
                    }
                }
            }
        }

        // The panes share one fixed-height spot, so switching doesn't resize the dialog.
        Item {
            width: parent.width; height: 196
            Column {
                width: parent.width; spacing: 16
                // ---- repeat ----
                Column {
                    visible: root.mode === "repeat"; width: parent.width; spacing: 12
                    RowLayout {
                        spacing: 8
                        Phrase { text: qsTr("Every") }
                        Choice {
                            values: root.unit === "minutes" ? root.minuteSteps : root.hourSteps
                            current: root.every
                            onChangedTo: v => root.every = v
                        }
                        BwComboBox {
                            model: [qsTr("minutes"), qsTr("hours")]; currentIndex: root.unit === "minutes" ? 0 : 1
                            implicitHeight: 32; implicitWidth: 110; font.pixelSize: 13
                            onActivated: index => {
                                root.unit = index === 0 ? "minutes" : "hours";
                                const steps = index === 0 ? root.minuteSteps : root.hourSteps;
                                if (steps.indexOf(root.every) < 0) root.every = steps[0];
                            }
                        }
                    }
                    RowLayout {
                        spacing: 8
                        BwCheckBox { id: windowBox; text: qsTr("Only between"); checked: root.windowed; onToggled: root.windowed = checked; font.pixelSize: 13 }
                        Choice {
                            enabled: root.windowed; opacity: enabled ? 1 : .45
                            values: Array.from({ length: 24 }, (_, i) => i); labels: values.map(h => Schedules.clock(h, 0))
                            current: root.fromHour
                            onChangedTo: v => { root.fromHour = v; if (root.toHour <= v) root.toHour = v + 1; }
                        }
                        Phrase { text: qsTr("and"); opacity: root.windowed ? 1 : .45 }
                        Choice {
                            enabled: root.windowed; opacity: enabled ? 1 : .45
                            values: Array.from({ length: 24 }, (_, i) => i + 1); labels: values.map(h => Schedules.clock(h, 0))
                            current: root.toHour
                            onChangedTo: v => { root.toHour = v; if (root.fromHour >= v) root.fromHour = v - 1; }
                        }
                    }
                }

                // ---- at a time ----
                RowLayout {
                    visible: root.mode === "daily"; spacing: 8
                    Phrase { text: qsTr("At") }
                    TimeButton { h: root.hour; m: root.minute; onChanged: (h, m) => { root.hour = h; root.minute = m; } }
                }

                // ---- days, for both of those ----
                Column {
                    visible: root.mode === "repeat" || root.mode === "daily"; width: parent.width; spacing: 8
                    SectionLabel { text: qsTr("ON THESE DAYS") }
                    Row {
                        spacing: 6
                        Repeater {
                            // Monday first.
                            model: [1, 2, 3, 4, 5, 6, 0]
                            delegate: Rectangle {
                                required property int modelData
                                readonly property bool on: root.days[modelData]
                                width: 52; height: 34; radius: 17
                                color: on ? Theme.accent : dayMouse.containsMouse ? "#3b3c40" : Theme.field
                                border.color: on ? Theme.accentHover : Theme.border
                                Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                                Text { anchors.centerIn: parent; text: Schedules.weekdayShort(modelData); color: on ? "white" : Theme.text; font.pixelSize: 12; font.weight: on ? Font.DemiBold : Font.Normal }
                                MouseArea {
                                    id: dayMouse
                                    anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                    onClicked: { const d = root.days.slice(); d[modelData] = !d[modelData]; root.days = d; }
                                }
                                Accessible.role: Accessible.CheckBox; Accessible.checked: on; Accessible.name: Schedules.weekdayShort(modelData)
                            }
                        }
                    }
                    Row {
                        spacing: 6
                        BwButton { text: qsTr("Every day"); flat: true; implicitHeight: 28; font.pixelSize: 12; onClicked: root.days = [true, true, true, true, true, true, true] }
                        BwButton { text: qsTr("Weekdays"); flat: true; implicitHeight: 28; font.pixelSize: 12; onClicked: root.days = [false, true, true, true, true, true, false] }
                        BwButton { text: qsTr("Weekends"); flat: true; implicitHeight: 28; font.pixelSize: 12; onClicked: root.days = [true, false, false, false, false, false, true] }
                    }
                }

                // ---- monthly ----
                Column {
                    visible: root.mode === "monthly"; width: parent.width; spacing: 8
                    RowLayout {
                        spacing: 8
                        Phrase { text: qsTr("On day") }
                        Choice { values: Array.from({ length: 31 }, (_, i) => i + 1); current: root.monthDay; onChangedTo: v => root.monthDay = v }
                        Phrase { text: qsTr("of every month, at") }
                        TimeButton { h: root.hour; m: root.minute; onChanged: (h, m) => { root.hour = h; root.minute = m; } }
                    }
                    Text {
                        visible: root.monthDay > 28; width: parent.width; wrapMode: Text.WordWrap
                        text: qsTr("Months without that day are skipped."); color: Theme.textDim; font.pixelSize: 12
                    }
                }

                // ---- custom ----
                Column {
                    visible: root.mode === "custom"; width: parent.width; spacing: 8
                    BwTextField { id: customField; width: parent.width; font.family: "monospace"; placeholderText: "*/15 9-17 * * mon-fri"; onAccepted: if (root.valid) root.accept() }
                    Text {
                        width: parent.width; wrapMode: Text.WordWrap; color: Theme.textDim; font.pixelSize: 12
                        text: qsTr("Cron format: minute hour day month weekday. Like '0 9 * * mon-fri' (9:00 on weekdays) or '*/15 * * * *' (every 15 minutes).")
                    }
                }
            }
        }

        // What it comes to.
        Rectangle {
            width: parent.width; height: summaryCol.implicitHeight + 20; radius: 8
            color: Theme.panelRaised; border.color: root.valid ? Theme.borderSoft : Theme.danger
            Behavior on border.color { enabled: Motion.enabled; ColorAnimation { duration: Motion.normal } }
            Column {
                id: summaryCol
                x: 12; y: 10; width: parent.width - 24; spacing: 4
                Row {
                    spacing: 8
                    LucideIcon { name: root.valid ? "clock" : "circle-alert"; width: 16; height: 16; color: root.valid ? Theme.accent : Theme.danger; anchors.verticalCenter: parent.verticalCenter }
                    Text {
                        width: summaryCol.width - 24; wrapMode: Text.WordWrap; font.pixelSize: 13; font.weight: Font.DemiBold
                        color: root.valid ? Theme.text : Theme.danger
                        text: root.valid ? qsTr("Runs %1").arg(root.summary)
                            : root.noDays ? qsTr("Pick at least one day.")
                            : root.expression.length ? qsTr("Blockwork can't read that schedule, or it never runs.") : qsTr("Type a schedule.")
                    }
                }
                Text {
                    visible: root.upcoming.length > 0; leftPadding: 24
                    text: qsTr("Next: %1").arg(root.upcoming.map(d => Schedules.runLabel(d)).join("  ·  "))
                    color: Theme.textDim; font.pixelSize: 12
                }
                Text {
                    visible: root.valid && root.mode !== "custom"; leftPadding: 24
                    text: root.expression; color: Theme.textDim; font.pixelSize: 11; font.family: "monospace"; opacity: 0.8
                }
            }
        }
    }
}
