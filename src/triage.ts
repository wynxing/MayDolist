/**
 * Inbox triage mode logic (#28).
 *
 * Triage is a pure view mode: it never changes the domain data format and
 * never adds a persisted field. All actions are dispatched through the
 * existing `todo_update_item` / `todo_move_item` / `todo_soft_delete`
 * commands, so the functions in this module only model the *queue state*:
 * which inbox item is shown next, how many remain, and which keyboard key
 * maps to which action.
 *
 * The cursor is based on stable item `id`s (never indexes), so deleting or
 * moving an item can never skip a queued entry.
 */

import type { TodoItem } from "./types/todo";

export type TriageAction = "today" | "later" | "move" | "complete" | "delete";

/** Keyboard mapping for triage actions (`1`-`5`). */
const ACTION_KEYS: Record<string, TriageAction> = {
  "1": "today",
  "2": "later",
  "3": "move",
  "4": "complete",
  "5": "delete",
};

/** Items that still need triage: not completed (deleted rows are already
 * filtered out by the backend list command, but we guard anyway). */
export function triagePending(items: TodoItem[]): TodoItem[] {
  return items.filter((item) => !item.completed && !item.deleted);
}

export interface TriageState {
  /** Ordered ids of the items captured when the mode was entered. */
  remainingIds: string[];
  /** Stable-id cursor: the item currently shown, or `null` when done/empty. */
  currentId: string | null;
}

/** Enter triage mode with the current pending inbox items. The queue is a
 * stable-id snapshot, so external edits (other windows) never reorder it. */
export function enterTriage(items: TodoItem[]): TriageState {
  const pending = triagePending(items);
  const remainingIds = pending.map((item) => item.id);
  return {
    remainingIds,
    currentId: remainingIds[0] ?? null,
  };
}

/**
 * Advance the cursor after a successful action on `currentId` (the item was
 * completed / soft-deleted / moved out of the inbox). The id is removed from
 * the queue, then the next still-pending id in queue order is selected, so a
 * cursor never jumps past entries that were not touched.
 */
export function advanceAfterAction(
  remainingIds: string[],
  currentId: string | null,
  items: TodoItem[]
): TriageState {
  const nextRemaining = remainingIds.filter((id) => id !== currentId);
  return pickNext(nextRemaining, items);
}

/**
 * Reconcile the cursor after the list changed externally (entity-changed /
 * another window). If the current id is gone or no longer pending, move to
 * the next still-pending queued id without re-processing anything.
 */
export function reconcileTriage(
  remainingIds: string[],
  currentId: string | null,
  items: TodoItem[]
): TriageState {
  const pendingIds = new Set(triagePending(items).map((item) => item.id));
  const nextRemaining = remainingIds.filter((id) => pendingIds.has(id));
  if (currentId && pendingIds.has(currentId)) {
    return { remainingIds: nextRemaining, currentId };
  }
  return pickNext(nextRemaining, items);
}

function pickNext(remainingIds: string[], items: TodoItem[]): TriageState {
  const pendingIds = new Set(triagePending(items).map((item) => item.id));
  const currentId = remainingIds.find((id) => pendingIds.has(id)) ?? null;
  return { remainingIds, currentId };
}

/** How many queued ids are still pending (the "剩余 N 条" counter). */
export function triageRemainingCount(remainingIds: string[], items: TodoItem[]): number {
  const pendingIds = new Set(triagePending(items).map((item) => item.id));
  return remainingIds.filter((id) => pendingIds.has(id)).length;
}

/** The mode is done when no queued item is still pending. */
export function isTriageDone(remainingIds: string[], items: TodoItem[]): boolean {
  return triageRemainingCount(remainingIds, items) === 0;
}

/**
 * Map a keydown to a triage action. IME composition input (`isComposing` or
 * keyCode 229) never triggers an action, matching the other keyboard-driven
 * surfaces in the app.
 */
export function triageKeyToAction(
  key: string,
  isComposing: boolean,
  keyCode?: number
): TriageAction | null {
  if (isComposing || keyCode === 229) return null;
  return ACTION_KEYS[key] ?? null;
}

/** Format a Date as a local `YYYY-MM-DD` due date. */
export function formatLocalDate(value: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())}`;
}

/** Due date for the `today` / `later` actions: local date plus offset days. */
export function triageDueDate(now: Date, offsetDays: number): string {
  const value = new Date(now);
  value.setDate(value.getDate() + offsetDays);
  return formatLocalDate(value);
}
