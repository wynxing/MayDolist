<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api/note";

const id = new URLSearchParams(location.search).get("note")!;
const note = ref<any>(null);
const status = ref("");
const loadError = ref("");
let timer: number | undefined;
let saveSeq = 0;

async function load() {
  loadError.value = "";
  try {
    note.value = await api.get(id);
  } catch (err) {
    loadError.value = String(err);
  }
}

onMounted(load);

watch(
  note,
  () => {
    if (!note.value) return;
    clearTimeout(timer);
    timer = window.setTimeout(save, 500);
  },
  { deep: true }
);

async function save() {
  if (!note.value) return;
  const seq = ++saveSeq;
  const snapshot = {
    title: note.value.title,
    content: note.value.content,
    collapsed: note.value.collapsed,
    alwaysOnTop: note.value.alwaysOnTop,
  };
  status.value = "保存中…";
  try {
    await api.update(id, snapshot);
    if (seq === saveSeq) status.value = "已保存";
  } catch (err) {
    if (seq === saveSeq) status.value = String(err);
  }
}

onBeforeUnmount(() => clearTimeout(timer));

async function dock() {
  await api.dock(id);
}

async function toggle() {
  note.value.collapsed = !note.value.collapsed;
  const { LogicalSize } = await import("@tauri-apps/api/dpi");
  await getCurrentWindow().setSize(
    new LogicalSize(360, note.value.collapsed ? 46 : 280)
  );
}
</script>
<template>
  <div v-if="loadError" class="floating">
    <header>
      <b class="floating-error">加载失败</b>
      <button aria-label="重新加载便签" @click="load">重试</button>
      <button aria-label="关闭悬浮便签" @click="dock">×</button>
    </header>
    <small class="floating-error">{{ loadError }}</small>
  </div>
  <div v-else-if="note" class="floating" :class="{ collapsed: note.collapsed }">
    <header data-tauri-drag-region>
      <input v-model="note.title" class="floating-title" aria-label="便签标题" />
      <button :aria-label="note.collapsed ? '展开便签' : '收起便签'" @click="toggle">
        {{ note.collapsed ? "▾" : "▴" }}
      </button>
      <button aria-label="关闭悬浮便签" @click="dock">×</button>
    </header>
    <textarea v-if="!note.collapsed" v-model="note.content" class="input" aria-label="便签内容"></textarea>
    <small v-if="!note.collapsed">{{ status }}</small>
  </div>
</template>
