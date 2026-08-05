<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "../api/note";

const id = new URLSearchParams(location.search).get("note")!;
const note = ref<any>(null);
const status = ref("");
let timer: number | undefined;
let saveSeq = 0;

onMounted(async () => {
  note.value = await api.get(id);
});

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
  <div v-if="note" class="floating" :class="{ collapsed: note.collapsed }">
    <header data-tauri-drag-region>
      <input v-model="note.title" class="floating-title" />
      <button @click="toggle">{{ note.collapsed ? "▾" : "▴" }}</button>
      <button @click="dock">×</button>
    </header>
    <textarea v-if="!note.collapsed" v-model="note.content" class="input"></textarea>
    <small v-if="!note.collapsed">{{ status }}</small>
  </div>
</template>
