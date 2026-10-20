#include "translations.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QDir>
#include <QtCore/QLocale>
#include <QtCore/QPointer>
#include <QtCore/QTranslator>

namespace {
QPointer<QQmlEngine> uiEngine;
QTranslator *active = nullptr;
QTranslator *english = nullptr;
}

void blockwork_set_qml_engine(QQmlEngine &engine)
{
    uiEngine = &engine;
}

bool blockwork_set_language(const QString &code)
{
    // Untranslated plural messages must still use English singular/plural
    // forms, even for an unsupported system locale (including the C locale).
    if (!english) {
        english = new QTranslator(QCoreApplication::instance());
        if (english->load(QStringLiteral("qml_en"), QStringLiteral(":/i18n")))
            QCoreApplication::installTranslator(english);
    }
    auto *next = new QTranslator(QCoreApplication::instance());
    // Locale-aware lookup also finds script catalogs such as zh_Hans for zh_CN.
    // Explicit codes include custom languages such as "pirate".
    const bool found = code.isEmpty()
        ? next->load(QLocale::system(), QStringLiteral("qml"), QStringLiteral("_"), QStringLiteral(":/i18n"))
        : next->load(QStringLiteral("qml_") + code, QStringLiteral(":/i18n"));
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

QStringList blockwork_languages()
{
    QStringList codes;
    const auto files = QDir(QStringLiteral(":/i18n")).entryList({QStringLiteral("qml_*.qm")}, QDir::Files, QDir::Name);
    for (const QString &file : files)
        codes << file.mid(4, file.size() - 7);
    return codes;
}
