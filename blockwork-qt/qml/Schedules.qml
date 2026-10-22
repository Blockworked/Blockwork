pragma Singleton
import QtQuick

// Times of day and cron schedules for the time blocks: reading them,
// writing them, and saying them in words. The cron reader follows
// blockwork-core's `input::cron` (five fields, lists, ranges, steps,
// names and the @hourly-style shortcuts).
QtObject {
    id: root

    readonly property var monthKeys: ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"]
    readonly property var weekdayKeys: ["sun", "mon", "tue", "wed", "thu", "fri", "sat"]
    // Sunday first, like cron.
    function weekdayShort(d) { return [qsTr("Sun"), qsTr("Mon"), qsTr("Tue"), qsTr("Wed"), qsTr("Thu"), qsTr("Fri"), qsTr("Sat")][d]; }
    function monthName(m) {
        return [qsTr("January"), qsTr("February"), qsTr("March"), qsTr("April"), qsTr("May"), qsTr("June"), qsTr("July"),
                qsTr("August"), qsTr("September"), qsTr("October"), qsTr("November"), qsTr("December")][m - 1];
    }

    // "12h", "24h", or "" to follow the system locale (Settings > General).
    property string clockFormat: ""
    readonly property bool use12h: clockFormat === "12h" || (clockFormat !== "24h" && /a/i.test(Qt.locale().timeFormat(Locale.ShortFormat)))
    readonly property string amText: Qt.locale().amText || "AM"
    readonly property string pmText: Qt.locale().pmText || "PM"

    function pad(n) { return String(n).padStart(2, "0"); }
    // How a time reads: "09:30", or "9:30 AM" on a 12-hour clock.
    function clock(hour, minute) {
        if (!use12h) return pad(hour % 24) + ":" + pad(minute);
        const h = hour % 24;
        return (h % 12 || 12) + ":" + pad(minute) + " " + (h < 12 ? amText : pmText);
    }
    // How a time is saved: always 24-hour "09:30", whatever the clock shows.
    function stamp(hour, minute) { return pad(hour) + ":" + pad(minute); }

    // "09:30", "9", "9:30 pm" -> {hour, minute} (24-hour), or null. Same
    // rules as blockwork-core's `parse_time_of_day`.
    function parseTime(text) {
        let t = String(text).trim().toLowerCase(), pm = null;
        for (const [suffix, isPm] of [["a.m.", false], ["p.m.", true], ["am", false], ["pm", true]])
            if (t.endsWith(suffix)) { t = t.slice(0, -suffix.length).trim(); pm = isPm; break; }
        const m = /^(\d{1,2})(?:[:.](\d{1,2}))?(?:[:.](\d{1,2}))?$/.exec(t);
        if (!m) return null;
        let hour = Number(m[1]);
        const minute = m[2] === undefined ? 0 : Number(m[2]), second = m[3] === undefined ? 0 : Number(m[3]);
        if (minute > 59 || second > 59) return null;
        if (pm === null) { if (hour > 23) return null; }
        else { if (hour < 1 || hour > 12) return null; hour = hour % 12 + (pm ? 12 : 0); }
        return { hour: hour, minute: minute };
    }
    // How a time slot reads on its block: tidied when it's a time, as typed otherwise.
    function timeLabel(text) {
        const t = parseTime(text);
        return t ? clock(t.hour, t.minute) : (String(text).trim().length ? String(text) : qsTr("set time"));
    }

    // ---- dates ----

    // "2026-12-24 18:00" -> {year, month, day, hour, minute}, or null. Reads
    // the formats blockwork-core's `parse_date` tries when it has none:
    // dashes or slashes, an optional T and seconds, or just the date (midnight).
    function parseDateTime(text) {
        const m = /^(\d{4})([-/])(\d{1,2})\2(\d{1,2})(?:[ T](\d{1,2}):(\d{1,2})(?::(\d{1,2}))?)?$/.exec(String(text).trim());
        if (!m) return null;
        const year = Number(m[1]), month = Number(m[3]), day = Number(m[4]);
        const hour = m[5] === undefined ? 0 : Number(m[5]), minute = m[6] === undefined ? 0 : Number(m[6]);
        const d = new Date(year, month - 1, day);
        if (d.getMonth() !== month - 1 || d.getDate() !== day || hour > 23 || minute > 59 || Number(m[7] || 0) > 59) return null;
        return { year: year, month: month, day: day, hour: hour, minute: minute };
    }
    // How a moment is saved: always "2026-12-24 18:00", whatever the clock shows.
    function dateTimeStamp(year, month, day, hour, minute) { return year + "-" + pad(month) + "-" + pad(day) + " " + stamp(hour, minute); }
    // How a date and time slot reads on its block: "2026-12-24 6:00 PM" on a
    // 12-hour clock, as typed when it isn't a date.
    function dateTimeLabel(text) {
        const t = parseDateTime(text);
        if (t) return t.year + "-" + pad(t.month) + "-" + pad(t.day) + " " + clock(t.hour, t.minute);
        return String(text).trim().length ? String(text) : qsTr("set date and time");
    }

    // ---- cron ----

    function expand(text) {
        const t = String(text).trim(), shortcuts = {
            "@yearly": "0 0 1 1 *", "@annually": "0 0 1 1 *", "@monthly": "0 0 1 * *", "@weekly": "0 0 * * 0",
            "@daily": "0 0 * * *", "@midnight": "0 0 * * *", "@hourly": "0 * * * *"
        };
        return shortcuts[t.toLowerCase()] || t;
    }
    // One field as an array of the values it allows, or null when it doesn't read.
    function field(text, min, max, names) {
        const value = part => {
            const i = names.indexOf(part.toLowerCase());
            if (i >= 0) return i + min;
            if (!/^\d+$/.test(part)) return NaN;
            const n = Number(part);
            return n < min || n > max ? NaN : n;
        };
        const out = [];
        for (const item of text.split(",")) {
            const [range, stepText] = item.split("/");
            const step = stepText === undefined ? 1 : Number(stepText);
            if (!(step >= 1) || !range.length) return null;
            let lo, hi;
            if (range === "*") { lo = min; hi = max; }
            else if (range.indexOf("-") > 0) { const [a, b] = range.split("-"); lo = value(a); hi = value(b); }
            else { lo = value(range); hi = stepText === undefined ? lo : max; }
            if (isNaN(lo) || isNaN(hi) || lo > hi) return null;
            for (let v = lo; v <= hi; v += step) if (out.indexOf(v) < 0) out.push(v);
        }
        return out.sort((a, b) => a - b);
    }
    // {minute, hour, day, month, weekday (arrays), parts (the five texts),
    //  daysRestricted, weekdaysRestricted}, or null.
    function parseCron(text) {
        const parts = expand(text).split(/\s+/).filter(p => p.length);
        if (parts.length !== 5) return null;
        const c = {
            minute: field(parts[0], 0, 59, []), hour: field(parts[1], 0, 23, []), day: field(parts[2], 1, 31, []),
            month: field(parts[3], 1, 12, monthKeys), weekday: field(parts[4], 0, 7, weekdayKeys),
            parts: parts, daysRestricted: parts[2][0] !== "*", weekdaysRestricted: parts[4][0] !== "*"
        };
        if (!c.minute || !c.hour || !c.day || !c.month || !c.weekday) return null;
        // 7 is Sunday too.
        c.weekday = c.weekday.map(d => d % 7).filter((d, i, a) => a.indexOf(d) === i).sort((a, b) => a - b);
        return c;
    }
    function dayMatches(c, date) {
        const d = c.day.indexOf(date.getDate()) >= 0, w = c.weekday.indexOf(date.getDay()) >= 0;
        if (c.daysRestricted && c.weekdaysRestricted) return d || w;
        return d && w;
    }
    // The next `count` times the schedule runs after `from`, looking a few years ahead.
    function nextRuns(text, count, from) {
        const c = parseCron(text);
        if (!c) return [];
        const out = [], start = new Date(from || Date.now());
        const day = new Date(start.getFullYear(), start.getMonth(), start.getDate());
        for (let i = 0; i < 366 * 5 && out.length < count; ++i, day.setDate(day.getDate() + 1)) {
            if (c.month.indexOf(day.getMonth() + 1) < 0 || !dayMatches(c, day)) continue;
            for (const h of c.hour) for (const m of c.minute) {
                const at = new Date(day.getFullYear(), day.getMonth(), day.getDate(), h, m);
                if (at > start && out.length < count) out.push(at);
            }
        }
        return out;
    }
    // A run time as it reads in a list: "Tue 09:30", with the date when it's further off.
    function runLabel(at, now) {
        const today = new Date(now || Date.now()); today.setHours(0, 0, 0, 0);
        const days = Math.round((new Date(at.getFullYear(), at.getMonth(), at.getDate()) - today) / 86400000);
        const time = clock(at.getHours(), at.getMinutes());
        if (days === 0) return qsTr("today %1").arg(time);
        if (days === 1) return qsTr("tomorrow %1").arg(time);
        if (days < 7) return weekdayShort(at.getDay()) + " " + time;
        return Qt.formatDate(at, "d MMM yyyy") + " " + time;
    }

    function isNumber(part) { return /^\d+$/.test(part); }
    // "*/15" -> 15, "*" -> 1, otherwise 0.
    function stepOf(part) { if (part === "*") return 1; const m = /^\*\/(\d+)$/.exec(part); return m ? Number(m[1]) : 0; }
    // "9-17" -> [9, 17], or null.
    function rangeOf(part) { const m = /^(\d+)-(\d+)$/.exec(part); return m ? [Number(m[1]), Number(m[2])] : null; }
    // A weekday set in words: "weekdays", "Mon, Wed", or "" for every day.
    function daysLabel(days) {
        const k = days.join(",");
        if (days.length === 7) return "";
        if (k === "1,2,3,4,5") return qsTr("weekdays");
        if (k === "0,6") return qsTr("weekends");
        // Monday first reads more naturally.
        return days.slice().sort((a, b) => ((a + 6) % 7) - ((b + 6) % 7)).map(weekdayShort).join(", ");
    }
    function withDays(text, days) {
        const d = daysLabel(days);
        return d ? qsTr("%1, %2").arg(text).arg(d) : text;
    }
    // The schedule in words, or "" when it's too unusual to say briefly.
    function describe(text) {
        const c = parseCron(text);
        if (!c) return "";
        const [m, h, dom, mon, dow] = c.parts;
        if (dom === "*" && mon === "*") {
            const minuteStep = stepOf(m), hourStep = stepOf(h), hours = rangeOf(h);
            if (minuteStep && (h === "*" || hours)) {
                let base = minuteStep === 1 ? qsTr("every minute") : qsTr("every %n minutes", "", minuteStep);
                if (hours) base = qsTr("%1 between %2 and %3").arg(base).arg(clock(hours[0], 0)).arg(clock(hours[1] + 1, 0));
                return withDays(base, c.weekday);
            }
            if (isNumber(m) && (hourStep || hours)) {
                const minute = Number(m);
                let base = hourStep === 1 ? (minute === 0 ? qsTr("every hour") : qsTr("every hour at :%1").arg(pad(minute)))
                    : hourStep ? (minute === 0 ? qsTr("every %n hours", "", hourStep) : qsTr("every %n hours at :%1", "", hourStep).arg(pad(minute)))
                    : qsTr("every hour from %1 to %2").arg(clock(hours[0], minute)).arg(clock(hours[1], minute));
                return withDays(base, c.weekday);
            }
            if (c.minute.length * c.hour.length <= 4 && !/[*\/-]/.test(m + h)) {
                const times = [];
                for (const hh of c.hour) for (const mm of c.minute) times.push(clock(hh, mm));
                const at = times.join(", ");
                const d = daysLabel(c.weekday);
                return d ? qsTr("%1 at %2").arg(d).arg(at) : qsTr("every day at %1").arg(at);
            }
            return "";
        }
        if (isNumber(m) && isNumber(h) && dow === "*") {
            const at = clock(Number(h), Number(m));
            if (isNumber(dom) && mon === "*") return qsTr("day %1 of every month at %2").arg(dom).arg(at);
            if (isNumber(dom) && c.month.length === 1 && !/[*,\/-]/.test(mon)) return qsTr("every %1 %2 at %3").arg(monthName(c.month[0])).arg(dom).arg(at);
        }
        return "";
    }
    // How a schedule slot reads on its block.
    function scheduleLabel(text) {
        const t = String(text).trim();
        if (!t.length) return qsTr("set schedule");
        return describe(t) || t;
    }
}
