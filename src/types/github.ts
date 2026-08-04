/** Mirrors Rust `models::github::*`. */
export interface GhAuthStatus {
  loggedIn: boolean;
  user: string | null;
  message: string;
}

export interface RepoWatch {
  fullName: string;
}

export interface GhIssue {
  number: number;
  title: string;
  state: string;
  url: string;
  updatedAt: string;
}

export interface GhPullRequest {
  number: number;
  title: string;
  state: string;
  draft: boolean;
  url: string;
  updatedAt: string;
}

export interface RepoSnapshot {
  repo: string;
  fetchedAt: string;
  issues: GhIssue[];
  pullRequests: GhPullRequest[];
}
