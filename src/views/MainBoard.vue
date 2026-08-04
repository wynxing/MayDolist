<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettingsStore } from "../stores/settings";
import GithubView from "./GithubView.vue";
import NoteView from "./NoteView.vue";
import SnippetView from "./SnippetView.vue";
import TodoView from "./TodoView.vue";

const settings = useSettingsStore();
const activeTab = ref("todo");
const tabs = [
  { id: "todo", label: "待办" },
  { id: "note", label: "笔记" },
  { id: "github", label: "GitHub" },
  { id: "snippet", label: "速记" },
];

onMounted(() => void settings.init());

function closeWindow() {
  void getCurrentWindow().close();
}
</script>

<template>
  <div class="shell">
    <header class="titlebar" data-tauri-drag-region>
      <span class="brand" data-tauri-drag-region>MayDolist</span>
      <nav class="tabs" data-tauri-drag-region>
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="tab"
          :class="{ active: activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          {{ tab.label }}
        </button>
      </nav>
      <button class="close" title="关闭" @click="closeWindow">×</button>
    </header>

    <main class="content">
      <TodoView v-show="activeTab === 'todo'" />
      <NoteView v-show="activeTab === 'note'" />
      <GithubView v-show="activeTab === 'github'" />
      <SnippetView v-show="activeTab === 'snippet'" />
    </main>

    <footer class="statusbar">
      <span v-if="settings.error" class="status-error">{{ settings.error }}</span>
      <span v-else-if="settings.dataDir">数据目录：{{ settings.dataDir }}</span>
      <span v-else>加载配置中…</span>
    </footer>
  </div>
</template>
