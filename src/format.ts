export function formatTime(timestampMs: number | null): string {
  if (timestampMs === null) return "—";
  return new Intl.DateTimeFormat("vi-VN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(timestampMs));
}

export function formatDate(date: string): string {
  const [year, month, day] = date.split("-").map(Number);
  return new Intl.DateTimeFormat("vi-VN", {
    weekday: "short",
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  }).format(new Date(year, month - 1, day));
}

export function formatDuration(totalSeconds: number): string {
  const safeSeconds = Math.max(0, totalSeconds);
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.floor((safeSeconds % 3600) / 60);
  return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}`;
}

/**
 * Formats a duration consistently with `formatTime`, which intentionally hides seconds.
 *
 * Example: 08:33:58 and 13:33:02 are displayed as 08:33 and 13:33. Truncating both
 * endpoints to minute precision before subtracting prevents the confusing result 04:59.
 * Raw timestamps remain unchanged in SQLite for audit and future calculations.
 */
export function formatDurationBetween(startTimestampMs: number, endTimestampMs: number): string {
  const startMinute = Math.floor(startTimestampMs / 60_000);
  const endMinute = Math.floor(endTimestampMs / 60_000);
  return formatDuration((endMinute - startMinute) * 60);
}

