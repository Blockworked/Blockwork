#include "translations.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QLocale>
#include <QtCore/QPointer>
#include <QtCore/QTranslator>

namespace {
QPointer<QQmlEngine> uiEngine;
QTranslator *active = nullptr;
}

void blockwork_set_qml_engine(QQmlEngine &engine)
{
    uiEngine = &engine;
}

bool blockwork_set_language(const QString &code)
{
    const QString name = code.isEmpty() ? QLocale::system().name() : code;
    auto *next = new QTranslator(QCoreApplication::instance());
    // Falls back from qml_de_DE to qml_de.
    const bool found = next->load(QStringLiteral("qml_") + name, QStringLiteral(":/i18n"));
    if (active) {
        QCoreApplication::removeTranslator(active);
        active->deleteLater();
        active = nullptr;
    }
    if (found) {
        QCoreApplication::installTranslator(next);
        active = next;
    } else {
        delete next;
    }
    if (uiEngine)
        uiEngine->retranslate();
    return found;
}
