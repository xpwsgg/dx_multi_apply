import { describe, expect, it } from "vitest";
import { expandDateRange } from "./dateRange";

describe("expandDateRange", () => {
  it("expands inclusive range", () => {
    expect(expandDateRange("2026-03-01", "2026-03-03")).toEqual([
      "2026-03-01",
      "2026-03-02",
      "2026-03-03",
    ]);
  });

  it("returns one day when start equals end", () => {
    expect(expandDateRange("2026-03-01", "2026-03-01")).toEqual([
      "2026-03-01",
    ]);
  });

  it("throws when start is later than end", () => {
    expect(() => expandDateRange("2026-03-03", "2026-03-01")).toThrow(
      "开始日期不能晚于结束日期",
    );
  });

  it("throws when date format is invalid", () => {
    expect(() => expandDateRange("", "2026-03-01")).toThrow("无效日期格式");
  });

  it("throws when date range exceeds ten days", () => {
    expect(() => expandDateRange("2026-03-01", "2026-03-11")).toThrow(
      "日期区间不能超过 10 天",
    );
  });

  it("allows date range up to ten days", () => {
    expect(expandDateRange("2026-03-01", "2026-03-10")).toHaveLength(10);
  });
});
