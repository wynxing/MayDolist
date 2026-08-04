<script setup lang="ts">
import { onMounted, ref } from "vue";
import EmptyState from "../components/EmptyState.vue";
import { useNoteStore } from "../stores/note";

const store = useNoteStore();
const selectedId = ref<string | null>(null);
const title = ref("");
const content = ref("");
const saving = ref(false);
const saveMessage = ref<string | null>(null);

onMounted(() => void store.init());

function selectNote(id: string) {
  selectedId.value = id;
  const note = store.notes.find((n) => n.id === id);
  title.value = note?.title ?? "";
  content.value = note?.content ?? "";
}

function newNote() {
  selectedId.value = null;
  title.value = "";
  content.value = "";
  saveMessage.value = null;
}

async function save() {
  const trimmed = title.value.trim();
  if (!trimmed) {
    saveMessage.value = "标题不能为空";
    return;
  }
  saving.value = true;
  saveMessage.value = null;
  try {
    const note = selectedId.value
      ? await store.update(selectedId.value, trimmed, content.value)
      : await store.create(trimmed, content.value);
    selectNote(note.id);
    saveMessage.value = "已保存";
  } catch (err) {
    saveMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <section class="pane-view">
    <aside class="list-pane">
      <button class="btn primary block" @click="newNote">新建笔记</button>
      <ul class="select-list">
        <li
          v-for="note in store.notes"
          :key="note.id"
          :class="{ active: selectedId === note.id }"
          @click="selectNote(note.id)"
        >
          <span class="select-title">{{ note.title }}</span>
          <span class="select-meta">{{ note.updatedAt.slice(0, 16).replace("T", " ") }}</span>
        </li>
        <li v-if="store.notes.length === 0">
          <EmptyState text="暂无笔记" />
        </li>
      </ul>
      <p v-if="store.error" class="error">{{ store.error }}</p>
    </aside>

    <div class="editor-pane">
      <input v-model="title" class="input editor-title" placeholder="标题" />
      <textarea
        v-model="content"
        class="input editor-content"
        placeholder="开始书写…"
      ></textarea>
      <div class="editor-footer">
        <span v-if="saveMessage" class="save-message">{{ saveMessage }}</span>
        <button class="btn primary" :disabled="saving" @click="save">
          {{ saving ? "保存中…" : "保存" }}
        </button>
      </div>
    </div>
  </section>
</template>
