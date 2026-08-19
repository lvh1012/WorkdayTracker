import { describe, expect, it } from "vitest";
import { parseThemePreference, resolveTheme } from "./theme";

describe("theme preference", () => {
  it("falls back to system for an unknown persisted value", () => {
    expect(parseThemePreference("unexpected")).toBe("system");
  });

  it("resolves system from the operating-system preference", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });

  it("keeps an explicit user preference", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
});
