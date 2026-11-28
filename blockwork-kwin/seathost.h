// SPDX-License-Identifier: GPL-3.0-or-later
// The seat-pool approach follows Seatgeist's agent seat (MIT OR Apache-2.0,
// https://github.com/seatgeist/seatgeist); the code here is Blockwork's own.
#pragma once

#include <input_event_spy.h>
#include <plugin.h>

#include <QElapsedTimer>
#include <QPointF>
#include <QPointer>
#include <QSet>

#include <wayland-server-core.h>

#include <memory>
#include <vector>

struct xkb_context;
struct xkb_keymap;
struct xkb_state;
class QTimer;

namespace KWin {

class ClientConnection;
class SeatInterface;
class Window;

/// Offers blockwork_seat_host_v1: a fixed pool of extra wl_seats made at
/// startup (GTK only binds seats that exist when it starts), each lent to
/// one client at a time and aimed at one window.
class BlockworkSeatHost final : public Plugin, public InputEventSpy
{
    Q_OBJECT

public:
    BlockworkSeatHost();
    ~BlockworkSeatHost() override;

    void keyboardKey(KeyboardKeyEvent *event) override;
    void pointerMotion(PointerMotionEvent *event) override;
    void pointerButton(PointerButtonEvent *event) override;
    void pointerAxis(PointerAxisEvent *event) override;

    struct Lane {
        std::unique_ptr<SeatInterface> seat;
        xkb_state *xkb = nullptr;
        /// The lane's protocol object while lent out.
        wl_resource *owner = nullptr;
        QPointer<Window> target;
        QPointF pointer;
        bool pointerIn = false;
        bool customKeymap = false;
        QSet<quint32> keys;
        QSet<quint32> buttons;
        QMetaObject::Connection closed;
        QTimer *binding = nullptr;
        qint64 bindDeadline = 0;
        /// Keyboard entered; bound waits for the app's next wl_display.sync.
        bool entered = false;
        /// The user's own pointer was over the target at the last check.
        bool userPointerOver = false;
        bool awaitingSync = false;
        qint64 syncDeadline = 0;
        qint64 lastPreempt = -1000;
    };

    // Protocol entry points, called from the C request tables.
    static void bindHost(wl_client *client, void *data, uint32_t version, uint32_t id);
    void getLane(wl_resource *host, uint32_t id);
    void bindLane(Lane &lane, const char *window);
    void setKeymap(Lane &lane, int fd, uint32_t size);
    void key(Lane &lane, uint32_t key, uint32_t state);
    void motion(Lane &lane, const QPointF &local);
    void relativeMotion(Lane &lane, const QPointF &delta);
    void button(Lane &lane, uint32_t button, uint32_t state);
    void axis(Lane &lane, uint32_t axis, int32_t value120);
    void releaseAll(Lane &lane);
    /// Lets go of everything and returns the lane to the pool.
    void giveBack(Lane &lane);

private:
    bool allowed(wl_client *client) const;
    bool makeLane(int index);
    void checkBinding(Lane &lane);
    void unbind(Lane &lane);
    void lose(Lane &lane, uint32_t reason, const QString &why);
    bool enterPointer(Lane &lane);
    void updateModifiers(Lane &lane);
    void stamp(Lane &lane);
    void setDefaultKeymap(Lane &lane);
    void preempt(Window *window);
    void recheckPointers();

    bool keyboardReady(SeatInterface *seat, wl_client *client) const;
    bool pointerReady(SeatInterface *seat, wl_client *client) const;
    static void logRequest(void *data, wl_protocol_logger_type direction, const wl_protocol_logger_message *message);

    wl_global *m_global = nullptr;
    wl_protocol_logger *m_logger = nullptr;
    std::vector<std::unique_ptr<Lane>> m_lanes;
    xkb_context *m_xkbContext = nullptr;
    xkb_keymap *m_defaultKeymap = nullptr;
    QByteArray m_defaultKeymapText;
    QElapsedTimer m_clock;

    /// wl_seat.get_keyboard calls on our seats: the keyboard object ids.
    struct KeyboardRequest {
        SeatInterface *seat;
        QPointer<ClientConnection> client;
        uint32_t id;
    };
    std::vector<KeyboardRequest> m_keyboards;
    bool m_pointerRecheck = false;
};

} // namespace KWin
