// Helpers for `TimeScheduleDto` (types.ts) - the recurrence + time-of-day
// picked by a "When [time]" block. Kept separate from types.ts the same way
// valueOps.ts's operator registry is kept separate from the ValueDto shapes
// it operates on.
import type { TimeScheduleDto, WeekdayDto } from './types';

export const SCHEDULE_MODE_OPTIONS: { value: TimeScheduleDto['kind']; label: string }[] = [
  { value: 'Daily', label: 'every day' },
  { value: 'Weekly', label: 'every week' },
  { value: 'Monthly', label: 'every month' },
  { value: 'Yearly', label: 'every year' },
];

export const WEEKDAY_OPTIONS: { value: WeekdayDto; label: string }[] = [
  { value: 'Sunday', label: 'Sunday' },
  { value: 'Monday', label: 'Monday' },
  { value: 'Tuesday', label: 'Tuesday' },
  { value: 'Wednesday', label: 'Wednesday' },
  { value: 'Thursday', label: 'Thursday' },
  { value: 'Friday', label: 'Friday' },
  { value: 'Saturday', label: 'Saturday' },
];

const MONTH_NAMES = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];
// `value` is the 1-based month number as a string (AppDropdown's Option is
// always string-valued) - parsed back with `Number(...)` in withMonth.
export const MONTH_OPTIONS: { value: string; label: string }[] = MONTH_NAMES.map((label, i) => ({ value: String(i + 1), label }));

// "HH:MM", 24-hour, zero-padded - the wire/value format `<input type="time">`
// always uses regardless of what it *displays* (12h AM/PM or 24h follows the
// browser/OS locale automatically; the value itself is locale-independent).
export function scheduleTimeValue(s: TimeScheduleDto): string {
  return `${String(s.hour).padStart(2, '0')}:${String(s.minute).padStart(2, '0')}`;
}

// Rebuilds `s` with a new hour/minute parsed from an `<input type="time">`
// change event's value, keeping every other field as-is. A no-op (returns
// `s` unchanged) if the browser ever hands back something unparseable - a
// cleared time input fires an empty string, not a valid "HH:MM".
export function withTimeValue(s: TimeScheduleDto, hhmm: string): TimeScheduleDto {
  const [h, m] = hhmm.split(':').map(Number);
  if (!Number.isFinite(h) || !Number.isFinite(m)) return s;
  return { ...s, hour: h, minute: m };
}

// Switches to a different recurrence kind, preserving hour/minute and this
// schedule's own day/month if the new kind still uses one (so toggling
// Monthly <-> Yearly keeps the day-of-month you already picked), otherwise
// filling in a sensible default (the 1st, Monday, January).
export function withScheduleMode(s: TimeScheduleDto, kind: TimeScheduleDto['kind']): TimeScheduleDto {
  const { hour, minute } = s;
  const day = s.kind === 'Monthly' || s.kind === 'Yearly' ? s.day : 1;
  switch (kind) {
    case 'Daily':
      return { kind: 'Daily', hour, minute };
    case 'Weekly':
      return { kind: 'Weekly', weekday: s.kind === 'Weekly' ? s.weekday : 'Monday', hour, minute };
    case 'Monthly':
      return { kind: 'Monthly', day, hour, minute };
    case 'Yearly':
      return { kind: 'Yearly', month: s.kind === 'Yearly' ? s.month : 1, day, hour, minute };
  }
}
