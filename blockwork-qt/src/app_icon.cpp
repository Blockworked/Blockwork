#include "app_icon.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QDebug>
#include <QtCore/QString>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QWindow>

void blockwork_apply_window_icon()
{
    // Displayed in about dialogs and used as a fallback identifier.
    QCoreApplication::setApplicationName(QStringLiteral("Blockwork"));
    QCoreApplication::setOrganizationName(QStringLiteral("Blockworked"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Blockwork"));
    // Matches res/blockwork.desktop (shipped as com.blockworked.Blockwork on
    // Flatpak) and its StartupWMClass, so Wayland compositors and docks can
    // pair windows with the installed desktop entry and its Icon=blockwork.
    QGuiApplication::setDesktopFileName(QStringLiteral("com.blockworked.Blockwork"));

    // Embedded through CxxQtBuilder::qrc_resources in build.rs. Note: in C++
    // resources are addressed as ":/...", the "qrc:/..." form only works in QML.
    const QIcon icon(QStringLiteral(":/icons/blockwork.png"));
    if (icon.isNull()) {
        qWarning("Blockwork: failed to load the window icon from :/icons/blockwork.png");
        return;
    }
    QGuiApplication::setWindowIcon(icon);
}

void blockwork_apply_window_icon_to_windows()
{
    const QIcon icon = QGuiApplication::windowIcon();
    if (icon.isNull()) {
        return;
    }
    for (QWindow *window : QGuiApplication::allWindows()) {
        if (window->icon().isNull()) {
            window->setIcon(icon);
        }
    }
}
