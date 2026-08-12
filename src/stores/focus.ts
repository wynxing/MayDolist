import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { ref } from "vue";
import * as api from "../api/focus";
import type { FocusOverview } from "../types/focus";

let ready = false;
let timer: number | undefined;

/** Debounce entity-changed refreshes so rapid edits (e.g. drag reorder) do
 *  not hammer the backend with overlapping focus queries. */
function scheduleRefresh(refresh: () => Promise<void>) {
  clearTimeout(timer);
  timer = window.setTimeout(() => void refresh(), 150);
}

export const useFocusStore = defineStore("focus", () => {
  const overview = ref<FocusOverview | null>(null);
  const loading = ref(false);
  /** Todo id requested by a notification click; cleared after highlighting. */
  const focusTodoId = ref<string | null>(null);
  /** Global IPC-level error; per-domain failures live in the overview. */
  const error = ref<string | null>(null);
  let inFlight: Promise<void> | null = null;

  const requestFocus = (id: string) => {
    focusTodoId.value = id;
    void refresh();
  };

  const refresh = async () => {
    if (inFlight) return inFlight;
    inFlight = (async () => {
      loading.value = true;
      try {
        overview.value = await api.overview();
        error.value = null;
      } catch (e) {
        // Keep the previous projection as cached content on failure.
        error.value = String(e);
      } finally {
        loading.value = false;
        inFlight = null;
      }
    })();
    return inFlight;
  };

  const init = async () => {
    if (!ready) {
      ready = true;
      await listen<{ domain: string }>("entity-changed", (e) => {
        const domain = e.payload.domain;
        if (
          domain.startsWith("todo") ||
          domain === "note" ||
          domain === "github"
        ) {
          scheduleRefresh(refresh);
        }
      });
    }
    await refresh();
  };

  return { overview, loading, focusTodoId, error, init, refresh, requestFocus };
});
