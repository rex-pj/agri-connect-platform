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

/** "0912345678" → "0912 345 678", as the website shows it. */
export function phone(number) {
  const n = String(number);
  return n.length === 10 ? `${n.slice(0, 4)} ${n.slice(4, 7)} ${n.slice(7)}` : n;
}
