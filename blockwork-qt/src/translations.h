#pragma once

#include <QtCore/QString>
#include <QtCore/QStringList>
#include <QtQml/QQmlEngine>

// Remembers the engine to retranslate when the language changes.
void blockwork_set_qml_engine(QQmlEngine &engine);

// Installs :/i18n/qml_<code>.qm (or the system language's when empty) and
// retranslates. False when there's no such file, which means English.
bool blockwork_set_language(const QString &code);

// Codes of every compiled translation, like "pirate" or "pt_BR".
QStringList blockwork_languages();
