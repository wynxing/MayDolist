import type {
  GhAuthStatus,
  GithubRefreshAllResult,
  GithubRefreshResult,
  GithubSyncSummary,
  RepoSnapshot,
  RepoWatch,
} from "../types/github";
import { call } from "./index";

export const status = () => call<GhAuthStatus>("github_status");
export const watchlist = () => call<RepoWatch[]>("github_watchlist");
export const addWatch = (fullName: string) =>
  call<RepoWatch[]>("github_watch_add", { fullName });
export const removeWatch = (fullName: string) =>
  call<RepoWatch[]>("github_watch_remove", { fullName });
export const filters = (fullName: string, filters: string[]) =>
  call<RepoWatch[]>("github_watch_filters", { fullName, filters });
export const signalFilters = (fullName: string, filters: string[]) =>
  call<RepoWatch[]>("github_watch_signal_filters", { fullName, filters });
export const collapsed = (fullName: string, collapsed: boolean) =>
  call<RepoWatch[]>("github_watch_collapsed", { fullName, collapsed });
export const ignoreItem = (fullName: string, number: number, kind: string) =>
  call<RepoWatch[]>("github_ignore_item", { fullName, number, kind });
export const pinItem = (fullName: string, number: number) =>
  call<RepoSnapshot>("github_pin_item", { fullName, number });
export const unpinItem = (fullName: string, number: number) =>
  call<RepoWatch[]>("github_unpin_item", { fullName, number });
export const refreshRepo = (fullName: string) =>
  call<GithubRefreshResult>("github_refresh_repo", { fullName });
export const refreshAll = () => call<GithubRefreshAllResult>("github_refresh_all");
export const syncLinkedTodos = () =>
  call<GithubSyncSummary>("github_sync_linked_todos");
export const snapshot = (fullName: string) =>
  call<RepoSnapshot | null>("github_get_snapshot", { fullName });
export const open = (url: string) => call<void>("open_external", { url });
