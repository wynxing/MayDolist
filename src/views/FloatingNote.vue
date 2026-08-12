<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api/note";
import { useSettingsStore } from "../stores/settings";

const id = new URLSearchParams(location.search).get("note")!;
const focusBody = new URLSearchParams(location.search).get("focus") === "body";
const settings = useSettingsStore();
const note = ref<any>(null);
const contentInput = ref<HTMLTextAreaElement | null>(null);
const status = ref("");
const loadError = ref("");
let timer: number | undefined;
let saveSeq = 0;
let hydrated = false;
let savePromise: Promise<void> | null = null;

async function load() {
  loadError.value = "";
  hydrated = false;
  try {
    note.value = await api.get(id);
    hydrated = true;
    if (focusBody) {
      await nextTick();
      contentInput.value?.focus();
    }
  } catch (err) {
    loadError.value = String(err);
  }
}

onMounted(async () => {
  await Promise.all([settings.init(), load()]);
});

watch(
  () =>
    note.value
      ? [
          note.value.title,
          note.value.content,
          note.value.collapsed,
          note.value.alwaysOnTop,
        ]
      : null,
  () => {
    if (!hydrated || !note.value) return;
    clearTimeout(timer);
    timer = window.setTimeout(() => {
      void save();
    }, 500);
  }
);

async function save() {
  if (!note.value) return;
  const seq = ++saveSeq;
  const snapshot = {
    title: String(note.value.title ?? "").trim() || "未命名",
    content: note.value.content,
    collapsed: note.value.collapsed,
    alwaysOnTop: note.value.alwaysOnTop,
  };
  status.value = "保存中…";
  const run = (async () => {
    try {
      await api.update(id, snapshot);
      if (seq === saveSeq) status.value = "已保存";
    } catch (err) {
      if (seq === saveSeq) status.value = String(err);
    }
  })();
  savePromise = run;
  await run;
  if (savePromise === run) savePromise = null;
}

async function flushSave() {
  clearTimeout(timer);
  timer = undefined;
  if (note.value) await save();
  else if (savePromise) await savePromise;
}

onBeforeUnmount(() => {
  clearTimeout(timer);
  timer = undefined;
  if (hydrated && note.value) void save();
});

async function dock() {
  await flushSave();
  await api.dock(id);
}

async function toggle() {
  note.value.collapsed = !note.value.collapsed;
  const { LogicalSize } = await import("@tauri-apps/api/dpi");
  await getCurrentWindow().setSize(
    new LogicalSize(360, note.value.collapsed ? 56 : 280)
  );
}
</script>
<template>
  <div v-if="loadError" class="floating">
    <div class="window-drag" data-tauri-drag-region aria-hidden="true"></div>
    <header>
      <b class="floating-error">加载失败</b>
      <button aria-label="重新加载便签" @click="load">重试</button>
      <button aria-label="关闭悬浮便签" @click="dock">×</button>
    </header>
    <small class="floating-error">{{ loadError }}</small>
  </div>
  <div v-else-if="note" class="floating" :class="{ collapsed: note.collapsed }">
    <div class="window-drag" data-tauri-drag-region aria-hidden="true"></div>
    <header>
      <input v-model="note.title" class="floating-title" aria-label="便签标题" />
      <button :aria-label="note.collapsed ? '展开便签' : '收起便签'" @click="toggle">
        {{ note.collapsed ? "▾" : "▴" }}
      </button>
      <button aria-label="关闭悬浮便签" @click="dock">×</button>
    </header>
    <textarea
      v-if="!note.collapsed"
      ref="contentInput"
      v-model="note.content"
      class="input"
      aria-label="便签内容"
    ></textarea>
    <small v-if="!note.collapsed" class="floating-status">{{ status }}</small>
  </div>
</template>
