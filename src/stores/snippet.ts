import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { ref } from "vue";
import * as snippetApi from "../api/snippet";
import type { DataChangedPayload } from "../types/events";
import type { Snippet } from "../types/snippet";

let unlisten: (() => void) | null = null;

export const useSnippetStore = defineStore("snippet", () => {
  const snippets = ref<Snippet[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  let refreshing = false;

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    loading.value = true;
    error.value = null;
    try {
      snippets.value = await snippetApi.list();
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
        if (event.payload.domain === "snippet") void refresh();
      });
    }
    await refresh();
  }

  async function create(title: string, content: string, tags: string[]) {
    const snippet = await snippetApi.create(title, content, tags);
    await refresh();
    return snippet;
  }

  async function update(
    id: string,
    patch: { title: string; content: string; tags: string[] },
  ) {
    const snippet = await snippetApi.update(id, patch);
    await refresh();
    return snippet;
  }

  async function remove(id: string) {
    await snippetApi.remove(id);
    await refresh();
  }

  return { snippets, loading, error, init, refresh, create, update, remove };
});
