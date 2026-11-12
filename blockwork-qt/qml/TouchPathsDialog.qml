import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch 1.0

AppDialog {
    id: root
    objectName: "touchPathsDialog"
    required property Item page
    title: qsTr("Touch paths")
    width: Math.min(800, parent ? parent.width - 32 : 800)
    height: Math.min(650, parent ? parent.height - 32 : 650)
    standardButtons: Dialog.NoButton
    property var request: null
    property string macroId: ""
    property var frames: []
    property string errorText: ""
    property var invalid: ({})
    readonly property bool valid: !Object.values(invalid).some(v => v)
    property bool busy: false
    property int serial: 0
    readonly property bool normalized: !!request && !!request.instruction.normalized
    function openFor(req) {
        serial++;
        request = JSON.parse(JSON.stringify(req));
        macroId = page.macro ? page.macro.id : "";
        errorText = ""; invalid = ({}); busy = false;
        try {
            const value = req.instruction.sequence;
            const data = value && value.kind === "Text" ? JSON.parse(value.value) : null;
            if (!data || data.version !== 1 || !Array.isArray(data.frames)) throw new Error(qsTr("These paths cannot be edited here. The paths input must contain saved contact frames."));
            frames = data.frames;
        } catch (error) { frames = []; errorText = String(error); }
        open();
    }
    function copy() { return JSON.parse(JSON.stringify(frames)); }
    function check(key, text, scale, integer) {
        const number = Number(text);
        const bad = !String(text).trim().length || !isFinite(number) || (integer && !Number.isInteger(number)) || Math.abs(number * scale) > 2147483647;
        invalid = Object.assign({}, invalid, {[key]: bad});
        return !bad;
    }
    function time(row, text, expected) {
        if (expected && frames[row] !== expected) return;
        if (!check("time" + row, text, 1, true)) return;
        if (frames[row].at_ms === Number(text)) return;
        const next = copy(); next[row].at_ms = Number(text); frames = next; errorText = "";
    }
    function point(row, column, key, text, expected) {
        if (expected && frames[row] !== expected) return;
        const scale = normalized ? 10000 : 1;
        if (!check(row + ":" + column + ":" + key, text, scale, false)) return;
        const value = Math.round(Number(text) * scale);
        if (frames[row].contacts[column][key] === value) return;
        const next = copy(); next[row].contacts[column][key] = value; frames = next; errorText = "";
    }
    function addFrame() {
        if (frames.length < 2) return;
        const next = copy(), release = next.pop(), previous = next[next.length - 1];
        const at = previous.at_ms + 100;
        next.push({at_ms: at, contacts: JSON.parse(JSON.stringify(previous.contacts))});
        release.at_ms = at + 1; release.contacts = []; next.push(release);
        frames = next; invalid = ({}); errorText = "";
    }
    function addContact(row) {
        const next = copy(), contacts = next[row].contacts;
        const id = Array.from({length: 10}, (_, i) => i).find(i => !contacts.some(p => p.id === i));
        if (id === undefined) return;
        contacts.push({id: id, x: normalized ? 500000 : 500, y: normalized ? 500000 : 500});
        frames = next; errorText = "";
    }
    function removeContact(row, column) { const next = copy(); next[row].contacts.splice(column, 1); frames = next; invalid = ({}); }
    function removeFrame(row) {
        if (frames.length <= 2) return;
        const next = copy(); next.splice(row, 1); next[0].at_ms = 0; frames = next; invalid = ({});
    }
    function apply() {
        contentItem.forceActiveFocus();
        Qt.callLater(() => {
            if (!valid || busy) return;
            const sequence = JSON.stringify({version: 1, frames: frames}), ticket = ++serial;
            busy = true;
            page.daemonCall("validate_touch_paths", {sequence: sequence}, (result, error) => {
                if (ticket !== serial || !root.visible) return;
                busy = false;
                if (error) { errorText = error; return; }
                if (!page.macro || page.macro.id !== macroId) { errorText = qsTr("The macro changed. Reopen its touch paths."); return; }
                if (page.recordingBusy) { errorText = qsTr("Stop recording before changing these paths."); return; }
                const current = page.instructionForRequest(request);
                if (!current || JSON.stringify(current.sequence) !== JSON.stringify(request.instruction.sequence)) { errorText = qsTr("The block changed. Reopen its touch paths."); return; }
                const updated = JSON.parse(JSON.stringify(current));
                updated.sequence = {kind: "Text", value: sequence};
                page.invoke("edit_instruction", {strandId: request.strandId, path: request.path, instruction: updated});
                root.accept();
            });
        });
    }
    onClosed: serial++
    onFramesChanged: {
        if (preview) preview.requestPaint();
        if (frameList) {
            const y = frameList.contentY;
            Qt.callLater(() => { if (frameList) frameList.contentY = Math.min(y, Math.max(0, frameList.contentHeight - frameList.height)); });
        }
    }
    contentItem: ColumnLayout {
        spacing: 8
        Label { Layout.fillWidth: true; text: qsTr("Every frame lists all fingers still touching. Missing fingers are released. The last frame releases every finger."); color: Theme.textDim; wrapMode: Text.WordWrap }
        Canvas {
            id: preview
            objectName: "touchPathPreview"
            Layout.fillWidth: true
            Layout.preferredHeight: 125
            onPaint: {
                const ctx = getContext("2d"); ctx.reset();
                ctx.fillStyle = Theme.field; ctx.fillRect(0,0,width,height);
                const colors = ["#4c97ff", "#ff6680", "#59c059", "#ffbf00", "#9966ff", "#5ba9d0", "#ff8c1a", "#19b88e", "#f27aed", "#8080f5"];
                let xmax = root.normalized ? 1000000 : 1920, ymax = root.normalized ? 1000000 : 1080;
                for (const frame of root.frames) for (const point of frame.contacts || []) { xmax = Math.max(xmax, point.x); ymax = Math.max(ymax, point.y); }
                const last = ({}), stride = Math.max(1, Math.ceil(root.frames.length / 2000));
                for (let i = 0; i < root.frames.length; i += stride) {
                    const active = new Set();
                    for (const point of root.frames[i].contacts || []) {
                        active.add(point.id);
                        const x = 8 + point.x / xmax * (width - 16), y = 8 + point.y / ymax * (height - 16);
                        ctx.strokeStyle = colors[point.id % 10]; ctx.fillStyle = ctx.strokeStyle; ctx.lineWidth = 2;
                        if (last[point.id]) { ctx.beginPath(); ctx.moveTo(last[point.id].x, last[point.id].y); ctx.lineTo(x,y); ctx.stroke(); }
                        ctx.beginPath(); ctx.arc(x,y,2,0,Math.PI*2); ctx.fill(); last[point.id] = {x:x,y:y};
                    }
                    for (const id of Object.keys(last)) if (!active.has(Number(id))) delete last[id];
                }
            }
        }
        Label { text: root.normalized ? qsTr("Positions are percentages of the selected monitor.") : qsTr("Positions are screen pixels."); color: Theme.textDim }
        StitchListView {
            id: frameList
            objectName: "touchFrames"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 10
            model: root.frames
            enabled: !root.busy
            delegate: ColumnLayout {
                id: frame
                required property int index
                required property var modelData
                width: frameList.width - 14
                spacing: 4
                RowLayout {
                    Layout.fillWidth: true
                    Label { text: qsTr("Frame %1 at").arg(frame.index + 1); color: Theme.text }
                    StitchTextField {
                        objectName: "touchFrameTime"
                        Layout.preferredWidth: 105
                        text: String(frame.modelData.at_ms)
                        Accessible.name: qsTr("Frame time in milliseconds")
                        onTextEdited: root.check("time" + frame.index, text, 1, true)
                        onEditingFinished: root.time(frame.index, text, frame.modelData)
                    }
                    Label { text: qsTr("ms"); color: Theme.textDim }
                    Item { Layout.fillWidth: true }
                    StitchButton { text: qsTr("Add finger"); enabled: (frame.modelData.contacts || []).length < 10 && frame.index < root.frames.length - 1; onClicked: root.addContact(frame.index) }
                    StitchButton { iconName: "trash-2"; enabled: root.frames.length > 2; onClicked: root.removeFrame(frame.index) }
                }
                Repeater {
                    model: frame.modelData.contacts || []
                    delegate: RowLayout {
                        id: contact
                        required property int index
                        required property var modelData
                        Layout.fillWidth: true
                        Label { text: qsTr("Finger %1").arg(contact.modelData.id + 1); color: Theme.textDim; Layout.preferredWidth: 90 }
                        Repeater {
                            model: ["x", "y"]
                            delegate: StitchTextField {
                                required property string modelData
                                Layout.fillWidth: true
                                Accessible.name: modelData.toUpperCase()
                                text: String(contact.modelData[modelData] / (root.normalized ? 10000 : 1))
                                onTextEdited: root.check(frame.index + ":" + contact.index + ":" + modelData, text, root.normalized ? 10000 : 1, false)
                                onEditingFinished: root.point(frame.index, contact.index, modelData, text, frame.modelData)
                            }
                        }
                        StitchButton { iconName: "x"; onClicked: root.removeContact(frame.index, contact.index) }
                    }
                }
                Label { visible: !(frame.modelData.contacts || []).length; text: qsTr("Release all fingers"); color: Theme.textDim }
            }
        }
        Label { Layout.fillWidth: true; visible: text.length > 0; text: !root.valid ? qsTr("Enter valid numbers before applying.") : root.errorText; color: Theme.danger; wrapMode: Text.WordWrap }
        RowLayout {
            Layout.fillWidth: true
            StitchButton { text: qsTr("Add frame"); enabled: root.frames.length >= 2 && !root.busy; onClicked: root.addFrame() }
            Item { Layout.fillWidth: true }
            StitchButton { text: qsTr("Cancel"); onClicked: root.reject() }
            StitchButton { objectName: "applyTouchPaths"; text: qsTr("Apply"); primary: true; enabled: root.valid && !root.busy && root.frames.length >= 2; onClicked: root.apply() }
        }
    }
}
