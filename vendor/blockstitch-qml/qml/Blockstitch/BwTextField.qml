import QtQuick
import QtQuick.Controls

TextField {
    id:control
    color:Theme.text; placeholderTextColor:Theme.textDim; selectionColor:Theme.accent
    font.pixelSize:13; selectByMouse:true; implicitHeight:Math.max(34, Theme.minimumTarget)
    leftPadding:10;rightPadding:10;topPadding:5;bottomPadding:5
    background:Rectangle {
        radius:5;color:control.enabled?Theme.field:Theme.canvas
        border.color:control.activeFocus?Theme.accent:Theme.border
        border.width:control.activeFocus?1.5:1
        Behavior on border.color { enabled:Motion.enabled; ColorAnimation { duration:Motion.quick } }
    }
}
