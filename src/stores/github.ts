import { defineStore } from "pinia";
import { ref } from "vue";
import * as api from "../api/github";
import { EntitySyncer } from "./entitySync";
import type { GhAuthStatus, GithubSyncSummary, RepoSnapshot, RepoWatch } from "../types/github";

export const useGithubStore = defineStore("github", () => {
  const auth = ref<GhAuthStatus | null>(null);
  const watchlist = ref<RepoWatch[]>([]);
  const snapshots = ref<RepoSnapshot[]>([]);
  const error = ref<string | null>(null);
  const lastSyncSummary = ref<GithubSyncSummary | null>(null);

  const replaceSnapshot = (snap: RepoSnapshot) => {
    const idx = snapshots.value.findIndex((v) => v.repo === snap.repo);
    if (idx >= 0) snapshots.value[idx] = snap;
    else snapshots.value.push(snap);
  };

  const load = async () => {
    try {
      // Single round trip: auth + watchlist + every snapshot.
      const result = await api.overview();
      auth.value = result.auth;
      watchlist.value = result.watchlist;
      snapshots.value = result.snapshots;
      error.value = null;
    } catch (e) {
      error.value = String(e);
    }
  };

  const syncer = new EntitySyncer((domain) => domain === "github", load);

  const init = () => syncer.init();

  return {
    auth,
    watchlist,
    snapshots,
    error,
    init,
    refresh: async () => {
      const result = await api.refreshAll();
      snapshots.value = result.snapshots;
      lastSyncSummary.value = result.sync;
      watchlist.value = await api.watchlist();
      return result.sync;
    },
    refreshRepo: async (fullName: string) => {
      const result = await api.refreshRepo(fullName);
      replaceSnapshot(result.snapshot);
      lastSyncSummary.value = result.sync;
      return result.sync;
    },
    syncLinkedTodos: async () => {
      const result = await api.syncLinkedTodos();
      lastSyncSummary.value = result;
      return result;
    },
    addWatch: async (v: string) => {
      await api.addWatch(v);
      await load();
    },
    removeWatch: async (v: string) => {
      await api.removeWatch(v);
      await load();
    },
    setFilters: async (v: string, f: string[]) => {
      watchlist.value = await api.filters(v, f);
    },
    setSignalFilters: async (v: string, f: string[]) => {
      watchlist.value = await api.signalFilters(v, f);
    },
    setCollapsed: async (v: string, collapsed: boolean) => {
      watchlist.value = await api.collapsed(v, collapsed);
    },
    ignoreItem: async (fullName: string, number: number, kind: string) => {
      watchlist.value = await api.ignoreItem(fullName, number, kind);
      const snap = await api.snapshot(fullName);
      if (snap) replaceSnapshot(snap);
    },
    pinItem: async (fullName: string, number: number) => {
      const snap = await api.pinItem(fullName, number);
      replaceSnapshot(snap);
      watchlist.value = await api.watchlist();
    },
    unpinItem: async (fullName: string, number: number) => {
      watchlist.value = await api.unpinItem(fullName, number);
      const snap = await api.snapshot(fullName);
      if (snap) replaceSnapshot(snap);
    },
    lastSyncSummary,
    open: api.open,
  };
});
