// Gives Blockwork the window list and window actions over D-Bus, since
// GNOME on Wayland keeps them from other apps. Only the session bus can
// reach it, and it does nothing a window's own title bar can't.
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'com.blockworked.Windows';
const OBJECT_PATH = '/com/blockworked/Windows';
const INTERFACE = `<node>
  <interface name="com.blockworked.Windows">
    <method name="List">
      <arg type="s" direction="out" name="windows"/>
    </method>
    <method name="Act">
      <arg type="u" direction="in" name="id"/>
      <arg type="s" direction="in" name="action"/>
      <arg type="d" direction="in" name="a"/>
      <arg type="d" direction="in" name="b"/>
    </method>
    <property name="Version" type="u" access="read"/>
  </interface>
</node>`;

// App windows, front to back.
function windows() {
    const all = global.get_window_actors()
        .map(actor => actor.meta_window)
        .filter(w => w && !w.skip_taskbar &&
            [Meta.WindowType.NORMAL, Meta.WindowType.DIALOG, Meta.WindowType.MODAL_DIALOG].includes(w.get_window_type()));
    return global.display.sort_windows_by_stacking(all).reverse();
}

function appName(win) {
    const app = Shell.WindowTracker.get_default().get_window_app(win);
    return (app && app.get_name()) || win.get_wm_class() || '';
}

function find(id) {
    const win = windows().find(w => w.get_id() === id);
    if (!win)
        throw new Error('the window closed');
    return win;
}

function maximized(win) {
    if (typeof win.is_maximized === 'function')
        return win.is_maximized();
    return win.get_maximized() === Meta.MaximizeFlags.BOTH;
}

// GNOME 49 dropped the flags argument.
function setMaximized(win, on) {
    const call = on ? 'maximize' : 'unmaximize';
    if (win[call].length > 0)
        win[call](Meta.MaximizeFlags.BOTH);
    else
        win[call]();
}

class Service {
    get Version() {
        return 1;
    }

    List() {
        const focus = global.display.focus_window;
        return JSON.stringify(windows().map(win => {
            const rect = win.get_frame_rect();
            const actor = win.get_compositor_private();
            return {
                id: win.get_id(),
                title: win.get_title() || '',
                app: appName(win),
                class: win.get_wm_class() || '',
                pid: win.get_pid(),
                x: rect.x, y: rect.y, width: rect.width, height: rect.height,
                focused: win === focus,
                minimized: win.minimized,
                maximized: maximized(win),
                above: win.is_above(),
                opacity: actor ? actor.opacity / 255 : 1,
                monitor: win.get_monitor(),
            };
        }));
    }

    Act(id, action, a, b) {
        const win = find(id);
        const time = global.get_current_time();
        switch (action) {
        case 'Focus':
        case 'Show':
            if (win.minimized)
                win.unminimize();
            win.activate(time);
            break;
        case 'Minimize':
        case 'Hide':
            win.minimize();
            break;
        case 'Maximize':
            setMaximized(win, true);
            break;
        case 'Restore':
            if (win.minimized)
                win.unminimize();
            if (maximized(win))
                setMaximized(win, false);
            if (win.is_fullscreen())
                win.unmake_fullscreen();
            break;
        case 'Close':
            win.delete(time);
            break;
        case 'AlwaysOnTop':
            win.make_above();
            break;
        case 'NotOnTop':
            win.unmake_above();
            break;
        case 'Move':
            win.move_frame(true, Math.round(a), Math.round(b));
            break;
        case 'Resize': {
            const rect = win.get_frame_rect();
            win.move_resize_frame(true, rect.x, rect.y, Math.round(a), Math.round(b));
            break;
        }
        case 'Opacity': {
            const actor = win.get_compositor_private();
            if (actor)
                actor.opacity = Math.round(Math.max(0, Math.min(1, a)) * 255);
            break;
        }
        default:
            throw new Error(`unknown window action ${action}`);
        }
    }
}

export default class BlockworkWindows extends Extension {
    enable() {
        this._object = Gio.DBusExportedObject.wrapJSObject(INTERFACE, new Service());
        this._object.export(Gio.DBus.session, OBJECT_PATH);
        this._owner = Gio.bus_own_name_on_connection(Gio.DBus.session, BUS_NAME,
            Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        Gio.bus_unown_name(this._owner);
        this._object.unexport();
        this._object = null;
    }
}
