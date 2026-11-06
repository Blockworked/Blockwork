import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

// Timings for the editor with a big macro: page creation, loading a macro,
// an edit's state push, a drag over the canvas and zoom steps.
// Run with `just bench-qml`; prints "PERF <name> <ms>" lines.
Item {
    id: scene
    width: 1400; height: 900
    property var calls: []
    function invoke(command, args) { calls.push({command: command, args: args}); }
    QtObject {
        id: bridge
        property int replySerial: 0
        property int previewSerial: 0
        property string activeBlocksJson: "[]"
        property string installedAppsError: ""
        property string replyKey: ""
        property string replyJson: "null"
        property string replyError: ""
        property string lastError: ""
        function callCommand(key, command, args) { replyKey = key; replyJson = "null"; replyError = ""; replySerial++; }
    }
    function macro(strands) {
        return {id: "m", name: "m", speed_multiplier: 1, settings: {always_listen: false}, signals: ["go"], variables: ["v"], lists: ["l"], comments: [],
            floating_values: [], block_defs: [], strands: strands};
    }
    function state(m) {
        return {current_macro: m, recording_phase: {phase: "Idle", countdown: null}, key_capture: null, can_undo: false, can_redo: false,
            record_mouse_movement: false, record_mouse_relative: false, absolute_mouse_position_available: true, browser: {unlocked: true}};
    }
    Component {
        id: editorComponent
        EditorPage {
            width: scene.width; height: scene.height
            invoke: scene.invoke
            appBridge: bridge
            appState: scene.state(scene.macro([]))
        }
    }
    TestCase {
        name: "EditorPerf"
        when: windowShown
        function all(object) {
            let found = [object];
            for (const child of object.children || []) found = found.concat(all(child));
            return found;
        }
        function report(name, ms) { console.log("PERF " + name + " " + ms.toFixed(1)); }
        // Lets queued work (timers, polish, delegate creation) run; a frame
        // that renders nothing would make waitForRendering sit out its timeout.
        function frame() { wait(1); }
        function time(name, fn, rounds) {
            const n = rounds || 1;
            let best = 1e9, total = 0, sync = 0;
            for (let i = 0; i < n; ++i) {
                const t0 = Date.now(); fn(i); const t1 = Date.now(); frame(); const dt = Date.now() - t0;
                sync += t1 - t0;
                best = Math.min(best, dt); total += dt;
            }
            report(name, n > 1 ? total / n : best);
            report(name + "_sync", sync / n);
        }
        // Real palette instructions, so every row kind the app draws shows up.
        function strands(palette, types, count, perStrand) {
            const out = [];
            let made = 0, s = 0;
            while (made < count) {
                const ins = [];
                for (let k = 0; k < perStrand && made < count; ++k, ++made) {
                    const type = types[made % types.length];
                    const i = JSON.parse(JSON.stringify(palette.prefab(type)));
                    i.id = "b" + made;
                    if (i.body !== undefined && made % 3 === 0) i.body = [{id: "n" + made, type: "Wait", duration: {kind: "Number", value: 100}}];
                    ins.push(i);
                }
                out.push({id: "s" + s, x: 40 + (s % 4) * 420, y: 40 + Math.floor(s / 4) * 900, instructions: ins});
                ++s;
            }
            return out;
        }
        function test_page() {
            mouseMove(scene, scene.width - 2, scene.height - 2, 0, Qt.NoButton);
            for (let i = 0; i < 3; ++i) {
                let page = null;
                time("page_create_" + i, () => { page = createTemporaryObject(editorComponent, scene); });
                page.destroy();
                wait(50);
            }
        }
        function test_load() {
            mouseMove(scene, scene.width - 2, scene.height - 2, 0, Qt.NoButton);
            const page = createTemporaryObject(editorComponent, scene);
            wait(3000);
            const palette = all(page).find(o => typeof o.prefab === "function" && o.resolvedSections !== undefined);
            const types = Object.keys(BlockRegistry.rows).filter(t => !BlockRegistry.isHeader(t)
                && !/^When|Header|CallBlock|BranchCallBlock|^Test/.test(t) && palette.prefab(t));
            const big = strands(palette, types, 400, 25);
            const canvas = all(page).find(o => o.strands !== undefined && typeof o.syncStrands === "function");
            // Best of three, since one load swings by a few hundred ms.
            // "all" waits until strands outside the view have built too.
            let best = 1e9, bestSync = 1e9, bestAll = 1e9;
            for (let round = 0; round < 3; ++round) {
                page.appState = scene.state(scene.macro([]));
                wait(300);
                const t0 = Date.now();
                page.appState = scene.state(scene.macro(big));
                const t1 = Date.now();
                frame();
                best = Math.min(best, Date.now() - t0); bestSync = Math.min(bestSync, t1 - t0);
                tryVerify(() => canvas.pendingBuilds ? canvas.pendingBuilds() === 0 : (!canvas.pendingStrands || canvas.pendingStrands() === 0), 30000);
                bestAll = Math.min(bestAll, Date.now() - t0);
            }
            report("load_only_400", best);
            report("load_only_400_sync", bestSync);
            report("load_all_400", bestAll);
        }
        // One very long strand: load, edits that shift every block below,
        // dragging its head (the whole strand) and its tail, and scrolling.
        function test_long() {
            mouseMove(scene, scene.width - 2, scene.height - 2, 0, Qt.NoButton);
            const page = createTemporaryObject(editorComponent, scene);
            wait(200);
            tryVerify(() => !all(page).some(o => o.pendingEntries > 0), 20000);
            const palette = all(page).find(o => typeof o.prefab === "function" && o.resolvedSections !== undefined);
            const types = Object.keys(BlockRegistry.rows).filter(t => !BlockRegistry.isHeader(t)
                && !/^When|Header|CallBlock|BranchCallBlock|^Test/.test(t) && palette.prefab(t));
            const canvas = all(page).find(o => o.strands !== undefined && typeof o.syncStrands === "function");
            const flick = all(canvas).find(o => o.contentY !== undefined && o.flickableDirection !== undefined);
            const wait0 = () => tryVerify(() => canvas.pendingBuilds ? canvas.pendingBuilds() === 0 : (!canvas.pendingStrands || canvas.pendingStrands() === 0), 60000);
            for (const count of [400, 800]) {
                const big = strands(palette, types, count, count);
                page.appState = scene.state(scene.macro([]));
                wait(300);
                time("long_load_" + count, () => { page.appState = scene.state(scene.macro(big)); });
                const t0 = Date.now(); wait0(); report("long_load_all_" + count, Date.now() - t0);
                time("long_edit_" + count, i => {
                    const next = JSON.parse(JSON.stringify(big));
                    next[0].instructions[Math.floor(count / 2)]["__edit"] = i;
                    page.appState = scene.state(scene.macro(next));
                }, 5);
                time("long_republish_" + count, () => { page.appState = scene.state(scene.macro(JSON.parse(JSON.stringify(big)))); }, 5);
                // A block added at the top, then taken away again.
                time("long_insert_top_" + count, i => {
                    const next = JSON.parse(JSON.stringify(big));
                    if (i % 2 === 0) next[0].instructions.unshift({id: "new" + i, type: "Wait", duration: {kind: "Number", value: 5}});
                    page.appState = scene.state(scene.macro(next));
                }, 4);
                for (const which of ["head", "tail"]) {
                    const id = which === "head" ? "b0" : "b" + (count - 1);
                    const index = which === "head" ? 0 : count - 1;
                    if (which === "tail") { flick.contentY = Math.max(0, flick.contentHeight - flick.height); frame(); }
                    else { flick.contentY = 0; frame(); }
                    const block = all(canvas).find(o => o.instruction && o.instruction.id === id && o.paletteMode === false);
                    verify(!!block, "dragged " + which);
                    const p = block.mapToItem(null, 10, 10);
                    canvas.beginDrag("s0", [{index: index}], count - index, p.x, p.y, 10, 10, block.width, block.height);
                    frame();
                    const t1 = Date.now();
                    const moves = 30;
                    for (let m = 0; m < moves; ++m) { canvas.moveDrag(p.x + 300 + (m % 10) * 10, p.y + 100 - m * 5); frame(); }
                    report("long_drag_" + which + "_" + count, (Date.now() - t1) / moves);
                    canvas.cancelDrag();
                    wait(300);
                }
                flick.contentY = 0; frame();
                const t2 = Date.now();
                for (let k = 0; k < 40; ++k) { flick.contentY = k * 120; frame(); }
                report("long_scroll_" + count, (Date.now() - t2) / 40);
            }
        }
        function test_editor() {
            mouseMove(scene, scene.width - 2, scene.height - 2, 0, Qt.NoButton);
            let page = null;
            time("page_create", () => { page = createTemporaryObject(editorComponent, scene); });
            wait(200);
            // The palette's later sections build over a few frames; measure after.
            tryVerify(() => !all(page).some(o => o.pendingEntries > 0), 20000);
            const palette = all(page).find(o => typeof o.prefab === "function" && o.resolvedSections !== undefined);
            verify(!!palette);
            const types = Object.keys(BlockRegistry.rows).filter(t => !BlockRegistry.isHeader(t)
                && !/^When|Header|CallBlock|BranchCallBlock|^Test/.test(t) && palette.prefab(t));
            console.log("PERF_INFO types " + types.length);
            const canvas = all(page).find(o => o.strands !== undefined && typeof o.syncStrands === "function");
            verify(!!canvas);
            const counts = [100, 400];
            for (const count of counts) {
                const big = strands(palette, types, count, 25);
                page.appState = scene.state(scene.macro([]));
                frame();
                time("load_" + count, () => { page.appState = scene.state(scene.macro(big)); });
                // One value edited: the backend republishes the whole state.
                time("edit_" + count, i => {
                    const next = JSON.parse(JSON.stringify(big));
                    next[1].instructions[3].id = next[1].instructions[3].id;
                    next[1].instructions[3]["__edit"] = i;
                    page.appState = scene.state(scene.macro(next));
                }, 5);
                // Unchanged republish (status ticks, recording phase).
                time("republish_" + count, () => { page.appState = scene.state(scene.macro(JSON.parse(JSON.stringify(big)))); }, 5);
                // Drag the last block of strand 0 around the canvas.
                const block = all(canvas).find(o => o.instruction && o.instruction.id === "b24" && o.paletteMode === false);
                verify(!!block, "dragged block");
                const p = block.mapToItem(null, 10, 10);
                canvas.beginDrag("s0", [{index: 24}], 1, p.x, p.y, 10, 10, block.width, block.height);
                const t0 = Date.now();
                const moves = 60;
                for (let m = 0; m < moves; ++m) {
                    canvas.moveDrag(p.x + 300 + (m % 30) * 10, p.y - 400 + m * 8);
                    frame();
                }
                report("drag_move_" + count, (Date.now() - t0) / moves);
                canvas.cancelDrag();
                frame();
                // Zoom steps.
                time("zoom_" + count, i => canvas.setZoom(i % 2 ? 0.6 : 1.0), 6);
                canvas.setZoom(1);
            }
            time("unload", () => { page.appState = scene.state(scene.macro([])); });
        }
    }
}
