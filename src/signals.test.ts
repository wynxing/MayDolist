import { describe, expect, it } from "vitest";
import type { ActionSignal } from "./types/github";
import { SIGNAL_FILTER_OPTIONS, SIGNAL_LABELS, signalBadges } from "./signals";

describe("signalBadges", () => {
  it("maps every stable signal to its display label", () => {
    const badges = signalBadges(["needsAction", "needsReview", "ciFailed", "stale", "draft"]);
    expect(badges.map((badge) => badge.label)).toEqual([
      "需要我处理",
      "需要 Review",
      "CI 失败",
      "长期未更新",
      "Draft",
    ]);
    expect(badges.map((badge) => badge.key)).toEqual([
      "needsAction",
      "needsReview",
      "ciFailed",
      "stale",
      "draft",
    ]);
  });

  it("keeps input order and degrades on empty or undefined input", () => {
    expect(signalBadges(undefined)).toEqual([]);
    expect(signalBadges([])).toEqual([]);
    const badges = signalBadges(["stale", "needsAction"]);
    expect(badges.map((badge) => badge.key)).toEqual(["stale", "needsAction"]);
  });

  it("falls back to the raw key for unknown signal values", () => {
    const badges = signalBadges(["bogus" as ActionSignal]);
    expect(badges).toEqual([{ key: "bogus", label: "bogus" }]);
  });
});

describe("SIGNAL_FILTER_OPTIONS", () => {
  it("excludes draft (display-only) and only uses labeled signals", () => {
    const keys = SIGNAL_FILTER_OPTIONS.map(([key]) => key);
    expect(keys).toEqual(["needsAction", "needsReview", "ciFailed", "stale"]);
    for (const [key, label] of SIGNAL_FILTER_OPTIONS) {
      expect(SIGNAL_LABELS[key]).toBe(label);
    }
  });
});
