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

#ifdef Q_OS_MACOS
#include <QtCore/QBuffer>
#include <QtCore/QFileInfo>
#include <QtCore/QThread>
#include <QtWidgets/QFileIconProvider>
#endif

QString blockwork_native_app_icon(const QString &command, int size)
{
#ifdef Q_OS_MACOS
    if (!QGuiApplication::instance() || QThread::currentThread() != QGuiApplication::instance()->thread())
        return {};
    QString path = command.trimmed();
    if (path.size() >= 2 && path.startsWith('"') && path.endsWith('"'))
        path = path.mid(1, path.size() - 2);
    const QFileInfo file(path);
    if (path.isEmpty() || !file.exists())
        return {};
    QFileIconProvider provider;
    const QIcon icon = provider.icon(file);
    const int pixels = qBound(1, size, 512);
    const QPixmap pixmap = icon.pixmap(pixels, pixels);
    if (pixmap.isNull())
        return {};
    QByteArray bytes;
    QBuffer buffer(&bytes);
    if (!buffer.open(QIODevice::WriteOnly) || !pixmap.save(&buffer, "PNG"))
        return {};
    return QStringLiteral("data:image/png;base64,") + QString::fromLatin1(bytes.toBase64());
#else
    Q_UNUSED(command);
    Q_UNUSED(size);
    return {};
#endif
}
