#include "qt_diagnostics.h"

#include <QtCore/QByteArray>
#include <QtCore/QString>
#include <QtCore/QtLogging>
#include <QtQuickControls2/QQuickStyle>

#include <cstdio>

namespace {
void blockwork_qt_message_handler(
    QtMsgType type,
    const QMessageLogContext &,
    const QString &message)
{
    const QByteArray encoded = message.toLocal8Bit();
    std::fprintf(stderr, "Qt[%d]: %s\n", static_cast<int>(type), encoded.constData());
    std::fflush(stderr);
}
} // namespace

void install_qt_message_handler()
{
#if defined(Q_OS_MACOS) && QT_VERSION >= QT_VERSION_CHECK(6, 10, 0) && QT_VERSION < QT_VERSION_CHECK(6, 11, 0)
    // Native Qt 6.10 CI crashes in incremental GC while incubating QML
    // components. Keep collection enabled, but complete it synchronously.
    // Preserve explicit overrides for diagnosis of the upstream failure.
    if (!qEnvironmentVariableIsSet("QV4_GC_TIMELIMIT"))
        qputenv("QV4_GC_TIMELIMIT", "0");
#endif
    QQuickStyle::setStyle(QStringLiteral("BlockworkStyle"));
    QQuickStyle::setFallbackStyle(QStringLiteral("Basic"));
    qInstallMessageHandler(blockwork_qt_message_handler);
}
