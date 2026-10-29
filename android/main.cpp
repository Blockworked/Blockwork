#include <QtGui/QGuiApplication>
#include <QtGui/QFontDatabase>
#include <QtGui/QFont>
#include <QtQuickControls2/QQuickStyle>
#include <QtQml/QQmlExtensionPlugin>

Q_IMPORT_QML_PLUGIN(com_blockworked_BlockstitchPlugin)
Q_IMPORT_QML_PLUGIN(com_blockworked_BlockworkPlugin)

int main(int argc, char **argv) {
    QQuickStyle::setStyle("BlockworkStyle");
    QGuiApplication app(argc, argv);
    QString family;
    for (const char *name : {"Regular", "Medium", "SemiBold", "Bold", "ExtraBold", "Italic"}) {
        const int id = QFontDatabase::addApplicationFont(QStringLiteral(":/fonts/AtkinsonHyperlegibleNext-%1.ttf").arg(QLatin1String(name)));
        if (id >= 0 && family.isEmpty()) family = QFontDatabase::applicationFontFamilies(id).value(0);
    }
    if (!family.isEmpty()) {
        QFont font = app.font();
        font.setFamilies({family});
        app.setFont(font);
    }
    // QtQuickView owns the editor window. The application stays alive when
    // an activity detaches; the independently loaded Rust runtime does too.
    app.setQuitOnLastWindowClosed(false);
    return app.exec();
}
