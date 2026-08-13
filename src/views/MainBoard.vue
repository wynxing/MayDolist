<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { call } from "../api";
import { openNoteInModule } from "../navigation";
import { useFocusStore } from "../stores/focus";
import { useGithubStore } from "../stores/github";
import { useNoteStore } from "../stores/note";
import { useSettingsStore } from "../stores/settings";

const settings = useSettingsStore();
const focus = useFocusStore();
const notes = useNoteStore();
const github = useGithubStore();
const active = ref("focus");
const tabs = [
  ["focus", "今日"],
  ["todo", "待办"],
  ["note", "便签"],
  ["github", "GitHub"],
  ["settings", "设置"],
];

const FocusView = defineAsyncComponent(() => import("./FocusView.vue"));
const TodoView = defineAsyncComponent(() => import("./TodoView.vue"));
const NoteView = defineAsyncComponent(() => import("./NoteView.vue"));
const GithubView = defineAsyncComponent(() => import("./GithubView.vue"));
const SettingsView = defineAsyncComponent(() => import("./SettingsView.vue"));

const activeView = computed(() => {
  switch (active.value) {
    case "focus":
      return FocusView;
    case "note":
      return NoteView;
    case "github":
      return GithubView;
    case "settings":
      return SettingsView;
    default:
      return TodoView;
  }
});

const isDemo = computed(() => (settings.config?.dataDir ?? "").includes("Demo 数据"));
const statusText = computed(() => {
  if (settings.error) return settings.error;
  if (isDemo.value) return settings.config?.dataDir ?? "";
  return "";
});
const showFirstRun = computed(() => settings.config?.firstRun === true && !isDemo.value);

async function dismissFirstRun() {
  await settings.update({ firstRun: false });
}

async function createNoteFromTray() {
  active.value = "note";
  await notes.init();
  const note = await notes.create("新便签");
  openNoteInModule(note.id);
  await notes.float(note.id);
}

async function refreshGithubFromTray() {
  active.value = "github";
  await github.init();
  await github.refresh();
}

onMounted(async () => {
  await settings.init();
  await listen<string>("tray-action", (e) => {
    if (e.payload === "settings") active.value = "settings";
    if (e.payload === "new-note") void createNoteFromTray();
    if (e.payload === "refresh-github") void refreshGithubFromTray();
  });
  await listen<string>("focus-todo", (e) => {
    active.value = "focus";
    focus.requestFocus(e.payload);
  });
  await listen<{ tab: string; noteId?: string }>("command-palette-navigate", (e) => {
    if (e.payload.noteId) openNoteInModule(e.payload.noteId);
    active.value = e.payload.tab;
  });
});
</script>

<template>
  <div class="shell">
    <div class="window-drag" data-tauri-drag-region aria-hidden="true"></div>
    <header class="titlebar">
      <b class="brand">MayDolist</b>
      <nav class="tabs" role="tablist">
        <button
          v-for="t in tabs"
          :key="t[0]"
          class="tab"
          role="tab"
          :class="{ active: active === t[0] }"
          :aria-selected="active === t[0]"
          @click="active = t[0]"
        >
          {{ t[1] }}
        </button>
      </nav>
      <button class="close" aria-label="隐藏 MayDolist" title="隐藏" @click="call('app_hide_main')">×</button>
    </header>
    <aside v-if="showFirstRun" class="first-run glass-card">
      <h2>欢迎使用 MayDolist</h2>
      <p>数据只存在本机。用快捷键随时捕获，从「今日」判断下一步。</p>
      <ul class="first-run-keys">
        <li><kbd>{{ settings.config?.hotkey || "Ctrl+Alt+M" }}</kbd> 主面板</li>
        <li><kbd>{{ settings.config?.quickCaptureHotkey || "Ctrl+Alt+Space" }}</kbd> 快速收集</li>
        <li><kbd>{{ settings.config?.commandPaletteHotkey || "Ctrl+K" }}</kbd> 命令面板</li>
      </ul>
      <p>数据目录：{{ settings.config?.dataDir }}</p>
      <button class="btn primary" type="button" @click="dismissFirstRun">开始使用</button>
    </aside>
    <main class="content">
      <KeepAlive>
        <component :is="activeView" :key="active" class="view-enter-active" @navigate="active = $event" />
      </KeepAlive>
    </main>
    <footer v-if="statusText" class="statusbar">
      <span :class="{ error: settings.error }">{{ statusText }}</span>
    </footer>
  </div>
</template>
