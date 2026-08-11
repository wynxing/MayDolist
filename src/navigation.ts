import { ref } from "vue";

/**
 * Pending note to open in the note module. The Focus view sets it before
 * switching to the note tab; the note view consumes it on mount or when it
 * changes, so the selection survives lazy component mounting.
 */
export const pendingNoteId = ref<string | null>(null);

export function openNoteInModule(id: string) {
  pendingNoteId.value = id;
}

export function consumePendingNoteId(): string | null {
  const value = pendingNoteId.value;
  pendingNoteId.value = null;
  return value;
}
