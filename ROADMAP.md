# Roadmap

Goal: a solid general automation app. Today Blockwork is a strong input macro
tool (record/playback, hotkeys, variables, lists, custom blocks, time/battery/
clipboard triggers). Getting to "general automation" means three things: see
the screen and system, talk to the outside world, and be trustworthy when run
unattended.

Items are ordered by priority inside each phase. Sizes: S (days), M (1-2
weeks), L (multi-week). Finished items are marked (done).

## TODO before the next release

- **Windows and macOS support.** Many blocks were only tested on Linux:
  window control, OCR, image capture, the screen picker, USB/network/mouse
  button triggers, dialogs (`blockwork --prompt`), detection and the webhook.
  Run each on Windows and macOS, fix what breaks, and make every block that
  can't work on a platform say so in its explainer and with a clear run log
  error instead of failing quietly. Write the result down as a support table
  per block (see "macOS and Windows parity audit" below).
- **Docs.** User docs for the block vocabulary, triggers, permissions,
  webhook, bundles and the block library; and the embedding SDK docs for
  `blockwork-ffi` (lockstep host clock, GDR import, step-stamped recording)
  with a worked example (see "Embedding SDK docs" below).
- **Translations.** Translators work on a self-hosted Weblate; stand up
  the server and connect the repo (see `packaging/weblate/README.md`).
  Real languages should come from people, not machine translation. Still
  English only: run log messages, block
  runtime errors, the tray menu and template names, which come from the
  daemon.

## Phase 1 - Reliability and debuggability

Automation people leave running has to be predictable first.

- **Run log and live trace (M, done).** Per-run log of which block ran, values of
  variables, errors, timestamps. Highlight the active block on the canvas
  while running.
- **Step / pause / breakpoints (M, done).** Single-step a strand, pause on a block,
  inspect variables.
- **Error handling blocks (S, done).** `Try / On error`, `Stop macro`, `Retry N
  times`, and a per-macro "on failure" strand. Today a failure just ends the
  run.
- **Global kill switch and run limits (S, done).** Configurable panic hotkey, max
  runtime, max loop iterations for unattended macros.
- **Macro library management (S, done).** Folders/tags, search, duplicate, import/
  export single macros, bundle with custom blocks.
- **Undo-safe migration and versioned files (S, done).** Schema version in macro
  files with tested migrations so upgrades never break saved work.
- **Wayland/Linux input hardening (M).** Keep closing gaps in the libei/evdev
  paths (held keys, absolute pointer, per-compositor quirks) with an
  integration test matrix.

## Phase 2 - Perception (react to what is on screen)

The biggest gap versus tools like AutoHotkey, Sikuli, or Power Automate.

- **Screen pixel / color reporters (M, done).** `pixel color at (x, y)`, wait until
  color changes, region average color.
- **Image match (L, done).** Find a template image on screen (with tolerance and
  region), returns found + position. Block for click-on-image. Include a
  capture-region picker in the UI.
- **OCR (M, done).** Read text from a screen region into a variable; `wait until
  text appears`.
- **Active window info (M, done).** Reporters for active window title/app/position;
  `When window focused/opened/closed` triggers; window blocks (move, resize,
  focus, minimize, close).
- **Cursor position and screen geometry reporters (S, done).** Multi-monitor aware.

## Phase 3 - Reach the outside world

- **File blocks (M, done).** Read/write/append text files, list a directory, file
  exists, `When file changes` trigger.
- **HTTP request block (M, done).** GET/POST with headers and body, parse JSON
  response into variables/lists; `When webhook received` trigger on a local
  port (opt-in).
- **Run program / shell with output (S, done).** Capture stdout and exit code into
  variables (currently `Command` is fire and forget).
- **Notifications and dialogs (S, done).** Desktop notification block, prompt for
  text input, confirm dialog, toast on macro finish.
- **Text and data tools (M, done).** String operators (join, split, replace, regex
  match, length), number formatting, JSON get, dates arithmetic.
- **Secrets store (S, done).** Named secrets kept in the OS keyring, usable in
  blocks but never saved in macro files or logs.

## Phase 4 - More triggers and composition

- **More triggers (M, done).** `When window opens`, `When app starts/exits`,
  `When network connects/disconnects`, `When USB/device connected`, `When
  idle for N`, `When screen locked/unlocked`, `When mouse button/gesture`.
- **Macro-to-macro calls (M, done).** `Run macro X (with args)`, `Stop macro X`,
  `Is macro X running` reporter; shared variables with clear scope.
- **Persistent variables (S, done).** Opt-in variables that survive restarts
  (counters, state), plus global vs per-macro scope.
- **Conditions on triggers (S, done).** Per-macro enable rules: only when app X is
  focused, only on schedule windows, only on AC power.
- **Parallel strand controls (S, done).** Named locks / mutex block, `wait for
  strand`, cooperative cancel, so concurrent entry points do not fight over
  input.
- **Richer scheduling (S, done).** Cron-style expressions, "once at", missed-run
  catch-up policy.

## Phase 5 - Sharing and ecosystem

- **Template gallery (M, done).** Built-in starter macros (auto clicker, text
  expander, app launcher, break reminder) shown in onboarding.
- **Import / export bundles (S, done).** `.blockwork` package of macros + custom
  blocks, with a review screen listing what a macro can do (input, shell,
  network, files) before first run.
- **Community block library (L, done).** Shared custom block packs fetched from a
  git/URL index, signed or hash-pinned.
- **Text expansion and snippets (M, done).** Typed-abbreviation trigger with
  clipboard-safe paste, a very common automation use case.
- **CLI (S, done).** `blockwork run <macro>`, `list`, `stop` through the existing
  daemon IPC for scripting and launchers.

## Phase 6 - Platform and polish

- **Permission model (M, done).** Capability prompts per macro (shell, network,
  file write, screen capture) so imported macros cannot silently do harm.
- **Accessibility pass (M).** Keyboard-only canvas navigation, screen reader
  labels, high contrast theme.
- **Localization (M, done).** Extract strings from QML and block labels.
- **Mobile/remote companion (L, speculative).** Trigger macros from a phone
  over LAN through the daemon.
- **Embedding SDK docs (S).** Document `blockwork-ffi` for host apps (games,
  tools) with a worked example.
- **macOS and Windows parity audit (M).** Per-feature support matrix; every
  new block states what it needs on each OS.

## Suggested order

1. Run log + error handling + kill switch (Phase 1): makes everything after
   it testable and safe.
2. File/HTTP/shell-output/notification blocks (Phase 3, small items): the
   cheapest jump from "macro recorder" to "automation".
3. Active window info and triggers (Phase 2/4): unlocks context-aware macros.
4. Pixel reporters, then image match and OCR.
5. Macro-to-macro calls, persistent variables, templates, CLI.

## Principles

- Every new block is available as a reporter or command that composes with
  existing `Value` operators; no one-off UI.
- Features that cannot work on a platform (Wayland screen capture, etc.)
  degrade with a clear message, not silently.
- Anything that can touch files, network, or run programs must be visible to
  the user before an imported macro runs.
