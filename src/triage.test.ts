import { describe, expect, it } from "vitest";
import type { TodoItem } from "./types/todo";
import {
  advanceAfterAction,
  enterTriage,
  formatLocalDate,
  isTriageDone,
  reconcileTriage,
  triageDueDate,
  triageKeyToAction,
  triagePending,
  triageRemainingCount,
} from "./triage";

function item(id: string, completed = false, deleted = false): TodoItem {
  return {
    id,
    title: `item-${id}`,
    completed,
    deleted,
    sortOrder: 0,
    createdAt: "2026-08-12T00:00:00Z",
    updatedAt: "2026-08-12T00:00:00Z",
  };
}

describe("triagePending", () => {
  it("keeps only uncompleted, non-deleted items", () => {
    const rows = [item("a"), item("b", true), item("c", false, true), item("d")];
    expect(triagePending(rows).map((row) => row.id)).toEqual(["a", "d"]);
  });
});

describe("enterTriage", () => {
  it("starts at the first pending item with a stable id queue", () => {
    const state = enterTriage([item("a"), item("b"), item("c")]);
    expect(state.currentId).toBe("a");
    expect(state.remainingIds).toEqual(["a", "b", "c"]);
  });

  it("handles an empty inbox as an immediate done state", () => {
    const state = enterTriage([]);
    expect(state.currentId).toBeNull();
    expect(state.remainingIds).toEqual([]);
  });

  it("ignores completed items when entering", () => {
    const state = enterTriage([item("a", true), item("b")]);
    expect(state.currentId).toBe("b");
    expect(state.remainingIds).toEqual(["b"]);
  });
});

describe("advanceAfterAction", () => {
  it("moves to the next queued item after completing the current one", () => {
    const items = [item("a"), item("b"), item("c")];
    const first = enterTriage(items);
    const second = advanceAfterAction(first.remainingIds, first.currentId, items);
    expect(second.currentId).toBe("b");
    expect(second.remainingIds).toEqual(["b", "c"]);
    const third = advanceAfterAction(second.remainingIds, second.currentId, items);
    expect(third.currentId).toBe("c");
    expect(third.remainingIds).toEqual(["c"]);
  });

  it("never skips untouched queued items when the current one disappears", () => {
    // Simulate a concurrent window deleting "a" while we act on it.
    const after = advanceAfterAction(["a", "b", "c"], "a", [item("b"), item("c")]);
    expect(after.currentId).toBe("b");
    expect(after.remainingIds).toEqual(["b", "c"]);
  });

  it("ends in the done state after the last item is processed", () => {
    const state = advanceAfterAction(["a"], "a", []);
    expect(state.currentId).toBeNull();
    expect(state.remainingIds).toEqual([]);
    expect(isTriageDone(state.remainingIds, [])).toBe(true);
  });

  it("keeps cursor stability when a middle item is deleted externally", () => {
    // "b" is deleted by another window before we process "a".
    const reconciled = reconcileTriage(["a", "b", "c", "d"], "a", [
      item("a"),
      item("c"),
      item("d"),
    ]);
    expect(reconciled.currentId).toBe("a");
    expect(reconciled.remainingIds).toEqual(["a", "c", "d"]);
    const second = advanceAfterAction(reconciled.remainingIds, "a", [
      item("a"),
      item("c"),
      item("d"),
    ]);
    expect(second.currentId).toBe("c");
    expect(second.remainingIds).toEqual(["c", "d"]);
  });
});

describe("reconcileTriage", () => {
  it("keeps the current cursor while the item is still pending", () => {
    const items = [item("a"), item("b")];
    const state = reconcileTriage(["a", "b"], "a", items);
    expect(state.currentId).toBe("a");
    expect(state.remainingIds).toEqual(["a", "b"]);
  });

  it("jumps to the next queued pending item when the current one is completed elsewhere", () => {
    const items = [item("a", true), item("b"), item("c")];
    const state = reconcileTriage(["a", "b", "c"], "a", items);
    expect(state.currentId).toBe("b");
    expect(state.remainingIds).toEqual(["b", "c"]);
  });

  it("does not re-add externally created items to the queue", () => {
    const items = [item("a"), item("new")];
    const state = reconcileTriage(["a"], "a", items);
    expect(state.currentId).toBe("a");
    expect(state.remainingIds).toEqual(["a"]);
    expect(triageRemainingCount(state.remainingIds, items)).toBe(1);
  });

  it("becomes done when every queued item was handled elsewhere", () => {
    const items = [item("a", true), item("b", true)];
    const state = reconcileTriage(["a", "b"], "a", items);
    expect(state.currentId).toBeNull();
    expect(isTriageDone(state.remainingIds, items)).toBe(true);
  });
});

describe("triageKeyToAction", () => {
  it("maps 1-5 to today / later / move / complete / delete", () => {
    expect(triageKeyToAction("1", false)).toBe("today");
    expect(triageKeyToAction("2", false)).toBe("later");
    expect(triageKeyToAction("3", false)).toBe("move");
    expect(triageKeyToAction("4", false)).toBe("complete");
    expect(triageKeyToAction("5", false)).toBe("delete");
  });

  it("ignores other keys and IME composition input", () => {
    expect(triageKeyToAction("Enter", false)).toBeNull();
    expect(triageKeyToAction("Escape", false)).toBeNull();
    expect(triageKeyToAction("1", true)).toBeNull();
    expect(triageKeyToAction("1", false, 229)).toBeNull();
  });
});

describe("triageDueDate / formatLocalDate", () => {
  it("formats today as a local YYYY-MM-DD date", () => {
    expect(formatLocalDate(new Date(2026, 7, 12))).toBe("2026-08-12");
  });

  it("adds three days for the later action", () => {
    expect(triageDueDate(new Date(2026, 7, 12), 3)).toBe("2026-08-15");
  });

  it("crosses month and year boundaries", () => {
    expect(triageDueDate(new Date(2026, 7, 30), 3)).toBe("2026-09-02");
    expect(triageDueDate(new Date(2026, 11, 30), 3)).toBe("2027-01-02");
  });
});
