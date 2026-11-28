// SPDX-License-Identifier: GPL-3.0-or-later

#include "seathost.h"

#include "blockwork-seat-host-v1-server.h"

#include <config-kwin.h>
#include <input.h>
#include <input_event.h>
#include <wayland/clientconnection.h>
#include <wayland/display.h>
#include <wayland/keyboard.h>
#include <wayland/pointer.h>
#include <wayland/seat.h>
#include <wayland/surface.h>
#include <wayland_server.h>
#include <window.h>
#include <workspace.h>

#include <QTimer>
#include <QUuid>

#include <xkbcommon/xkbcommon.h>

#include <sys/mman.h>
#include <unistd.h>

#include <cstring>

namespace {

constexpr int LaneCount = 4;
/// Blockwork waits two seconds for bound; answer before it gives up.
constexpr int BindTimeoutMs = 1500;
constexpr int BindCheckMs = 10;
/// Longest wait for an app's sync after keyboard enter.
constexpr int SyncWaitMs = 100;
/// At most one preempted event per lane this often.
constexpr int PreemptEveryMs = 100;
/// Biggest keymap a client may send.
constexpr uint32_t MaxKeymapBytes = 1 << 20;
/// Sandboxed clients may only bind as this app.
const QString BlockworkAppId = QStringLiteral("com.blockworked.Blockwork");

KWin::BlockworkSeatHost *s_host = nullptr;

using Lane = KWin::BlockworkSeatHost::Lane;

Lane *laneOf(wl_resource *resource)
{
    return static_cast<Lane *>(wl_resource_get_user_data(resource));
}

// Requests on an inert lane (no free lane, or plugin gone) do nothing.
#define WITH_LANE(resource, call)              \
    do {                                       \
        if (auto *lane = laneOf(resource)) {   \
            if (s_host) {                      \
                s_host->call;                  \
            }                                  \
        }                                      \
    } while (0)

void laneDestroy(wl_client *, wl_resource *resource)
{
    wl_resource_destroy(resource);
}

void laneBind(wl_client *, wl_resource *resource, const char *window)
{
    WITH_LANE(resource, bindLane(*lane, window));
}

void laneSetKeymap(wl_client *, wl_resource *resource, int32_t fd, uint32_t size)
{
    if (auto *lane = laneOf(resource); lane && s_host) {
        s_host->setKeymap(*lane, fd, size);
    }
    close(fd);
}

void laneKey(wl_client *, wl_resource *resource, uint32_t key, uint32_t state)
{
    WITH_LANE(resource, key(*lane, key, state));
}

void laneMotion(wl_client *, wl_resource *resource, wl_fixed_t x, wl_fixed_t y)
{
    WITH_LANE(resource, motion(*lane, QPointF(wl_fixed_to_double(x), wl_fixed_to_double(y))));
}

void laneRelative(wl_client *, wl_resource *resource, wl_fixed_t dx, wl_fixed_t dy)
{
    WITH_LANE(resource, relativeMotion(*lane, QPointF(wl_fixed_to_double(dx), wl_fixed_to_double(dy))));
}

void laneButton(wl_client *, wl_resource *resource, uint32_t button, uint32_t state)
{
    WITH_LANE(resource, button(*lane, button, state));
}

void laneAxis(wl_client *, wl_resource *resource, uint32_t axis, int32_t value120)
{
    WITH_LANE(resource, axis(*lane, axis, value120));
}

void laneReleaseAll(wl_client *, wl_resource *resource)
{
    WITH_LANE(resource, releaseAll(*lane));
}

const struct blockwork_seat_lane_v1_interface laneRequests = {
    .destroy = laneDestroy,
    .bind = laneBind,
    .set_keymap = laneSetKeymap,
    .key = laneKey,
    .motion = laneMotion,
    .relative_motion = laneRelative,
    .button = laneButton,
    .axis = laneAxis,
    .release_all = laneReleaseAll,
};

// Runs for an explicit destroy and when the client disconnects.
void laneResourceGone(wl_resource *resource)
{
    if (auto *lane = laneOf(resource); lane && s_host) {
        s_host->giveBack(*lane);
    }
}

void hostDestroy(wl_client *, wl_resource *resource)
{
    wl_resource_destroy(resource);
}

void hostGetLane(wl_client *, wl_resource *resource, uint32_t id)
{
    if (s_host) {
        s_host->getLane(resource, id);
    }
}

const struct blockwork_seat_host_v1_interface hostRequests = {
    .destroy = hostDestroy,
    .get_lane = hostGetLane,
};

} // namespace

