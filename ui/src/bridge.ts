// When this app is opened as a plain browser tab (e.g. for automated
// testing) instead of inside the real Tauri webview, `window.__TAURI_INTERNALS__`
// doesn't exist, so `@tauri-apps/api`'s invoke()/listen() have nothing to call
// into. This module swaps them for calls to the debug-only HTTP+WebSocket
// bridge (`blockwork-daemon/src/dev_bridge.rs`, only present when the backend is
// built with `cargo run -p blockwork-daemon --features dev-bridge`), which
// forwards to the exact same command handlers and mirrors the app's one
// `state-updated` event.
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen, type Event } from '@tauri-apps/api/event';
import { getVersion as tauriGetVersion } from '@tauri-apps/api/app';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const BRIDGE_ORIGIN = 'http://127.0.0.1:4127';
const BRIDGE_WS = 'ws://127.0.0.1:4127/events';

async function bridgeInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const res = await fetch(`${BRIDGE_ORIGIN}/invoke/${cmd}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(args),
  });
  const body = await res.json();
  if (!body.ok) throw new Error(body.error ?? `dev bridge: ${cmd} failed`);
  return body.data as T;
}

type StateListener = (payload: unknown) => void;
const stateListeners = new Set<StateListener>();
let socket: WebSocket | null = null;

function ensureSocket() {
  if (socket) return;
  socket = new WebSocket(BRIDGE_WS);
  socket.onmessage = evt => {
    const payload = JSON.parse(evt.data);
    stateListeners.forEach(cb => cb(payload));
  };
  socket.onclose = () => {
    socket = null;
    if (stateListeners.size > 0) setTimeout(ensureSocket, 1000);
  };
  socket.onerror = () => socket?.close();
}

async function bridgeListen<T>(event: string, cb: (evt: Event<T>) => void): Promise<() => void> {
  if (event !== 'state-updated') {
    console.warn(`dev bridge: unsupported event "${event}"`);
    return () => {};
  }
  ensureSocket();
  const wrapped: StateListener = payload => cb({ event, id: 0, payload: payload as T });
  stateListeners.add(wrapped);
  return () => stateListeners.delete(wrapped);
}

// Commands the UI process handles itself; everything else is app state and
// lives in the daemon, reached through the UI process's `daemon_call`.
const UI_COMMANDS = new Set(['reset_zoom', 'pick_macro_file', 'set_theme_background']);

function daemonInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (UI_COMMANDS.has(cmd)) return tauriInvoke<T>(cmd, args);
  return tauriInvoke<T>('daemon_call', { cmd, args });
}

async function browserInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  // No native dialogs in a plain browser tab.
  if (cmd === 'pick_macro_file') {
    return (window.prompt('Path to the .macro file', (args.defaultName as string) ?? '') || null) as T;
  }
  if (cmd === 'reset_zoom' || cmd === 'set_theme_background') return undefined as T;
  return bridgeInvoke<T>(cmd, args);
}

export const invoke = isTauri ? daemonInvoke : browserInvoke;
export const listen = isTauri ? tauriListen : bridgeListen;
export const getVersion = isTauri ? tauriGetVersion : async () => 'dev-bridge';
