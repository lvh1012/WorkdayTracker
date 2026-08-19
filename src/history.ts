import type { WorkdaySummary } from "./types";

export const ALL_FILTER_VALUE = "all";

/** Returns distinct ISO years in descending order for the year selector. */
export function getHistoryYears(history: WorkdaySummary[]): string[] {
  return [...new Set(history.map((workday) => workday.date.slice(0, 4)))].sort((a, b) =>
    b.localeCompare(a),
  );
}

/** Applies independent year and month filters to ISO local-date values. */
export function filterHistory(
  history: WorkdaySummary[],
  selectedYear: string,
  selectedMonth: string,
): WorkdaySummary[] {
  return history.filter((workday) => {
    const [year, month] = workday.date.split("-");
    const matchesYear = selectedYear === ALL_FILTER_VALUE || year === selectedYear;
    const matchesMonth = selectedMonth === ALL_FILTER_VALUE || month === selectedMonth;
    return matchesYear && matchesMonth;
  });
}