namespace KWin {

BlockworkSeatHost::BlockworkSeatHost()
{
    s_host = this;
    m_clock.start();
    m_xkbContext = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    xkb_rule_names us{};
    us.model = "pc105";
    us.layout = "us";
    m_defaultKeymap = m_xkbContext ? xkb_keymap_new_from_names(m_xkbContext, &us, XKB_KEYMAP_COMPILE_NO_FLAGS) : nullptr;
    if (!m_defaultKeymap) {
        qWarning("Blockwork background input: no US keymap (is xkeyboard-config installed?); not starting");
        return;
    }
    if (char *text = xkb_keymap_get_as_string(m_defaultKeymap, XKB_KEYMAP_FORMAT_TEXT_V1)) {
        m_defaultKeymapText = QByteArray(text);
        free(text);
    }

    wl_display *display = *waylandServer()->display();
    m_logger = wl_display_add_protocol_logger(display, &BlockworkSeatHost::logRequest, this);
    for (int index = 0; index < LaneCount; ++index) {
        if (!makeLane(index)) {
            break;
        }
    }
    m_global = wl_global_create(display, &blockwork_seat_host_v1_interface, 1, this, &BlockworkSeatHost::bindHost);
    input()->installInputEventSpy(this);
}

BlockworkSeatHost::~BlockworkSeatHost()
{
    input()->uninstallInputEventSpy(this);
    if (m_global) {
        wl_global_destroy(m_global);
    }
    for (auto &lane : m_lanes) {
        if (lane->owner) {
            wl_resource_set_user_data(lane->owner, nullptr);
            giveBack(*lane);
        }
        if (lane->xkb) {
            xkb_state_unref(lane->xkb);
        }
    }
    m_lanes.clear();
    if (m_logger) {
        wl_protocol_logger_destroy(m_logger);
    }
    if (m_defaultKeymap) {
        xkb_keymap_unref(m_defaultKeymap);
    }
    if (m_xkbContext) {
        xkb_context_unref(m_xkbContext);
    }
    s_host = nullptr;
}

bool BlockworkSeatHost::makeLane(int index)
{
    auto lane = std::make_unique<Lane>();
    lane->seat = std::make_unique<SeatInterface>(waylandServer()->display(), QStringLiteral("blockwork-%1").arg(index), this);
    lane->seat->setHasPointer(true);
    lane->seat->setHasKeyboard(true);
    // Key repeat as the user set it, so a held key repeats the same way.
    auto *primary = waylandServer()->seat()->keyboard();
    lane->seat->keyboard()->setRepeatInfo(primary->keyRepeatRate(), primary->keyRepeatDelay());
    setDefaultKeymap(*lane);
    if (!lane->xkb) {
        return false;
    }
    lane->binding = new QTimer(this);
    lane->binding->setInterval(BindCheckMs);
    Lane *raw = lane.get();
    connect(lane->binding, &QTimer::timeout, this, [this, raw] {
        checkBinding(*raw);
    });
    m_lanes.push_back(std::move(lane));
    return true;
}

void BlockworkSeatHost::setDefaultKeymap(Lane &lane)
{
    lane.seat->keyboard()->setKeymap(m_defaultKeymapText);
    if (lane.xkb) {
        xkb_state_unref(lane.xkb);
    }
    lane.xkb = xkb_state_new(m_defaultKeymap);
    lane.customKeymap = false;
}

bool BlockworkSeatHost::allowed(wl_client *client) const
{
    auto *connection = ClientConnection::get(client);
    if (!connection) {
        return false;
    }
    // Outside a sandbox any client could already drive input another way.
    return !connection->isSandboxed() || connection->securityContextAppId() == BlockworkAppId;
}

void BlockworkSeatHost::bindHost(wl_client *client, void *data, uint32_t version, uint32_t id)
{
    auto *self = static_cast<BlockworkSeatHost *>(data);
    if (!self->allowed(client)) {
        wl_client_post_implementation_error(client, "blockwork_seat_host_v1 is only for Blockwork");
        return;
    }
    wl_resource *resource = wl_resource_create(client, &blockwork_seat_host_v1_interface, version, id);
    if (!resource) {
        wl_client_post_no_memory(client);
        return;
    }
    wl_resource_set_implementation(resource, &hostRequests, self, nullptr);
    blockwork_seat_host_v1_send_info(resource, KWIN_PLUGIN_VERSION_STRING, BLOCKWORK_KWIN_BUILD, self->m_lanes.size());
}

void BlockworkSeatHost::getLane(wl_resource *host, uint32_t id)
{
    wl_client *client = wl_resource_get_client(host);
    wl_resource *resource = wl_resource_create(client, &blockwork_seat_lane_v1_interface, wl_resource_get_version(host), id);
    if (!resource) {
        wl_client_post_no_memory(client);
        return;
    }
    Lane *free = nullptr;
    for (auto &lane : m_lanes) {
        if (!lane->owner) {
            free = lane.get();
            break;
        }
    }
    wl_resource_set_implementation(resource, &laneRequests, free, laneResourceGone);
    if (!free) {
        blockwork_seat_lane_v1_send_refused(resource, BLOCKWORK_SEAT_LANE_V1_REASON_NO_FREE_LANE,
                                            "every background input lane is in use: stop another macro that uses In window, then try again");
        return;
    }
    free->owner = resource;
}

void BlockworkSeatHost::bindLane(Lane &lane, const char *id)
{
    unbind(lane);
    auto refuse = [&](uint32_t reason, const char *why) {
        blockwork_seat_lane_v1_send_refused(lane.owner, reason, why);
    };
    Window *window = workspace()->findWindow(QUuid(QString::fromUtf8(id)));
    if (!window) {
        refuse(BLOCKWORK_SEAT_LANE_V1_REASON_NO_WINDOW, "that window is gone: check the In window block's name");
        return;
    }
    // Only xdg toplevels: XWayland windows go through one X11 seat.
    if (!window->inherits("KWin::XdgToplevelWindow") || !window->surface()) {
        refuse(BLOCKWORK_SEAT_LANE_V1_REASON_NOT_NATIVE,
               "that window can't take background input (XWayland apps can't): pick \"brought to front\" in the In window block");
        return;
    }
    // Firefox routes keys to its last focused window, whichever seat they came in on.
    if (window->resourceClass().contains(QLatin1String("firefox"), Qt::CaseInsensitive)) {
        for (Window *other : workspace()->windows()) {
            if (other != window && other->surface() && other->surface()->client() == window->surface()->client()
                && (other->isNormalWindow() || other->isDialog())) {
                refuse(BLOCKWORK_SEAT_LANE_V1_REASON_SHARED_FOCUS,
                       "Firefox shares keyboard focus between its windows: close its other windows, or use a separate profile");
                return;
            }
        }
    }
    lane.target = window;
    lane.closed = connect(window, &Window::closed, this, [this, &lane] {
        lose(lane, BLOCKWORK_SEAT_LANE_V1_REASON_CLOSED, QStringLiteral("the window closed"));
    });
    // Keep drawing it while covered or minimized, so screen blocks see it.
    window->refOffscreenRendering();
    lane.bindDeadline = m_clock.elapsed() + BindTimeoutMs;
    checkBinding(lane);
}

void BlockworkSeatHost::checkBinding(Lane &lane)
{
    Window *window = lane.target;
    if (!window || !lane.owner) {
        lane.binding->stop();
        return;
    }
    if (!window->surface()) {
        lose(lane, BLOCKWORK_SEAT_LANE_V1_REASON_CLOSED, QStringLiteral("the window closed"));
        return;
    }
    wl_client *client = window->surface()->client()->client();
    if (!keyboardReady(lane.seat.get(), client) || !pointerReady(lane.seat.get(), client)) {
        if (m_clock.elapsed() < lane.bindDeadline) {
            // Back to KWin's loop so the app can bind the seat.
            lane.binding->start();
            return;
        }
        lane.binding->stop();
        auto *resource = lane.owner;
        unbind(lane);
        blockwork_seat_lane_v1_send_refused(resource, BLOCKWORK_SEAT_LANE_V1_REASON_NOT_BOUND,
                                            "that app didn't pick up a second keyboard: it may not support background input "
                                            "(Chromium, Electron, GLFW and SDL2 apps can't), or it started before the KWin plugin did; restart the app");
        return;
    }
    lane.binding->stop();
    const QSizeF size = window->clientGeometry().size();
    if (!lane.entered) {
        // Point at the middle right away: games like Sober only take keys
        // while a pointer is over them.
        lane.pointer = QPointF(uint32_t(size.width()) / 2.0, uint32_t(size.height()) / 2.0);
        restPointer(lane);
        stamp(lane);
        lane.seat->setFocusedKeyboardSurface(window->surface());
        lane.entered = true;
        // Qt activates a window one wl_display.sync after keyboard enter, and
        // only an active window runs shortcuts or Qt Quick key handling.
        lane.awaitingSync = !window->isActive();
        lane.syncDeadline = m_clock.elapsed() + SyncWaitMs;
    }
    if (lane.awaitingSync && m_clock.elapsed() < lane.syncDeadline) {
        lane.binding->start();
        return;
    }
    lane.awaitingSync = false;
    blockwork_seat_lane_v1_send_bound(lane.owner, uint32_t(size.width()), uint32_t(size.height()));
}

void BlockworkSeatHost::unbind(Lane &lane)
{
    releaseAll(lane);
    lane.binding->stop();
    disconnect(lane.closed);
    if (lane.target) {
        lane.target->unrefOffscreenRendering();
    }
    lane.target = nullptr;
    lane.entered = false;
    lane.awaitingSync = false;
    lane.seat->setFocusedKeyboardSurface(nullptr);
    if (lane.pointerIn) {
        lane.seat->notifyPointerLeave();
        lane.seat->notifyPointerFrame();
        lane.pointerIn = false;
    }
    lane.pointer = {};
}

void BlockworkSeatHost::lose(Lane &lane, uint32_t reason, const QString &why)
{
    auto *resource = lane.owner;
    unbind(lane);
    if (resource) {
        blockwork_seat_lane_v1_send_lost(resource, reason, why.toUtf8().constData());
    }
}

void BlockworkSeatHost::giveBack(Lane &lane)
{
    unbind(lane);
    if (lane.customKeymap) {
        setDefaultKeymap(lane);
    }
    xkb_state_update_mask(lane.xkb, 0, 0, 0, 0, 0, 0);
    lane.owner = nullptr;
}

void BlockworkSeatHost::setKeymap(Lane &lane, int fd, uint32_t size)
{
    if (size == 0 || size > MaxKeymapBytes) {
        return;
    }
    void *map = mmap(nullptr, size, PROT_READ, MAP_PRIVATE, fd, 0);
    if (map == MAP_FAILED) {
        return;
    }
    const QByteArray text(static_cast<const char *>(map), qstrnlen(static_cast<const char *>(map), size));
    munmap(map, size);
    xkb_keymap *keymap = xkb_keymap_new_from_string(m_xkbContext, text.constData(), XKB_KEYMAP_FORMAT_TEXT_V1, XKB_KEYMAP_COMPILE_NO_FLAGS);
    if (!keymap) {
        return;
    }
    // Keys held under the old keymap would mean something else now.
    releaseAll(lane);
    lane.seat->keyboard()->setKeymap(text);
    xkb_state_unref(lane.xkb);
    lane.xkb = xkb_state_new(keymap);
    xkb_keymap_unref(keymap);
    lane.customKeymap = true;
}

void BlockworkSeatHost::key(Lane &lane, uint32_t key, uint32_t state)
{
    if (!lane.target || lane.binding->isActive()) {
        return;
    }
    const bool pressed = state == BLOCKWORK_SEAT_LANE_V1_KEY_STATE_PRESSED;
    if (pressed == lane.keys.contains(key)) {
        return;
    }
    pressed ? (void)lane.keys.insert(key) : (void)lane.keys.remove(key);
    stamp(lane);
    lane.seat->notifyKeyboardKey(key, pressed ? KeyboardKeyState::Pressed : KeyboardKeyState::Released,
                                 waylandServer()->display()->nextSerial());
    xkb_state_update_key(lane.xkb, key + 8, pressed ? XKB_KEY_DOWN : XKB_KEY_UP);
    updateModifiers(lane);
}

bool BlockworkSeatHost::enterPointer(Lane &lane)
{
    Window *window = lane.target;
    if (!window || lane.binding->isActive()) {
        return false;
    }
    // KWin allows one pointer constraint per surface, whatever the seat, and
    // kills a client that asks for a second; Sober asks once per pointer in it.
    if (!lane.pointerIn && constrained(lane)) {
        return false;
    }
    const QRectF client = window->clientGeometry();
    lane.pointer.setX(qBound(0.0, lane.pointer.x(), client.width() - 1));
    lane.pointer.setY(qBound(0.0, lane.pointer.y(), client.height() - 1));
    // inputTransformation maps KWin's global logical position into the surface.
    const QPointF global = client.topLeft() + lane.pointer;
    stamp(lane);
    if (lane.pointerIn && lane.seat->focusedPointerSurface() == window->surface()) {
        lane.seat->setFocusedPointerSurfaceTransformation(window->inputTransformation());
        lane.seat->notifyPointerMotion(global);
    } else {
        lane.seat->notifyPointerEnter(window->surface(), global, window->inputTransformation());
        lane.pointerIn = true;
    }
    lane.seat->notifyPointerFrame();
    return true;
}

void BlockworkSeatHost::motion(Lane &lane, const QPointF &local)
{
    lane.pointer = local;
    enterPointer(lane);
}

void BlockworkSeatHost::relativeMotion(Lane &lane, const QPointF &delta)
{
    if (!lane.pointerIn && !enterPointer(lane)) {
        return;
    }
    stamp(lane);
    const auto time = std::chrono::microseconds(m_clock.nsecsElapsed() / 1000);
    lane.seat->relativePointerMotion(delta, delta, time);
    lane.seat->notifyPointerFrame();
}

void BlockworkSeatHost::button(Lane &lane, uint32_t button, uint32_t state)
{
    if (!lane.pointerIn && !enterPointer(lane)) {
        return;
    }
    const bool pressed = state == BLOCKWORK_SEAT_LANE_V1_KEY_STATE_PRESSED;
    if (pressed == lane.buttons.contains(button)) {
        return;
    }
    pressed ? (void)lane.buttons.insert(button) : (void)lane.buttons.remove(button);
    stamp(lane);
    lane.seat->notifyPointerButton(button, pressed ? PointerButtonState::Pressed : PointerButtonState::Released);
    lane.seat->notifyPointerFrame();
    // Stay out from under the user's pointer once the click is done.
    if (lane.buttons.isEmpty() && userPointerOver(lane)) {
        leavePointer(lane);
    }
}

void BlockworkSeatHost::axis(Lane &lane, uint32_t axis, int32_t value120)
{
    if (!lane.pointerIn && !enterPointer(lane)) {
        return;
    }
    stamp(lane);
    // 15 degrees per notch, as libinput reports a wheel.
    const auto orientation = axis == BLOCKWORK_SEAT_LANE_V1_AXIS_HORIZONTAL ? Qt::Horizontal : Qt::Vertical;
    lane.seat->notifyPointerAxis(orientation, value120 * 15.0 / 120.0, value120, PointerAxisSource::Wheel);
    lane.seat->notifyPointerFrame();
}

void BlockworkSeatHost::releaseAll(Lane &lane)
{
    const auto keys = lane.keys;
    for (quint32 key : keys) {
        this->key(lane, key, BLOCKWORK_SEAT_LANE_V1_KEY_STATE_RELEASED);
    }
    const auto buttons = lane.buttons;
    for (quint32 button : buttons) {
        this->button(lane, button, BLOCKWORK_SEAT_LANE_V1_KEY_STATE_RELEASED);
    }
    // Anything left had no window to go to.
    lane.keys.clear();
    lane.buttons.clear();
    if (lane.xkb) {
        xkb_state_update_mask(lane.xkb, 0, 0, 0, 0, 0, 0);
        if (lane.target) {
            updateModifiers(lane);
        }
    }
}

void BlockworkSeatHost::updateModifiers(Lane &lane)
{
    lane.seat->notifyKeyboardModifiers(xkb_state_serialize_mods(lane.xkb, XKB_STATE_MODS_DEPRESSED),
                                       xkb_state_serialize_mods(lane.xkb, XKB_STATE_MODS_LATCHED),
                                       xkb_state_serialize_mods(lane.xkb, XKB_STATE_MODS_LOCKED),
                                       xkb_state_serialize_layout(lane.xkb, XKB_STATE_LAYOUT_EFFECTIVE));
}

void BlockworkSeatHost::stamp(Lane &lane)
{
    lane.seat->setTimestamp(std::chrono::microseconds(m_clock.nsecsElapsed() / 1000));
}

void BlockworkSeatHost::preempt(Window *window)
{
    if (!window) {
        return;
    }
    const qint64 now = m_clock.elapsed();
    for (auto &lane : m_lanes) {
        if (lane->owner && lane->target == window && now - lane->lastPreempt >= PreemptEveryMs) {
            lane->lastPreempt = now;
            blockwork_seat_lane_v1_send_preempted(lane->owner);
        }
    }
}

void BlockworkSeatHost::keyboardKey(KeyboardKeyEvent *)
{
    preempt(workspace()->activeWindow());
}

void BlockworkSeatHost::pointerMotion(PointerMotionEvent *)
{
    // KWin moves the user's pointer focus after the spies run, so look next turn.
    if (!m_pointerRecheck) {
        m_pointerRecheck = true;
        QTimer::singleShot(0, this, [this] {
            m_pointerRecheck = false;
            recheckPointers();
        });
    }
}

void BlockworkSeatHost::recheckPointers()
{
    for (auto &lane : m_lanes) {
        if (!lane->target || lane->binding->isActive() || !lane->buttons.isEmpty()) {
            continue;
        }
        if (userPointerOver(*lane)) {
            leavePointer(*lane);
        } else {
            restPointer(*lane);
        }
    }
}

bool BlockworkSeatHost::userPointerOver(const Lane &lane) const
{
    SurfaceInterface *user = waylandServer()->seat()->focusedPointerSurface();
    return user && lane.target && user == lane.target->surface();
}

bool BlockworkSeatHost::constrained(const Lane &lane) const
{
    SurfaceInterface *surface = lane.target ? lane.target->surface() : nullptr;
    return surface && (surface->lockedPointer() || surface->confinedPointer());
}

void BlockworkSeatHost::restPointer(Lane &lane)
{
    // Games like Sober only take keys with a pointer over them. The user's
    // own pointer does that while it's there, and two pointers can't both be.
    if (!lane.pointerIn && !userPointerOver(lane)) {
        enterPointer(lane);
    }
}

void BlockworkSeatHost::leavePointer(Lane &lane)
{
    if (!lane.pointerIn) {
        return;
    }
    stamp(lane);
    lane.seat->notifyPointerLeave();
    lane.seat->notifyPointerFrame();
    lane.pointerIn = false;
}

void BlockworkSeatHost::pointerButton(PointerButtonEvent *event)
{
    Window *window = input()->findToplevel(event->position);
    preempt(window);
    // Leave before the app sees the user's click, so it constrains one pointer.
    for (auto &lane : m_lanes) {
        if (window && lane->target == window && lane->buttons.isEmpty()) {
            leavePointer(*lane);
        }
    }
}

void BlockworkSeatHost::pointerAxis(PointerAxisEvent *event)
{
    preempt(input()->findToplevel(event->position));
}

bool BlockworkSeatHost::keyboardReady(SeatInterface *seat, wl_client *client) const
{
    for (const auto &request : m_keyboards) {
        if (request.seat != seat || !request.client || request.client->client() != client) {
            continue;
        }
        wl_resource *keyboard = wl_client_get_object(client, request.id);
        if (keyboard && std::strcmp(wl_resource_get_class(keyboard), "wl_keyboard") == 0) {
            return true;
        }
    }
    return false;
}

bool BlockworkSeatHost::pointerReady(SeatInterface *seat, wl_client *client) const
{
    struct Query {
        SeatInterface *seat;
        bool found = false;
    } query{seat};
    wl_client_for_each_resource(client, [](wl_resource *resource, void *data) {
        auto &query = *static_cast<Query *>(data);
        if (std::strcmp(wl_resource_get_class(resource), "wl_pointer") == 0) {
            auto *pointer = PointerInterface::get(resource);
            if (pointer && pointer->seat() == query.seat) {
                query.found = true;
                return WL_ITERATOR_STOP;
            }
        }
        return WL_ITERATOR_CONTINUE;
    }, &query);
    return query.found;
}

void BlockworkSeatHost::logRequest(void *data, wl_protocol_logger_type direction, const wl_protocol_logger_message *message)
{
    if (direction != WL_PROTOCOL_LOGGER_REQUEST) {
        return;
    }
    auto *self = static_cast<BlockworkSeatHost *>(data);
    if (std::strcmp(message->message->name, "sync") == 0
        && std::strcmp(wl_resource_get_class(message->resource), "wl_display") == 0) {
        wl_client *client = wl_resource_get_client(message->resource);
        for (const auto &lane : self->m_lanes) {
            if (lane->awaitingSync && lane->target && lane->target->surface()
                && lane->target->surface()->client()->client() == client) {
                lane->awaitingSync = false;
            }
        }
        return;
    }
    // KWin keeps no list of which seat a wl_keyboard came from, so note
    // get_keyboard on our seats as it happens.
    if (std::strcmp(message->message->name, "get_keyboard") != 0
        || std::strcmp(wl_resource_get_class(message->resource), "wl_seat") != 0) {
        return;
    }
    auto *seat = SeatInterface::get(message->resource);
    auto *client = ClientConnection::get(wl_resource_get_client(message->resource));
    for (const auto &lane : self->m_lanes) {
        if (lane->seat.get() == seat) {
            // Drop requests whose client or keyboard has gone, so ids can't pile up.
            std::erase_if(self->m_keyboards, [](const KeyboardRequest &request) {
                if (!request.client || request.client->tearingDown()) {
                    return true;
                }
                wl_resource *keyboard = wl_client_get_object(request.client->client(), request.id);
                return !keyboard || std::strcmp(wl_resource_get_class(keyboard), "wl_keyboard") != 0;
            });
            self->m_keyboards.push_back({seat, client, message->arguments[0].n});
            return;
        }
    }
}

} // namespace KWin

#include "moc_seathost.cpp"
