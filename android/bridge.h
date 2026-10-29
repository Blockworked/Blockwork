#pragma once
#include <QtCore/QObject>
#include <QtCore/QString>
#include <QtQml/qqmlregistration.h>

class NativeBridge : public QObject {
    Q_OBJECT
    QML_ELEMENT
#define STRING_PROPERTY(name, initial) \
    Q_PROPERTY(QString name MEMBER name NOTIFY name##Changed) \
    QString name = initial; \
    Q_SIGNAL void name##Changed();
#define INT_PROPERTY(name) \
    Q_PROPERTY(int name MEMBER name NOTIFY name##Changed) \
    int name = 0; \
    Q_SIGNAL void name##Changed();
#define BOOL_PROPERTY(name) \
    Q_PROPERTY(bool name MEMBER name NOTIFY name##Changed) \
    bool name = false; \
    Q_SIGNAL void name##Changed();
public:
    STRING_PROPERTY(stateJson, "{}")
    STRING_PROPERTY(lastError, "")
    STRING_PROPERTY(replyKey, "")
    STRING_PROPERTY(replyJson, "null")
    STRING_PROPERTY(replyError, "")
    INT_PROPERTY(replySerial)
    STRING_PROPERTY(previewKey, "")
    STRING_PROPERTY(previewText, "")
    BOOL_PROPERTY(previewError)
    INT_PROPERTY(previewSerial)
    STRING_PROPERTY(activeBlocksJson, "[]")
    STRING_PROPERTY(scheduleStatus, "Checking Android schedules…")
    STRING_PROPERTY(installedAppsJson, "[]")
    STRING_PROPERTY(installedAppsError, "")
    BOOL_PROPERTY(installedAppsLoading)
    INT_PROPERTY(installedAppsSerial)
    explicit NativeBridge(QObject *parent = nullptr);
    ~NativeBridge() override;
    Q_INVOKABLE void invokeCommand(const QString &command, const QString &args);
    Q_INVOKABLE void callCommand(const QString &key, const QString &command, const QString &args);
    Q_INVOKABLE void previewValue(const QString &key, const QString &value);
    Q_INVOKABLE void refreshInstalledApps();
    Q_INVOKABLE QString nativeAppIcon(const QString &) { return {}; }
    Q_INVOKABLE void setWindowFrameMargin(int) {}
    Q_INVOKABLE void importDocument();
    Q_INVOKABLE void exportDocument(const QString &macroId);
    Q_INVOKABLE void accessibilitySettings();
    Q_INVOKABLE void exactAlarmSettings();
    Q_INVOKABLE void backgroundEditor();
    Q_SIGNAL void backRequested();
private:
    void poll();
    uint64_t m_session = 0;
};
#undef STRING_PROPERTY
#undef INT_PROPERTY
#undef BOOL_PROPERTY

// Native picker mailbox, separate from the command bridge/editor session.
class NativeScreenPicker : public QObject {
    Q_OBJECT
    QML_ELEMENT
public:
    explicit NativeScreenPicker(QObject *parent = nullptr);
    Q_INVOKABLE void begin(const QString &mode, const QString &hint);
signals:
    void result(const QString &json);
};
