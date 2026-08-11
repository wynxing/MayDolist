import type { ActionSignal } from "./github";
import type { TodoSource } from "./todo";

/** Mirrors Rust `models::focus::FocusSectionState`. */
export type FocusSectionState = "ready" | "error";

/** One incomplete Todo item in the Focus projection. */
export interface FocusTodo {
  id: string;
  title: string;
  listId: string;
  listTitle: string;
  inbox: boolean;
  updatedAt: string;
  /** Optional GitHub source; `null` for plain todos. */
  source: TodoSource | null;
}

/** One pinned or recently-updated Note in the Focus projection. */
export interface FocusNote {
  id: string;
  title: string;
  pinned: boolean;
  floating: boolean;
  updatedAt: string;
  preview: string;
}

/** One open GitHub issue / PR needing action. */
export interface FocusGithub {
  kind: "pr" | "issue";
  repo: string;
  number: number;
  title: string;
  state: string;
  draft: boolean;
  url: string;
  updatedAt: string;
  pinned: boolean;
  matches: string[];
  /** Stable action signals of the source item (empty for old caches). */
  signals: ActionSignal[];
}

/** One Focus section with per-domain state (read-only projection). */
export interface FocusSection<T> {
  state: FocusSectionState;
  error: string | null;
  total: number;
  offlineCache: boolean;
  items: T[];
}

export interface FocusOverview {
  generatedAt: string;
  todo: FocusSection<FocusTodo>;
  note: FocusSection<FocusNote>;
  github: FocusSection<FocusGithub>;
}
