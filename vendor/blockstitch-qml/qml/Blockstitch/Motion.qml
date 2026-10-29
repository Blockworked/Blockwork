pragma Singleton
import QtQuick

// Shared UI animation timing. Every transition takes its duration from here,
// so the app's animation speed setting reaches all of them at once.
QtObject {
    // Multiplies every duration: 0 is Instant (no animation), 1 is normal, higher is slower.
    property real durationScale: 1
    readonly property bool enabled: durationScale > 0

    // Hover and press feedback.
    readonly property int quick: dur(110)
    // Menus, popups, toggles and other small state changes.
    readonly property int normal: dur(170)
    // Dialogs and page changes.
    readonly property int slow: dur(240)
    // Easing a mouse wheel notch to its scroll target.
    readonly property int scroll: dur(180)

    function dur(ms) { return Math.round(ms * durationScale); }
}
