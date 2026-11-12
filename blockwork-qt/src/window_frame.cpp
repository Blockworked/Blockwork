#include "window_frame.h"

#include <QtCore/QMargins>
#include <QtGui/QGuiApplication>
#include <QtGui/QWindow>

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
#else
    (void)margin;
#endif
}
