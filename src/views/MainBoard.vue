<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { call } from "../api";
import { useFocusStore } from "../stores/focus";
import { useSettingsStore } from "../stores/settings";

const settings = useSettingsStore();
const focus = useFocusStore();
const buildId = __BUILD_ID__;
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

onMounted(async () => {
  await settings.init();
  await listen<string>("tray-action", (e) => {
    if (e.payload === "settings") active.value = "settings";
    if (e.payload === "new-note") active.value = "note";
    if (e.payload === "refresh-github") active.value = "github";
  });
  await listen<string>("focus-todo", (e) => {
    // Notification click: open the Focus tab and highlight the item.
    active.value = "focus";
    focus.requestFocus(e.payload);
  });
});
</script>

<template>
  <div class="shell">
    <div class="window-drag" data-tauri-drag-region aria-hidden="true"></div>
    <header class="titlebar">
      <b class="brand">MayDolist</b>
      <nav class="tabs">
        <button
          v-for="t in tabs"
          :key="t[0]"
          class="tab"
          :class="{ active: active === t[0] }"
          @click="active = t[0]"
        >
          {{ t[1] }}
        </button>
      </nav>
      <button class="close" aria-label="隐藏 MayDolist" title="隐藏" @click="call('app_hide_main')">×</button>
    </header>
    <main class="content">
      <KeepAlive>
        <component :is="activeView" @navigate="active = $event" />
      </KeepAlive>
    </main>
    <footer class="statusbar">
      <span v-if="settings.error" class="error">{{ settings.error }}</span>
      <span v-else>{{ settings.config?.dataDir || "MayDolist 本地数据" }}</span>
      <span class="build-id" :title="`Frontend build ${buildId}`">build {{ buildId }}</span>
    </footer>
  </div>
</template>
