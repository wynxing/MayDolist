export interface GhAuthStatus {
  state: string;
  loggedIn: boolean;
  user: string | null;
  version: string | null;
  message: string;
}

/** Stable action signals; the UI never depends on raw GitHub strings. */
export type ActionSignal =
  | "needsAction"
  | "needsReview"
  | "ciFailed"
  | "stale"
  | "draft";

export interface GhIgnoredItem {
  number: number;
  kind: "pr" | "issue" | string;
}

export interface RepoWatch {
  fullName: string;
  filters: string[];
  collapsed: boolean;
  ignored: GhIgnoredItem[];
  pinned: number[];
  /** Action-signal filters; empty means no signal filtering (legacy behavior). */
  signalFilters: string[];
}

export interface GhIssue {
  number: number;
  title: string;
  state: string;
  url: string;
  updatedAt: string;
  kind: string;
  matches: string[];
  assignees: string[];
  signals: ActionSignal[];
}

export interface GhPullRequest {
  number: number;
  title: string;
  state: string;
  draft: boolean;
  url: string;
  updatedAt: string;
  matches: string[];
  assignees: string[];
  reviewers: string[];
  headSha: string | null;
  checksState: string | null;
  signals: ActionSignal[];
}

export interface RepoSnapshot {
  schemaVersion: number;
  repo: string;
  fetchedAt: string;
  lastSuccessAt: string | null;
  lastError: string | null;
  issues: GhIssue[];
  pullRequests: GhPullRequest[];
  /** When the persisted signals were computed; null for pre-signal caches. */
  signalsComputedAt: string | null;
}
