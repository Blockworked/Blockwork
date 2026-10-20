#include "window_frame.h"

#include <QtCore/QMargins>
#include <QtGui/QGuiApplication>
#include <QtGui/QWindow>

#if defined(Q_OS_WIN)
#include <QtCore/qt_windows.h>
#endif

#if defined(__linux__) && __has_include(<QtGui/qpa/qplatformwindow_p.h>)
#include <QtGui/qpa/qplatformwindow_p.h>
#endif

void blockwork_set_window_frame_margin(int margin)
{
#if defined(__linux__) && __has_include(<QtGui/qpa/qplatformwindow_p.h>) && QT_CONFIG(wayland)
    const QMargins margins(margin, margin, margin, margin);
    for (QWindow *window : QGuiApplication::topLevelWindows()) {
        if (!(window->flags() & Qt::FramelessWindowHint)) {
            continue;
        }
        window->create();
        if (auto *wl = window->nativeInterface<QNativeInterface::Private::QWaylandWindow>()) {
            wl->setCustomMargins(margins);
        }
    }
#elif defined(Q_OS_WIN)
    (void)margin;
    for (QWindow *window : QGuiApplication::topLevelWindows()) {
        // Only the editor opts in; overlays must remain plain frameless windows.
        if (window->objectName() != QStringLiteral("blockworkMainWindow")) {
            continue;
        }
        window->create();
        const HWND hwnd = reinterpret_cast<HWND>(window->winId());
        const LONG_PTR style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        // Qt removes WS_THICKFRAME for FramelessWindowHint. Windows needs it
        // for native edge snapping even though Qt still draws no native frame.
        const LONG_PTR snapStyle = style | WS_THICKFRAME | WS_MINIMIZEBOX
            | WS_MAXIMIZEBOX | WS_SYSMENU;
        if (style != snapStyle) {
            SetWindowLongPtrW(hwnd, GWL_STYLE, snapStyle);
            SetWindowPos(hwnd, nullptr, 0, 0, 0, 0,
                         SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER
                         | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
    }
#else
    (void)margin;
#endif
}
