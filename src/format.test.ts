import { describe, expect, it } from "vitest";
import { formatDuration } from "./format";

describe("formatDuration", () => {
  it("formats durations longer than one day without wrapping", () => {
    expect(formatDuration(27 * 3600 + 4 * 60)).toBe("27:04");
  });

  it("clamps a defensive negative value", () => {
    expect(formatDuration(-1)).toBe("00:00");
  });
});

