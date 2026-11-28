# Blockwork KWin plugin

Extra input seats for Blockwork's **In window** block, so macros can type and
click into one window on KDE Plasma (Wayland) without moving the user's
focus. See `docs/background-input.md` for what it does and how to build it.

KWin loads it from `<libdir>/qt6/plugins/kwin/plugins/` at startup; it is on
by default once installed. Check it with:

```bash
qdbus6 org.kde.KWin /Plugins org.kde.KWin.Plugins.LoadedPlugins | grep blockwork
```
