<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as api from "../api/quick";
import { useSettingsStore } from "../stores/settings";

const text = ref("");
const error = ref("");
const busy = ref(false);
const submitted = ref(false);
const input = ref<HTMLInputElement | null>(null);
let unlisten: UnlistenFn | null = null;

function focusInput() {
  error.value = "";
  input.value?.focus();
  input.value?.select();
}

onMounted(async () => {
  void useSettingsStore().init().catch(() => {});
  unlisten = await listen("quick-capture-open", focusInput);
  focusInput();
});

onBeforeUnmount(() => {
  unlisten?.();
  unlisten = null;
});

async function submit() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await api.submit(text.value);
    text.value = "";
    submitted.value = true;
    window.setTimeout(() => {
      submitted.value = false;
      void api.hide();
    }, 120);
  } catch (err) {
    // Keep the input content so nothing is silently lost on failure.
    error.value = String(err);
  } finally {
    busy.value = false;
  }
}

function onEnter(event: KeyboardEvent) {
  if (event.isComposing) return;
  void submit();
}

function onEsc() {
  void api.hide();
}
</script>

<template>
  <div class="quick-capture" @keydown.esc="onEsc">
    <div class="quick-drag" data-tauri-drag-region aria-hidden="true"></div>
    <button
      class="quick-close"
      type="button"
      aria-label="关闭快速收集"
      title="关闭（Esc）"
      @click="onEsc"
    >
      ×
    </button>
    <form class="quick-form glass-card" :class="{ 'is-submitted': submitted }" @submit.prevent="submit">
      <label class="sr-only" for="quick-input">快速收集</label>
      <input
        id="quick-input"
        ref="input"
        v-model="text"
        class="quick-input"
        placeholder="输入待办，或输入 /note 打开悬浮便签"
        :disabled="busy"
        autocomplete="off"
        spellcheck="false"
        @keydown.enter="onEnter"
      />
      <button class="btn primary quick-submit" type="submit" :disabled="busy">
        {{ busy ? "保存中…" : "保存" }}
      </button>
    </form>
    <p v-if="error" class="quick-error" role="alert">{{ error }}</p>
    <p class="quick-hint">Enter 保存到收件箱；/note 打开空白悬浮便签；Esc 或快捷键关闭</p>
  </div>
</template>
