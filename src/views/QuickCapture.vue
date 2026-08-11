<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as api from "../api/quick";
import { useSettingsStore } from "../stores/settings";

const text = ref("");
const error = ref("");
const busy = ref(false);
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
    await api.hide();
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
  <div class="quick-capture">
    <form class="quick-form glass-card" @submit.prevent="submit">
      <label class="sr-only" for="quick-input">快速收集</label>
      <input
        id="quick-input"
        ref="input"
        v-model="text"
        class="quick-input"
        placeholder="记录 Todo，或输入 note: 记录便签"
        :disabled="busy"
        autocomplete="off"
        spellcheck="false"
        @keydown.enter="onEnter"
        @keydown.esc="onEsc"
      />
      <button class="btn primary quick-submit" type="submit" :disabled="busy">
        {{ busy ? "保存中…" : "保存" }}
      </button>
    </form>
    <p v-if="error" class="quick-error" role="alert">{{ error }}</p>
    <p class="quick-hint">Enter 保存到收件箱，Esc 关闭；前缀 note: 创建便签</p>
  </div>
</template>
