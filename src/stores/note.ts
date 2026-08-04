import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { ref } from "vue";
import * as noteApi from "../api/note";
import type { DataChangedPayload } from "../types/events";
import type { Note } from "../types/note";

let unlisten: (() => void) | null = null;

export const useNoteStore = defineStore("note", () => {
  const notes = ref<Note[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  let refreshing = false;

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    loading.value = true;
    error.value = null;
    try {
      notes.value = await noteApi.list();
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
        if (event.payload.domain === "note") void refresh();
      });
    }
    await refresh();
  }

  async function create(title: string, content: string) {
    const note = await noteApi.create(title, content);
    await refresh();
    return note;
  }

  async function update(id: string, title: string, content: string) {
    const note = await noteApi.update(id, { title, content });
    await refresh();
    return note;
  }

  return { notes, loading, error, init, refresh, create, update };
});
