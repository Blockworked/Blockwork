import QtQuick
import QtQuick.Controls

ComboBox {
    id:control
    implicitHeight:Math.max(34,Theme.minimumTarget);leftPadding:10;rightPadding:32;font.pixelSize:13
    HoverHandler { cursorShape: control.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor }
    // The list opens at least as wide as its longest entry, so a narrow box doesn't cut them off.
    FontMetrics { id:itemMetrics; font:control.font }
    readonly property real widestItem: { let w=0; for (let i=0;i<count;++i) w=Math.max(w,itemMetrics.advanceWidth(textAt(i))); return w; }
    contentItem:Text { text:control.displayText;color:control.enabled?Theme.text:Theme.textDim;font:control.font;verticalAlignment:Text.AlignVCenter;elide:Text.ElideRight }
    indicator:LucideIcon { name:"chevron-down";rotation:control.popup.visible?180:0;color:control.enabled?Theme.text:Theme.textDim;width:15;height:15;x:control.width-width-9;y:(control.height-height)/2
        Behavior on rotation { enabled:Motion.enabled; NumberAnimation { duration:Motion.normal;easing.type:Easing.OutCubic } } }
    background:Rectangle { radius:5;color:control.down?Theme.field:(control.hovered?Theme.hover:Theme.panelRaised);border.color:control.activeFocus?Theme.accent:Theme.border
        Behavior on color { enabled:Motion.enabled; ColorAnimation { duration:Motion.quick } } }
    popup:Popup {
        y:control.height+3;width:Math.max(control.width,implicitWidth,Math.ceil(control.widestItem)+36);height:Math.min(264,control.count*Math.max(32,Theme.minimumTarget)+8);padding:4;topInset:0;bottomInset:0;leftInset:0;rightInset:0
        contentItem:ListView { clip:true;model:control.popup.visible?control.delegateModel:null;currentIndex:control.highlightedIndex;boundsBehavior:Flickable.StopAtBounds;ScrollBar.vertical:ScrollBar{policy:control.count*Math.max(32,Theme.minimumTarget)>256?ScrollBar.AsNeeded:ScrollBar.AlwaysOff} }
        background:Rectangle { radius:6;color:Theme.menu;border.color:Theme.border }
        // Drops open from the box: fades in while unfolding downward.
        transformOrigin:Popup.Top
        enter:Transition { enabled:Motion.enabled
            ParallelAnimation {
                NumberAnimation { property:"opacity";from:0;to:1;duration:Motion.normal;easing.type:Easing.OutCubic }
                NumberAnimation { property:"scale";from:0.94;to:1;duration:Motion.normal;easing.type:Easing.OutCubic }
            } }
        exit:Transition { enabled:Motion.enabled; NumberAnimation { property:"opacity";from:1;to:0;duration:Motion.quick;easing.type:Easing.InCubic } }
    }
    delegate:ItemDelegate {
        width:ListView.view.width;implicitHeight:Math.max(32,Theme.minimumTarget);highlighted:control.highlightedIndex===index
        HoverHandler { cursorShape: Qt.PointingHandCursor }
        contentItem:Text { text:control.textRole?((modelData!==null&&typeof modelData==="object"&&modelData[control.textRole]!==undefined)?modelData[control.textRole]:model[control.textRole]):modelData;color:Theme.text;font:control.font;verticalAlignment:Text.AlignVCenter }
        background:Rectangle { radius:4;color:parent.highlighted?Theme.hover:"transparent"; Behavior on color { enabled:Motion.enabled; ColorAnimation { duration:Motion.quick } } }
    }
}
