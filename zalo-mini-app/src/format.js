// Numbers and dates as Vietnamese farmers read them.

/** 6200 → "6.200" (đồng have no decimals; dots group thousands). */
export function dong(amount) {
  return String(Math.round(amount)).replace(/\B(?=(\d{3})+(?!\d))/g, ".");
}

/** "2026-11-07" → "07/11". */
export function dayMonth(isoDay) {
  const [, month, day] = String(isoDay).split("-");
  return `${day}/${month}`;
}

/** Whole days from `isoDay` to `isoToday` ("2026-11-20", "2026-11-22" → 2), never below 0. */
export function daysBetween(isoDay, isoToday) {
  const ms = Date.parse(`${isoToday}T00:00:00Z`) - Date.parse(`${isoDay}T00:00:00Z`);
  return Number.isFinite(ms) ? Math.max(0, Math.round(ms / 86_400_000)) : 0;
}

/** "0912345678" → "0912 345 678", as the website shows it. */
export function phone(number) {
  const n = String(number);
  return n.length === 10 ? `${n.slice(0, 4)} ${n.slice(4, 7)} ${n.slice(7)}` : n;
}
