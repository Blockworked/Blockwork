#include "application.h"

#include "cxx-qt-lib/qcoreapplication.h"
#include <QtGui/QFont>
#include <QtGui/QFontDatabase>
#include <QtWidgets/QApplication>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickWindow>

#include <QWKQuick/qwkquickglobal.h>

// Blockwork's own typeface (Atkinson Hyperlegible Next, OFL, res/fonts), so the
// app reads the same on every desktop. Falls back to the platform font.
static void blockwork_use_bundled_font(QGuiApplication &app)
{
    static const char *const files[] = {"Regular", "Medium", "SemiBold", "Bold", "ExtraBold", "Italic"};
    QString family;
    for (const char *name : files) {
        const int id = QFontDatabase::addApplicationFont(QStringLiteral(":/fonts/AtkinsonHyperlegibleNext-%1.ttf").arg(QLatin1String(name)));
        if (id >= 0 && family.isEmpty())
            family = QFontDatabase::applicationFontFamilies(id).value(0);
    }
    if (family.isEmpty()) {
        qWarning("Blockwork: bundled font missing from :/fonts, using the platform font");
        return;
    }
    QFont font = app.font();
    font.setFamilies({family});
    app.setFont(font);
}

std::unique_ptr<QGuiApplication> blockwork_new_application(const QVector<QByteArray> &args)
{
    auto argsData = std::make_unique<rust::cxxqtlib1::ApplicationArgsData>(args);
    QQuickWindow::setDefaultAlphaBuffer(true);
    // KDE's native dialog helpers require QApplication, even for a QML UI.
    auto app = std::make_unique<QApplication>(argsData->size(), argsData->data());
    argsData->setParent(app.get());
    argsData.release();
    blockwork_use_bundled_font(*app);
    return app;
}

void blockwork_init_appearance(QQmlEngine &engine, const QString &scheme)
{
    engine.rootContext()->setContextProperty(QStringLiteral("blockworkAppearance"), scheme);
    QWK::registerTypes(&engine);
}
