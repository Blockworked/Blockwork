#include <QtGui/QGuiApplication>
#include <QtQuickControls2/QQuickStyle>
#include <QtQml/QQmlExtensionPlugin>

Q_IMPORT_QML_PLUGIN(com_blockworked_BlockstitchPlugin)
Q_IMPORT_QML_PLUGIN(com_blockworked_BlockworkPlugin)

int main(int argc, char **argv) {
    QQuickStyle::setStyle("BlockworkStyle");
    QGuiApplication app(argc, argv);
    // QtQuickView owns the editor window. The application stays alive when
    // an activity detaches; the independently loaded Rust runtime does too.
    app.setQuitOnLastWindowClosed(false);
    return app.exec();
}
