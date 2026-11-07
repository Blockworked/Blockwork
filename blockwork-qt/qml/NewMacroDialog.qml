import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

AppDialog {
    id: dialog
    title: qsTr("New macro")
    acceptText: qsTr("Create")
    standardButtons: Dialog.Ok | Dialog.Cancel
    width: Math.min(560, parent ? parent.width - 32 : 560)
    height: Math.min(advanced.checked ? 760 : 260, parent ? parent.height - 32 : 760)
    property var guardedAbilities: []
    property var draft: ({name: "", speed_multiplier: 1, settings: {}})
    property var permissions: null
    signal createRequested(var options)
    readonly property bool valid: titleField.text.trim().length > 0

    function edit(command, args) {
        const next = JSON.parse(JSON.stringify(draft));
        switch (command) {
        case "set_macro_speed_multiplier":
            if (isFinite(args.multiplier)) next.speed_multiplier = Math.max(.1, Math.min(10, args.multiplier));
            break;
        case "set_macro_max_run_secs": next.settings.max_run_secs = args.seconds; break;
        case "set_macro_max_loop_iterations": next.settings.max_loop_iterations = args.iterations; break;
        case "set_macro_always_listen": next.settings.always_listen = args.enabled; break;
        case "set_macro_catch_up": next.settings.catch_up_missed = args.enabled; break;
        case "set_macro_conditions":
            Object.assign(next.settings, {only_in_window: args.onlyInWindow, power: args.power,
                active_from: args.activeFrom, active_until: args.activeUntil, active_days: args.activeDays});
            break;
        case "set_macro_restricted": permissions = args.restricted ? [] : null; break;
        case "set_macro_permission":
            const allowed = (permissions || []).filter(kind => kind !== args.kind);
            if (args.allowed) allowed.push(args.kind);
            permissions = allowed;
            break;
        }
        draft = next;
    }
    onOpened: {
        draft = {name: "", speed_multiplier: 1, settings: {}};
        permissions = null;
        titleField.text = "";
        advanced.checked = false;
        editor.reset();
        standardButton(Dialog.Ok).enabled = Qt.binding(() => dialog.valid);
        titleField.forceActiveFocus();
    }
    onAccepted: {
        // Commit any active text editor before collecting the draft.
        titleField.forceActiveFocus();
        if (!valid) return;
        const s = draft.settings;
        createRequested({title: titleField.text.trim(), speedMultiplier: draft.speed_multiplier,
            maxRunSecs: s.max_run_secs || 0, maxLoopIterations: s.max_loop_iterations || 0,
            alwaysListen: !!s.always_listen, catchUpMissed: !!s.catch_up_missed,
            onlyInWindow: s.only_in_window || "", power: s.power || "Any",
            activeFrom: s.active_from || "", activeUntil: s.active_until || "",
            activeDays: s.active_days || 0, permissions: permissions});
    }
    contentItem: ColumnLayout {
        spacing: 12
        Text { text: qsTr("Title"); color: Theme.text; font.pixelSize: 13 }
        StitchTextField {
            id: titleField
            objectName: "newMacroTitle"
            Layout.fillWidth: true
            Accessible.name: qsTr("Title")
            placeholderText: qsTr("Macro title")
            onAccepted: if (dialog.valid) dialog.accept()
        }
        StitchButton {
            id: advanced
            objectName: "newMacroAdvanced"
            Layout.fillWidth: true
            text: qsTr("Advanced settings")
            iconName: checked ? "chevron-down" : "chevron-right"
            checkable: true
            flat: true
        }
        MacroSettingsEditor {
            id: editor
            visible: advanced.checked
            Layout.fillWidth: true
            Layout.fillHeight: true
            showTitle: false
            macro: dialog.draft
            permissions: dialog.permissions
            guardedAbilities: dialog.guardedAbilities
            invoke: (command, args) => dialog.edit(command, args)
        }
    }
}
