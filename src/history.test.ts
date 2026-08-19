import { describe, expect, it } from "vitest";
import { ALL_FILTER_VALUE, filterHistory, getHistoryYears } from "./history";
import type { WorkdaySummary } from "./types";

function workday(date: string): WorkdaySummary {
  return {
    date,
    arrivalMs: 0,
    departureMs: 1,
    pendingDepartureMs: null,
    durationSeconds: 0,
    status: "complete",
  };
}

const history = [workday("2026-08-19"), workday("2026-07-01"), workday("2025-08-02")];

describe("history filters", () => {
  it("returns years in descending order without duplicates", () => {
    expect(getHistoryYears(history)).toEqual(["2026", "2025"]);
  });

  it("filters by year and month together", () => {
    expect(filterHistory(history, "2026", "08").map(({ date }) => date)).toEqual(["2026-08-19"]);
  });

  it("supports filtering one dimension across all values of the other", () => {
    expect(filterHistory(history, ALL_FILTER_VALUE, "08").map(({ date }) => date)).toEqual([
      "2026-08-19",
      "2025-08-02",
    ]);
  });
});
