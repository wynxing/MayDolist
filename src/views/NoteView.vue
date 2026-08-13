<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import ConfirmBar from "../components/ConfirmBar.vue";
import EmptyState from "../components/EmptyState.vue";
import PageHeader from "../components/PageHeader.vue";
import PinMark from "../components/PinMark.vue";
import { consumePendingNoteId, pendingNoteId } from "../navigation";
import { NOTE_COLORS, noteColorId } from "../noteColor";
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
const pendingDelete = ref(false);
let timer: number | undefined;
let applyingRemote = false;

const shown = computed(() => store.notes.filter((note) => {
  const queryValue = query.value.toLowerCase();
  const matchesQuery = !queryValue || [note.title, note.content, ...note.tags]
    .some((value) => value.toLowerCase().includes(queryValue));
  return matchesQuery && (!selectedTag.value || note.tags.includes(selectedTag.value));
}));
const allTags = computed(() => [...new Set(store.notes.flatMap((note) => note.tags))].sort());
const selectedNote = computed(() => store.notes.find((note) => note.id === selectedId.value) ?? null);

onMounted(async () => {
  await store.init();
  watch(
    pendingNoteId,
    (id) => {
      if (!id) return;
      consumePendingNoteId();
      selectPending(id);
    },
    { immediate: true }
  );
});

function selectPending(id: string) {
  if (store.notes.some((note) => note.id === id)) {
    choose(id);
    return;
  }
  const stop = watch(
    () => store.notes,
    (notes) => {
      if (notes.some((note) => note.id === id)) {
        choose(id);
        stop();
      }
    },
    { deep: true }
  );
}

async function applyNote(note: { title: string; content: string; tags: string[] }) {
  applyingRemote = true;
  title.value = note.title;
  content.value = note.content;
  tagsText.value = note.tags.join(", ");
  dirty.value = false;
  pendingDelete.value = false;
  await nextTick();
  applyingRemote = false;
}

function choose(id: string) {
  selectedId.value = id;
  pendingDelete.value = false;
  const note = store.notes.find((value) => value.id === id);
  if (note) void applyNote(note);
}

function preview(text: string) {
  const line = text.replace(/\s+/g, " ").trim();
  return line.slice(0, 48);
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

async function setColor(color: string) {
  if (!selectedId.value) return;
  await store.update(selectedId.value, { color });
}

async function confirmDelete() {
  if (!selectedId.value) return;
  await store.remove(selectedId.value);
  selectedId.value = null;
  pendingDelete.value = false;
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
  <section class="note-view" aria-labelledby="note-heading">
    <PageHeader heading-id="note-heading" title="我的便签" subtitle="把想法留在手边，需要时再拖到桌面。">
      <template #actions>
        <button class="btn primary" type="button" @click="add">新建便签</button>
      </template>
    </PageHeader>

    <div class="pane-view">
      <aside class="list-pane" aria-label="便签列表">
        <input v-model="query" class="input" placeholder="搜索标题、正文、标签" />
        <select v-model="selectedTag" class="input">
          <option value="">全部标签</option>
          <option v-for="tag in allTags" :key="tag">{{ tag }}</option>
        </select>
        <div v-if="shown.length" class="select-list" role="listbox">
          <button
            v-for="note in shown"
            :key="note.id"
            type="button"
            role="option"
            class="note-accent"
            :class="{ active: selectedId === note.id }"
            :data-color="noteColorId(note.color)"
            :aria-selected="selectedId === note.id"
            @click="choose(note.id)"
          >
            <span class="select-title">
              <PinMark :on="note.pinned" />{{ note.title }}
            </span>
            <small v-if="note.tags.length" class="select-meta">{{ note.tags.join(" · ") }}</small>
            <small v-else-if="preview(note.content)" class="select-preview">{{ preview(note.content) }}</small>
          </button>
        </div>
        <EmptyState v-else text="没有匹配的便签" action-label="新建便签" @action="add" />
      </aside>
      <div v-if="selectedId && selectedNote" class="editor-pane">
        <input v-model="title" class="input editor-title" aria-label="便签标题" />
        <input v-model="tagsText" class="input" placeholder="标签，以逗号分隔" />
        <textarea v-model="content" class="input editor-content" placeholder="记录内容…"></textarea>
        <ConfirmBar
          v-if="pendingDelete"
          message="将这条便签移入回收站？"
          confirm-label="移入回收站"
          danger
          @confirm="confirmDelete"
          @cancel="pendingDelete = false"
        />
        <div class="editor-footer">
          <span :class="{ error: status.startsWith('Error') || status.includes('失败') }" role="status">
            {{ status }}
          </span>
          <div class="note-color-dots" role="group" aria-label="便签颜色">
            <button
              v-for="color in NOTE_COLORS"
              :key="color.id"
              class="note-color-dot"
              :class="{ active: noteColorId(selectedNote.color) === color.id }"
              :data-color="color.id"
              type="button"
              :title="color.label"
              :aria-label="`颜色：${color.label}`"
              @click="setColor(color.id)"
            />
          </div>
          <button
            class="btn"
            type="button"
            @click="store.update(selectedId, { pinned: !selectedNote.pinned })"
          >
            {{ selectedNote.pinned ? "取消置顶" : "置顶" }}
          </button>
          <button class="btn" type="button" @click="store.float(selectedId)">悬浮</button>
          <button class="btn danger" type="button" @click="pendingDelete = true">删除</button>
        </div>
      </div>
      <EmptyState v-else title="还没有打开便签" text="选择左侧一条，或新建一条开始写。" action-label="新建便签" @action="add" />
    </div>
  </section>
</template>
