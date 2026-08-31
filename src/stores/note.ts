import { defineStore } from "pinia";
import { ref } from "vue";
import * as api from "../api/note";
import { EntitySyncer } from "./entitySync";
import type { Note, NotePatch } from "../types/note";

function sortNotes(notes: Note[]): Note[] {
  return [...notes].sort(
    (a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt.localeCompare(a.updatedAt)
  );
}

export const useNoteStore = defineStore("note", () => {
  const notes = ref<Note[]>([]);
  const error = ref<string | null>(null);
  let inFlight: Promise<void> | null = null;

  const refresh = async () => {
    if (inFlight) return inFlight;
    inFlight = (async () => {
      try {
        notes.value = await api.list();
        error.value = null;
      } catch (e) {
        error.value = String(e);
      } finally {
        inFlight = null;
      }
    })();
    return inFlight;
  };

  const syncer = new EntitySyncer((domain) => domain === "note", refresh);

  const init = () => syncer.init();

  const upsert = (note: Note) => {
    const index = notes.value.findIndex((v) => v.id === note.id);
    if (index >= 0) notes.value[index] = note;
    else notes.value.push(note);
    notes.value = sortNotes(notes.value);
  };

  return {
    notes,
    error,
    init,
    refresh,
    create: async (t: string, c = "") => {
      const v = await api.create(t, c);
      upsert(v);
      return v;
    },
    update: async (id: string, p: NotePatch) => {
      const v = await api.update(id, p);
      upsert(v);
      return v;
    },
    remove: async (id: string) => {
      await api.remove(id);
      notes.value = notes.value.filter((v) => v.id !== id);
    },
    float: async (id: string) => {
      const v = await api.float(id);
      upsert(v);
      return v;
    },
    dock: async (id: string) => {
      const v = await api.dock(id);
      upsert(v);
      return v;
    },
  };
});
