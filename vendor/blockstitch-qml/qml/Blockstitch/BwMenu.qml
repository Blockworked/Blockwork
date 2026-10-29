import QtQuick
import QtQuick.Controls

// Themed context menu. Sized explicitly so it never collapses to the width of
// its narrowest item, and independent of the system palette.
Menu {
    id: control
    padding: 5
    topInset: 0; bottomInset: 0; leftInset: 0; rightInset: 0
    implicitWidth: 220
    background: Rectangle { implicitWidth: 220; radius: 8; color: Theme.panelRaised; border.color: Theme.border }
    contentItem: ListView {
        implicitHeight: contentHeight
        model: control.contentModel
        interactive: Window.window ? contentHeight + control.topPadding + control.bottomPadding > Window.window.height : false
        clip: true
        currentIndex: control.currentIndex
        boundsBehavior: Flickable.StopAtBounds
    }
    delegate: BwMenuItem {}

    // Grows out of the corner it opened from while fading in.
    transformOrigin: Popup.TopLeft
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
}
