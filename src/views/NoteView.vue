<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useNoteStore } from "../stores/note";

const store = useNoteStore();
const selectedId = ref<string | null>(null);
const query = ref("");
const selectedTag = ref("");
const title = ref("");
const content = ref("");
const tagsText = ref("");
const status = ref("");
const dirty = ref(false);
let timer: number | undefined;
let applyingRemote = false;

const shown = computed(() => store.notes.filter((note) => {
  const queryValue = query.value.toLowerCase();
  const matchesQuery = !queryValue || [note.title, note.content, ...note.tags]
    .some((value) => value.toLowerCase().includes(queryValue));
  return matchesQuery && (!selectedTag.value || note.tags.includes(selectedTag.value));
}));
const allTags = computed(() => [...new Set(store.notes.flatMap((note) => note.tags))].sort());

onMounted(() => store.init());

async function applyNote(note: { title: string; content: string; tags: string[] }) {
  applyingRemote = true;
  title.value = note.title;
  content.value = note.content;
  tagsText.value = note.tags.join(", ");
  dirty.value = false;
  await nextTick();
  applyingRemote = false;
}

function choose(id: string) {
  selectedId.value = id;
  const note = store.notes.find((value) => value.id === id);
  if (note) void applyNote(note);
}

async function add() {
  const note = await store.create("新便签");
  choose(note.id);
}

async function save() {
  if (!selectedId.value || applyingRemote) return;
  const snapshot = {
    title: title.value || "未命名",
    content: content.value,
    tags: tagsText.value.split(/[,，]/).map((value) => value.trim()).filter(Boolean),
  };
  const tagsSnapshot = snapshot.tags.join(", ");
  status.value = "保存中…";
  try {
    await store.update(selectedId.value, snapshot);
    if (
      (title.value || "未命名") === snapshot.title &&
      content.value === snapshot.content &&
      tagsText.value.split(/[,，]/).map((value) => value.trim()).filter(Boolean).join(", ") ===
        tagsSnapshot
    ) {
      dirty.value = false;
    }
    status.value = "已保存";
  } catch (error) {
    status.value = String(error);
  }
}

watch([title, content, tagsText], () => {
  if (applyingRemote || !selectedId.value) return;
  dirty.value = true;
  clearTimeout(timer);
  timer = window.setTimeout(save, 500);
});

watch(
  () => store.notes,
  () => {
    if (!selectedId.value || dirty.value || applyingRemote) return;
    const note = store.notes.find((value) => value.id === selectedId.value);
    if (!note) return;
    if (
      note.title === title.value &&
      note.content === content.value &&
      note.tags.join(", ") === tagsText.value
    ) {
      return;
    }
    void applyNote(note);
  },
  { deep: true }
);
</script>

<template>
  <section class="pane-view">
    <aside class="list-pane">
      <button class="btn primary" @click="add">新建便签</button>
      <input v-model="query" class="input" placeholder="搜索标题、正文、标签" />
      <select v-model="selectedTag" class="input">
        <option value="">全部标签</option>
        <option v-for="tag in allTags" :key="tag">{{ tag }}</option>
      </select>
      <ul class="select-list">
        <li v-for="note in shown" :key="note.id" :class="{ active: selectedId === note.id }" @click="choose(note.id)">
          <b>{{ note.pinned ? "📌 " : "" }}{{ note.title }}</b>
          <small>{{ note.tags.join(" · ") }}</small>
        </li>
      </ul>
    </aside>
    <div v-if="selectedId" class="editor-pane">
      <input v-model="title" class="input editor-title" />
      <input v-model="tagsText" class="input" placeholder="标签，以逗号分隔" />
      <textarea v-model="content" class="input editor-content" placeholder="记录内容…"></textarea>
      <div class="editor-footer">
        <span>{{ status }}</span>
        <button class="btn" @click="store.update(selectedId, { pinned: !store.notes.find((note) => note.id === selectedId)?.pinned })">置顶</button>
        <button class="btn" @click="store.float(selectedId)">悬浮</button>
        <button class="btn danger" @click="store.remove(selectedId); selectedId = null">删除</button>
      </div>
    </div>
    <div v-else class="empty-state">选择或新建一条便签</div>
  </section>
</template>
