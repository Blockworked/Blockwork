import QtQuick

// A list of instructions as a model keyed by id. Syncing a new list keeps
// the delegate of every block whose own JSON didn't change, so an edit only
// rebuilds the blocks it touched instead of the whole stack. Delegates read
// `payload`, the instruction as JSON (a ListModel role can't hold an array).
ListModel {
    id: model
    property var instructions: []
    onInstructionsChanged: sync()
    Component.onCompleted: sync()

    function sync() {
        const list = instructions || [];
        for (let j = 0; j < list.length; ++j) {
            const ins = list[j];
            const key = ins && ins.id ? String(ins.id) : "#" + j;
            const payload = JSON.stringify(ins);
            let at = -1;
            for (let i = j; i < count; ++i) if (get(i).key === key) { at = i; break; }
            if (at < 0) { insert(j, { key: key, payload: payload }); continue; }
            if (at !== j) move(at, j, 1);
            if (get(j).payload !== payload) setProperty(j, "payload", payload);
        }
        if (count > list.length) remove(list.length, count - list.length);
    }
}
