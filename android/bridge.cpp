#include "bridge.h"
#include "runtime.h"
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTimer>
#include <QtCore/QJniObject>
#include <QtCore/QCoreApplication>
#include <QtGui/QGuiApplication>
#include <QtGui/QInputMethod>
#include <QtGui/QKeyEvent>
#include <QtGui/QWindow>

static QJsonObject take(char *raw) {
    const auto object = QJsonDocument::fromJson(raw ? QByteArray(raw) : QByteArray()).object();
    blockwork_android_free(raw);
    return object;
}
static QString encode(const QJsonValue &value) {
    if (value.isObject()) return QString::fromUtf8(QJsonDocument(value.toObject()).toJson(QJsonDocument::Compact));
    if (value.isArray()) return QString::fromUtf8(QJsonDocument(value.toArray()).toJson(QJsonDocument::Compact));
    auto wrapped = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(wrapped.mid(1, wrapped.size() - 2));
}
static QJsonObject call(const QString &command, const QString &args) {
    return take(blockwork_android_call(command.toUtf8().constData(), args.toUtf8().constData()));
}
NativeBridge::NativeBridge(QObject *parent) : QObject(parent) {
    const auto result = take(blockwork_android_attach());
    if (!result["ok"].toBool()) { lastError = result["error"].toString(); return; }
    m_session = result["result"].toInteger();
    auto timer = new QTimer(this);
    connect(timer, &QTimer::timeout, this, &NativeBridge::poll);
    timer->start(33);
    poll();
}
NativeBridge::~NativeBridge() {
    if (m_session) take(blockwork_android_detach(m_session));
}
void NativeBridge::poll() {
    if (QJniObject::callStaticMethod<jboolean>("com/blockworked/blockwork/BlockworkActivity", "takeBackPress", "()Z")) {
        if (QGuiApplication::inputMethod()->isVisible()) QGuiApplication::inputMethod()->hide();
        else {
            // Escape follows Qt's popup stack, including nested menus and
            // dialogs whose rejection must run before closing their sheet.
            bool handled = false;
            if (auto window = QGuiApplication::focusWindow()) {
                QKeyEvent press(QEvent::KeyPress, Qt::Key_Escape, Qt::NoModifier);
                press.setAccepted(false);
                QCoreApplication::sendEvent(window, &press);
                handled = press.isAccepted();
                QKeyEvent release(QEvent::KeyRelease, Qt::Key_Escape, Qt::NoModifier);
                QCoreApplication::sendEvent(window, &release);
            }
            if (!handled) emit backRequested();
        }
    }
    const auto status = QJniObject::callStaticObjectMethod("com/blockworked/blockwork/ScheduleManager", "currentStatus", "()Ljava/lang/String;").toString();
    if (scheduleStatus != status) { scheduleStatus = status; emit scheduleStatusChanged(); }
    const auto appsRaw = QJniObject::callStaticObjectMethod("com/blockworked/blockwork/AndroidApps", "takeInstalledApps", "()Ljava/lang/String;").toString();
    if (!appsRaw.isEmpty()) {
        const auto apps = QJsonDocument::fromJson(appsRaw.toUtf8()).object();
        installedAppsLoading = false;
        installedAppsError = apps["error"].toString();
        if (apps["ok"].toBool()) installedAppsJson = encode(apps["result"]);
        ++installedAppsSerial;
        emit installedAppsLoadingChanged(); emit installedAppsErrorChanged(); emit installedAppsJsonChanged(); emit installedAppsSerialChanged();
    }
    const auto reply = take(blockwork_android_poll(m_session));
    if (!reply["ok"].toBool()) { lastError = reply["error"].toString(); emit lastErrorChanged(); return; }
    for (const auto &value : reply["result"].toArray()) {
        const auto event = value.toObject();
        if (event["kind"] == "state") { stateJson = encode(event["data"]); emit stateJsonChanged(); }
        else if (event["kind"] == "trace") { activeBlocksJson = encode(event["data"]); emit activeBlocksJsonChanged(); }
    }
}
void NativeBridge::invokeCommand(const QString &command, const QString &args) {
    const auto reply = call(command, args);
    if (!reply["ok"].toBool()) { lastError = reply["error"].toString(); emit lastErrorChanged(); }
}
void NativeBridge::callCommand(const QString &key, const QString &command, const QString &args) {
    const auto reply = call(command, args);
    replyKey = key;
    replyJson = encode(reply["result"]);
    replyError = reply["error"].toString();
    ++replySerial;
    emit replyKeyChanged(); emit replyJsonChanged(); emit replyErrorChanged(); emit replySerialChanged();
}
void NativeBridge::previewValue(const QString &key, const QString &value) {
    const auto reply = call("preview_value", "{\"value\":" + value + "}");
    previewKey = key;
    previewError = !reply["ok"].toBool();
    previewText = previewError ? reply["error"].toString() : reply["result"].toString();
    ++previewSerial;
    emit previewKeyChanged(); emit previewTextChanged(); emit previewErrorChanged(); emit previewSerialChanged();
}
void NativeBridge::importDocument() {
    QJniObject::callStaticMethod<void>("com/blockworked/blockwork/BlockworkActivity", "importDocument", "()V");
}
void NativeBridge::exportDocument(const QString &macroId) {
    const auto id = QJniObject::fromString(macroId);
    QJniObject::callStaticMethod<void>("com/blockworked/blockwork/BlockworkActivity", "exportDocument", "(Ljava/lang/String;)V", id.object<jstring>());
}
void NativeBridge::accessibilitySettings() {
    QJniObject::callStaticMethod<void>("com/blockworked/blockwork/BlockworkActivity", "accessibilitySettings", "()V");
}

NativeScreenPicker::NativeScreenPicker(QObject *parent) : QObject(parent) {
    auto timer = new QTimer(this);
    connect(timer, &QTimer::timeout, this, [this]() {
        const auto pending = QJniObject::callStaticObjectMethod(
            "com/blockworked/blockwork/ScreenPicker", "takeResult", "()Ljava/lang/String;").toString();
        if (!pending.isEmpty()) emit result(pending);
    });
    timer->start(50);
}
void NativeScreenPicker::begin(const QString &mode, const QString &hint) {
    const auto jmode = QJniObject::fromString(mode);
    const auto jhint = QJniObject::fromString(hint);
    QJniObject::callStaticMethod<void>("com/blockworked/blockwork/ScreenPicker", "begin",
        "(Ljava/lang/String;Ljava/lang/String;)V", jmode.object<jstring>(), jhint.object<jstring>());
}

void NativeBridge::refreshInstalledApps() {
    if (installedAppsLoading) return;
    installedAppsLoading = true; installedAppsError.clear();
    emit installedAppsLoadingChanged(); emit installedAppsErrorChanged();
    QJniObject::callStaticMethod<void>("com/blockworked/blockwork/AndroidApps", "requestInstalledApps", "()V");
}

void NativeBridge::exactAlarmSettings() {
    QJniObject::callStaticMethod<void>("com/blockworked/blockwork/BlockworkActivity", "exactAlarmSettings", "()V");
}

void NativeBridge::backgroundEditor() {
    QJniObject::callStaticMethod<void>("com/blockworked/blockwork/BlockworkActivity", "backgroundEditor", "()V");
}
