#include "application.h"

#include "cxx-qt-lib/qcoreapplication.h"
#include <QtWidgets/QApplication>
#include <QtQml/QQmlContext>

std::unique_ptr<QGuiApplication> blockwork_new_application(const QVector<QByteArray> &args)
{
    auto argsData = std::make_unique<rust::cxxqtlib1::ApplicationArgsData>(args);
    // KDE's native dialog helpers require QApplication, even for a QML UI.
    auto app = std::make_unique<QApplication>(argsData->size(), argsData->data());
    argsData->setParent(app.get());
    argsData.release();
    return app;
}

void blockwork_init_appearance(QQmlEngine &engine, const QString &scheme)
{
    engine.rootContext()->setContextProperty(QStringLiteral("blockworkAppearance"), scheme);
}
