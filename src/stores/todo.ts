import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { ref } from "vue";
import * as api from "../api/todo";
import type { TodoItem, TodoList } from "../types/todo";

let ready = false;

function sortLists(lists: TodoList[]): TodoList[] {
  return [...lists].sort((a, b) => a.sortOrder - b.sortOrder);
}

function sortItems(items: TodoItem[]): TodoItem[] {
  return [...items].sort((a, b) => a.sortOrder - b.sortOrder);
}

export const useTodoStore = defineStore("todo", () => {
  const lists = ref<TodoList[]>([]);
  const error = ref<string | null>(null);

  const refresh = async () => {
    try {
      lists.value = await api.list();
      error.value = null;
    } catch (e) {
      error.value = String(e);
    }
  };

  const init = async () => {
    if (!ready) {
      ready = true;
      await listen<{ domain: string }>("entity-changed", (e) => {
        if (e.payload.domain.startsWith("todo")) void refresh();
      });
    }
    await refresh();
  };

  const upsertList = (list: TodoList) => {
    const index = lists.value.findIndex((v) => v.id === list.id);
    if (index >= 0) lists.value[index] = list;
    else lists.value.push(list);
    lists.value = sortLists(lists.value);
  };

  const upsertItem = (item: TodoItem) => {
    const list = lists.value.find((v) => v.items.some((row) => row.id === item.id));
    if (!list) return;
    const index = list.items.findIndex((row) => row.id === item.id);
    if (index >= 0) list.items[index] = item;
    list.items = sortItems(list.items);
  };

  return {
    lists,
    error,
    init,
    refresh,
    createList: async (t: string) => {
      const v = await api.createList(t);
      upsertList(v);
      return v;
    },
    updateList: async (id: string, p: Partial<{ title: string; deleted: boolean }>) => {
      const v = await api.updateList(id, p);
      upsertList(v);
      return v;
    },
    createItem: async (id: string, t: string) => {
      const v = await api.createItem(id, t);
      const list = lists.value.find((l) => l.id === id);
      if (list) {
        list.items.push(v);
        list.items = sortItems(list.items);
      }
      return v;
    },
    toggleItem: async (id: string, completed: boolean) => {
      const v = await api.updateItem(id, { completed: !completed });
      upsertItem(v);
    },
    renameItem: async (id: string, title: string) => {
      const v = await api.updateItem(id, { title });
      upsertItem(v);
    },
    softDelete: async (id: string) => {
      await api.softDelete(id);
      const list = lists.value.find((l) => l.items.some((row) => row.id === id));
      if (list) list.items = list.items.filter((row) => row.id !== id);
    },
    moveItem: async (id: string, targetListId: string, index: number) => {
      const v = await api.moveItem(id, targetListId, index);
      for (const list of lists.value) {
        const from = list.items.findIndex((row) => row.id === id);
        if (from >= 0) {
          list.items.splice(from, 1);
          break;
        }
      }
      const target = lists.value.find((l) => l.id === targetListId);
      if (target) {
        target.items.splice(Math.min(index, target.items.length), 0, v);
        target.items = sortItems(target.items);
      }
    },
    reorderLists: async (ids: string[]) => {
      lists.value = await api.reorderLists(ids);
    },
    reorderItems: async (listId: string, ids: string[]) => {
      const v = await api.reorderItems(listId, ids);
      upsertList(v);
    },
  };
});
