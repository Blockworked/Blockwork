#include <QtQuick/QQuickWindow>
#include <QtQuickTest/quicktest.h>
#include <QWKQuick/qwkquickglobal.h>

class WindowTestSetup : public QObject {
    Q_OBJECT
public slots:
    void applicationAvailable() { QQuickWindow::setDefaultAlphaBuffer(true); }
    void qmlEngineAvailable(QQmlEngine *engine) { QWK::registerTypes(engine); }
};

QUICK_TEST_MAIN_WITH_SETUP(blockwork_window, WindowTestSetup)
#include "main.moc"
