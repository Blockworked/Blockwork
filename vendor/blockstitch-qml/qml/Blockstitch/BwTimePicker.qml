import QtQuick
import QtQuick.Controls

// A popup for picking a time of day: tap an hour and a minute, or nudge
// either with the arrows or the mouse wheel. Opens under the item it's
// given and reports the choice with `picked`; Escape or a click outside
// leaves the time as it was. With `withDate` a month calendar sits beside
// the clock and `pickedAt` reports the day too.
Popup {
    id: control
    property int hour: 9
    property int minute: 0
    property string title: qsTr("Time")
    // Shows hours 1-12 with AM/PM buttons. `picked` still gives 0-23.
    property bool use12h: false
    property string amText: Qt.locale().amText || "AM"
    property string pmText: Qt.locale().pmText || "PM"
    signal picked(int hour, int minute)

    // The date half, used while `withDate`. Months count from 1.
    property bool withDate: false
    property int year: 2026
    property int month: 1
    property int day: 1
    // The month the calendar shows, which arrows and the wheel move.
    property int shownYear: year
    property int shownMonth: month
    // January first, and Sunday first, like Date.
    property var monthNames: Array.from({ length: 12 }, (_, i) => Qt.locale().standaloneMonthName(i, Locale.LongFormat))
    property var weekdayNames: Array.from({ length: 7 }, (_, i) => Qt.locale().dayName(i, Locale.ShortFormat))
    // The calendar's first column; 0 is Sunday.
    property int firstDayOfWeek: 0
    // Refreshed on every open, for the calendar's ring and dimmed past days.
    property var today: new Date()
    signal pickedAt(int year, int month, int day, int hour, int minute)

    function pad(n) { return String(n).padStart(2, "0"); }
    function hourLabel(h) { return use12h ? String(h % 12 || 12) : pad(h); }
    // Opens below `item` (above it when there's no room), with the time set.
    function openAt(item, h, m) {
        hour = Math.max(0, Math.min(23, h | 0)); minute = Math.max(0, Math.min(59, m | 0));
        today = new Date();
        const host = parent || Overlay.overlay;
        const p = item.mapToItem(host, 0, 0);
        const below = p.y + item.height + 6;
        x = Math.max(8, Math.min(p.x, host.width - implicitWidth - 8));
        y = below + implicitHeight + 8 <= host.height ? below : Math.max(8, p.y - implicitHeight - 6);
        transformOrigin = below + implicitHeight + 8 <= host.height ? Popup.TopLeft : Popup.BottomLeft;
        open();
    }
    // Opens with the date and time set; the calendar shows that month.
    function openAtDate(item, y, mo, d, h, m) {
        setDate(y, mo, d);
        openAt(item, h, m);
    }
    function setDate(y, mo, d) {
        const date = new Date(y | 0, (mo | 0) - 1, d | 0);
        year = date.getFullYear(); month = date.getMonth() + 1; day = date.getDate();
        shownYear = year; shownMonth = month;
    }
    function shiftMonth(by) {
        const date = new Date(shownYear, shownMonth - 1 + by, 1);
        shownYear = date.getFullYear(); shownMonth = date.getMonth() + 1;
    }
    function step(part, by) {
        if (part === "hour") hour = (hour + by + 24) % 24;
        else minute = (minute + by + 60) % 60;
    }
    function accept() {
        picked(hour, minute);
        if (withDate) pickedAt(year, month, day, hour, minute);
        close();
    }

    parent: Overlay.overlay
    modal: false; focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    padding: 14
    topInset: 0; bottomInset: 0; leftInset: 0; rightInset: 0
    background: Rectangle { radius: 10; color: Theme.panelRaised; border.color: Theme.border }
    enter: Transition {
        enabled: Motion.enabled
        ParallelAnimation {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Motion.normal; easing.type: Easing.OutCubic }
            NumberAnimation { property: "scale"; from: 0.92; to: 1; duration: Motion.normal; easing.type: Easing.OutCubic }
        }
    }
    exit: Transition {
        enabled: Motion.enabled
        NumberAnimation { property: "opacity"; from: 1; to: 0; duration: Motion.quick; easing.type: Easing.InCubic }
    }

    contentItem: Column {
        spacing: 12
        Keys.onReturnPressed: control.accept()
        Keys.onEnterPressed: control.accept()

        Row {
            spacing: 18
            Calendar { visible: control.withDate }
            Rectangle { visible: control.withDate; width: 1; height: clock.height; color: Theme.borderSoft }

            Column {
                id: clock
                spacing: 12

                // The time as it stands: each half with an arrow above and below it,
                // and AM/PM beside them on a 12-hour clock.
                Row {
                    id: header
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 10
                    Repeater {
                        model: ["hour", "minute"]
                        delegate: Row {
                            id: part
                            required property string modelData
                            required property int index
                            spacing: 10
                            // Two dots drawn level with the middle of the boxes.
                            Item {
                                visible: part.index > 0
                                width: 6; height: partColumn.height
                                Repeater {
                                    model: [-9, 9]
                                    delegate: Rectangle {
                                        required property int modelData
                                        width: 6; height: 6; radius: 3; color: Theme.textDim
                                        x: 0; y: partColumn.height / 2 + modelData - 3
                                    }
                                }
                            }
                            Column {
                                id: partColumn
                                spacing: 4
                                Nudge { part: part.modelData; by: 1; icon: "chevron-up" }
                                Rectangle {
                                    width: 64; height: 52; radius: 8
                                    color: Theme.field; border.color: partWheel.hovered ? Theme.accent : Theme.border
                                    Behavior on border.color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                                    Text {
                                        anchors.centerIn: parent
                                        text: part.modelData === "hour" ? control.hourLabel(control.hour) : control.pad(control.minute)
                                        color: Theme.text; font.pixelSize: 30; font.weight: Font.DemiBold
                                    }
                                    HoverHandler { id: partWheel }
                                    WheelHandler {
                                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                        property real rest: 0
                                        onWheel: event => {
                                            rest += event.angleDelta.y / 120;
                                            const notches = rest > 0 ? Math.floor(rest) : Math.ceil(rest);
                                            if (notches !== 0) { control.step(part.modelData, notches); rest -= notches; }
                                        }
                                    }
                                }
                                Nudge { part: part.modelData; by: -1; icon: "chevron-down" }
                            }
                        }
                    }
                    Column {
                        visible: control.use12h
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        Repeater {
                            model: [false, true]
                            delegate: Rectangle {
                                required property bool modelData
                                readonly property bool on: (control.hour >= 12) === modelData
                                width: 50; height: 30; radius: 6
                                color: on ? Theme.accent : meridiemMouse.containsMouse ? Theme.hover : Theme.field
                                border.color: on ? Theme.accentHover : Theme.border
                                Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                                Text {
                                    anchors.centerIn: parent; text: modelData ? control.pmText : control.amText
                                    color: on ? "white" : Theme.text; font.pixelSize: 13; font.weight: on ? Font.DemiBold : Font.Normal
                                }
                                MouseArea {
                                    id: meridiemMouse
                                    anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                                    onClicked: if (!on) control.hour = (control.hour + 12) % 24
                                }
                                Accessible.role: Accessible.RadioButton; Accessible.checked: on; Accessible.name: modelData ? control.pmText : control.amText
                            }
                        }
                    }
                }

                Text { text: qsTr("Hour"); color: Theme.textDim; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 0.5 }
                ChoiceGrid {
                    // On a 12-hour clock the grid is 12, 1 ... 11 of the half of the day shown.
                    values: control.use12h ? Array.from({ length: 12 }, (_, i) => i + (control.hour >= 12 ? 12 : 0)) : Array.from({ length: 24 }, (_, i) => i)
                    labels: control.use12h ? values.map(h => String(h % 12 || 12)) : values.map(control.pad)
                    current: control.hour
                    onChosen: v => control.hour = v
                }
                Text { text: qsTr("Minute"); color: Theme.textDim; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 0.5 }
                ChoiceGrid {
                    values: Array.from({ length: 12 }, (_, i) => i * 5)
                    labels: values.map(control.pad)
                    current: control.minute
                    onChosen: v => control.minute = v
                }
            }
        }

        Row {
            width: parent.width
            spacing: 8
            BwButton { id: nowButton; text: qsTr("Now"); flat: true; implicitHeight: 32; onClicked: { const d = new Date(); control.hour = d.getHours(); control.minute = d.getMinutes(); if (control.withDate) control.setDate(d.getFullYear(), d.getMonth() + 1, d.getDate()); } }
            Item { width: parent.width - nowButton.width - cancelButton.width - setButton.width - 3 * parent.spacing; height: 1 }
            BwButton { id: cancelButton; text: qsTr("Cancel"); implicitHeight: 32; onClicked: control.close() }
            BwButton { id: setButton; text: qsTr("Set"); primary: true; implicitHeight: 32; onClicked: control.accept() }
        }
    }

    // An arrow that moves the hour or minute by one, and keeps going while held.
    component Nudge: Rectangle {
        id: nudgeButton
        property string part: "hour"
        property int by: 1
        property string icon: "chevron-up"
        width: 64; height: 22; radius: 5
        color: nudge.pressed ? Theme.field : nudge.containsMouse ? Theme.hover : "transparent"
        Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
        LucideIcon { anchors.centerIn: parent; name: nudgeButton.icon; width: 16; height: 16; color: Theme.textDim }
        MouseArea {
            id: nudge
            anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
            onClicked: control.step(nudgeButton.part, nudgeButton.by)
            onPressAndHold: repeatTimer.start()
            onReleased: repeatTimer.stop()
            onCanceled: repeatTimer.stop()
            Timer { id: repeatTimer; interval: 70; repeat: true; onTriggered: control.step(nudgeButton.part, nudgeButton.by) }
        }
        Accessible.role: Accessible.Button
        Accessible.name: (by > 0 ? qsTr("Later") : qsTr("Earlier")) + " " + (part === "hour" ? qsTr("hour") : qsTr("minute"))
    }

    // Six to a row; the chosen cell's highlight glides to the next choice.
    component ChoiceGrid: Item {
        id: grid
        property var values: []
        property var labels: values.map(String)
        property int current: -1
        signal chosen(int value)
        readonly property int columns: 6
        readonly property real cell: 38
        readonly property real gap: 4
        readonly property int chosenIndex: values.indexOf(current)
        width: columns * cell + (columns - 1) * gap
        height: Math.ceil(values.length / columns) * (30 + gap) - gap
        Rectangle {
            visible: grid.chosenIndex >= 0
            width: grid.cell; height: 30; radius: 6; color: Theme.accent
            x: (Math.max(0, grid.chosenIndex) % grid.columns) * (grid.cell + grid.gap)
            y: Math.floor(Math.max(0, grid.chosenIndex) / grid.columns) * (30 + grid.gap)
            Behavior on x { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }
            Behavior on y { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }
        }
        Repeater {
            model: grid.values
            delegate: Rectangle {
                required property int modelData
                required property int index
                readonly property bool isChosen: index === grid.chosenIndex
                x: (index % grid.columns) * (grid.cell + grid.gap)
                y: Math.floor(index / grid.columns) * (30 + grid.gap)
                width: grid.cell; height: 30; radius: 6
                color: isChosen ? "transparent" : cellMouse.containsMouse ? Theme.hover : Theme.field
                Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                Text {
                    anchors.centerIn: parent; text: grid.labels[index] || ""
                    color: isChosen ? "white" : Theme.text; font.pixelSize: 13
                    font.weight: isChosen ? Font.DemiBold : Font.Normal
                }
                MouseArea { id: cellMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: grid.chosen(modelData) }
            }
        }
    }

    // A month of days to pick from: arrows or the wheel change the month,
    // today has a ring, and the chosen day's highlight glides like the grids'.
    component Calendar: Column {
        id: calendar
        spacing: 8
        readonly property real cell: 34
        readonly property real gap: 4
        readonly property real rowHeight: 30
        readonly property int lead: (new Date(control.shownYear, control.shownMonth - 1, 1).getDay() - control.firstDayOfWeek + 7) % 7
        readonly property int length: new Date(control.shownYear, control.shownMonth, 0).getDate()
        readonly property int chosenIndex: control.year === control.shownYear && control.month === control.shownMonth ? lead + control.day - 1 : -1
        readonly property var today: control.today
        readonly property int todayIndex: today.getFullYear() === control.shownYear && today.getMonth() + 1 === control.shownMonth ? lead + today.getDate() - 1 : -1
        width: 7 * cell + 6 * gap

        WheelHandler {
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            property real rest: 0
            onWheel: event => {
                rest += event.angleDelta.y / 120;
                const notches = rest > 0 ? Math.floor(rest) : Math.ceil(rest);
                if (notches !== 0) { control.shiftMonth(-notches); rest -= notches; }
            }
        }

        Item {
            width: parent.width; height: 32
            MonthArrow { anchors.left: parent.left; by: -1 }
            Text {
                anchors.centerIn: parent
                text: (control.monthNames[control.shownMonth - 1] || "") + " " + control.shownYear
                color: Theme.text; font.pixelSize: 15; font.weight: Font.DemiBold
            }
            MonthArrow { anchors.right: parent.right; by: 1 }
        }
        Row {
            spacing: calendar.gap
            Repeater {
                model: 7
                delegate: Text {
                    required property int index
                    width: calendar.cell; horizontalAlignment: Text.AlignHCenter
                    text: control.weekdayNames[(index + control.firstDayOfWeek) % 7] || ""
                    color: Theme.textDim; font.pixelSize: 11; font.weight: Font.DemiBold
                }
            }
        }
        Item {
            width: parent.width
            // Always six weeks tall, so the popup doesn't jump between months.
            height: 6 * (calendar.rowHeight + calendar.gap) - calendar.gap
            Rectangle {
                visible: calendar.chosenIndex >= 0
                width: calendar.cell; height: calendar.rowHeight; radius: 6; color: Theme.accent
                x: (Math.max(0, calendar.chosenIndex) % 7) * (calendar.cell + calendar.gap)
                y: Math.floor(Math.max(0, calendar.chosenIndex) / 7) * (calendar.rowHeight + calendar.gap)
                Behavior on x { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }
                Behavior on y { enabled: Motion.enabled; NumberAnimation { duration: Motion.normal; easing.type: Easing.OutCubic } }
            }
            Repeater {
                model: calendar.length
                delegate: Rectangle {
                    required property int index
                    readonly property int slot: calendar.lead + index
                    readonly property bool isChosen: slot === calendar.chosenIndex
                    readonly property bool isToday: slot === calendar.todayIndex
                    readonly property bool past: new Date(control.shownYear, control.shownMonth - 1, index + 1) < new Date(calendar.today.getFullYear(), calendar.today.getMonth(), calendar.today.getDate())
                    x: (slot % 7) * (calendar.cell + calendar.gap)
                    y: Math.floor(slot / 7) * (calendar.rowHeight + calendar.gap)
                    width: calendar.cell; height: calendar.rowHeight; radius: 6
                    color: isChosen ? "transparent" : dayMouse.containsMouse ? Theme.hover : Theme.field
                    border.color: isToday && !isChosen ? Theme.accent : "transparent"
                    Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
                    Text {
                        anchors.centerIn: parent; text: index + 1
                        color: isChosen ? "white" : past ? Theme.textDim : Theme.text; font.pixelSize: 13
                        font.weight: isChosen || isToday ? Font.DemiBold : Font.Normal
                    }
                    MouseArea {
                        id: dayMouse
                        anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                        onClicked: { control.year = control.shownYear; control.month = control.shownMonth; control.day = index + 1; }
                        onDoubleClicked: control.accept()
                    }
                    Accessible.role: Accessible.Button; Accessible.checked: isChosen
                    Accessible.name: (index + 1) + " " + (control.monthNames[control.shownMonth - 1] || "") + " " + control.shownYear
                }
            }
        }
    }

    // Steps the calendar a month back or on.
    component MonthArrow: Rectangle {
        id: monthArrow
        property int by: 1
        width: 32; height: 32; radius: 6
        color: arrowMouse.pressed ? Theme.field : arrowMouse.containsMouse ? Theme.hover : "transparent"
        Behavior on color { enabled: Motion.enabled; ColorAnimation { duration: Motion.quick } }
        LucideIcon { anchors.centerIn: parent; name: monthArrow.by > 0 ? "chevron-right" : "chevron-left"; width: 16; height: 16; color: Theme.textDim }
        MouseArea {
            id: arrowMouse
            anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
            onClicked: control.shiftMonth(monthArrow.by)
        }
        Accessible.role: Accessible.Button
        Accessible.name: monthArrow.by > 0 ? qsTr("Next month") : qsTr("Previous month")
    }
}
