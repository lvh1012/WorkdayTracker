import { describe, expect, it } from "vitest";
import { formatDuration, formatDurationBetween } from "./format";

describe("formatDuration", () => {
  it("formats durations longer than one day without wrapping", () => {
    expect(formatDuration(27 * 3600 + 4 * 60)).toBe("27:04");
  });

  it("clamps a defensive negative value", () => {
    expect(formatDuration(-1)).toBe("00:00");
  });
});

describe("formatDurationBetween", () => {
  it("matches the minute precision shown by the arrival and departure labels", () => {
    const arrival = Date.UTC(2026, 7, 19, 8, 33, 58);
    const departure = Date.UTC(2026, 7, 19, 13, 33, 2);

    expect(formatDurationBetween(arrival, departure)).toBe("05:00");
  });

  it("clamps clock anomalies defensively", () => {
    expect(formatDurationBetween(120_000, 60_000)).toBe("00:00");
  });
});

