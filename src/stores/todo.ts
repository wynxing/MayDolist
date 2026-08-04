import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { ref } from "vue";
import * as todoApi from "../api/todo";
import type { DataChangedPayload } from "../types/events";
import type { TodoList } from "../types/todo";

let unlisten: (() => void) | null = null;

export const useTodoStore = defineStore("todo", () => {
  const lists = ref<TodoList[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  let refreshing = false;

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    loading.value = true;
    error.value = null;
    try {
      lists.value = await todoApi.list();
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
        if (event.payload.domain === "todo") void refresh();
      });
    }
    await refresh();
  }

  async function createList(title: string) {
    await todoApi.createList(title);
    await refresh();
  }

  async function createItem(listId: string, title: string) {
    await todoApi.createItem(listId, title);
    await refresh();
  }

  async function toggleItem(itemId: string, completed: boolean) {
    await todoApi.updateItem(itemId, { completed: !completed });
    await refresh();
  }

  async function renameItem(itemId: string, title: string) {
    await todoApi.updateItem(itemId, { title });
    await refresh();
  }

  async function softDelete(itemId: string) {
    await todoApi.softDelete(itemId);
    await refresh();
  }

  return {
    lists,
    loading,
    error,
    init,
    refresh,
    createList,
    createItem,
    toggleItem,
    renameItem,
    softDelete,
  };
});
