import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { ref } from "vue";
import * as githubApi from "../api/github";
import type { DataChangedPayload } from "../types/events";
import type { GhAuthStatus, RepoSnapshot, RepoWatch } from "../types/github";

let unlisten: (() => void) | null = null;

export const useGithubStore = defineStore("github", () => {
  const auth = ref<GhAuthStatus | null>(null);
  const watchlist = ref<RepoWatch[]>([]);
  const snapshots = ref<RepoSnapshot[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  let refreshing = false;

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    loading.value = true;
    error.value = null;
    try {
      const [authResult, watchResult, snapshotsResult] = await Promise.all([
        githubApi.authStatus(),
        githubApi.watchlist(),
        githubApi.refresh(),
      ]);
      auth.value = authResult;
      watchlist.value = watchResult;
      snapshots.value = snapshotsResult;
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    } finally {
      loading.value = false;
      refreshing = false;
    }
  }

  async function init() {
    if (!unlisten) {
      unlisten = await listen<DataChangedPayload>("data-changed", (event) => {
        if (event.payload.domain === "github") void refresh();
      });
    }
    await refresh();
  }

  async function addWatch(fullName: string) {
    watchlist.value = await githubApi.addWatch(fullName);
    await refresh();
  }

  async function removeWatch(fullName: string) {
    watchlist.value = await githubApi.removeWatch(fullName);
    await refresh();
  }

  return {
    auth,
    watchlist,
    snapshots,
    loading,
    error,
    init,
    refresh,
    addWatch,
    removeWatch,
  };
});
