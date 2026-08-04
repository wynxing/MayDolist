import type { GhAuthStatus, RepoSnapshot, RepoWatch } from "../types/github";
import { call } from "./index";

export const authStatus = () => call<GhAuthStatus>("github_auth_status");

export const watchlist = () => call<RepoWatch[]>("github_watchlist");

export const addWatch = (fullName: string) =>
  call<RepoWatch[]>("github_watch_add", { fullName });

export const removeWatch = (fullName: string) =>
  call<RepoWatch[]>("github_watch_remove", { fullName });

export const refresh = () => call<RepoSnapshot[]>("github_refresh");
