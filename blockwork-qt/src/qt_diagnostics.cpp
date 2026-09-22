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
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    qInstallMessageHandler(blockwork_qt_message_handler);
}
