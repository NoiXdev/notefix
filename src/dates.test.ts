import { describe, it, expect } from "vitest";
import { formatDate, toDateInputValue, fromDateInputValue } from "./dates";

const ts = new Date(2026, 5, 23).getTime(); // 2026-06-23 local

describe("formatDate", () => {
  it("formats de / iso / us", () => {
    expect(formatDate(ts, "de")).toBe("23.06.2026");
    expect(formatDate(ts, "iso")).toBe("2026-06-23");
    expect(formatDate(ts, "us")).toBe("06/23/2026");
  });
});

describe("date input helpers", () => {
  it("round-trips a date", () => {
    expect(toDateInputValue(ts)).toBe("2026-06-23");
    expect(fromDateInputValue("2026-06-23")).toBe(ts);
    expect(toDateInputValue(null)).toBe("");
    expect(fromDateInputValue("")).toBe(null);
  });
});
