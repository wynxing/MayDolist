<script setup lang="ts">
import { onMounted, ref } from "vue";
import EmptyState from "../components/EmptyState.vue";
import { useSnippetStore } from "../stores/snippet";

const store = useSnippetStore();
const selectedId = ref<string | null>(null);
const title = ref("");
const content = ref("");
const tagsText = ref("");
const saving = ref(false);
const saveMessage = ref<string | null>(null);

onMounted(() => void store.init());

function parseTags(text: string): string[] {
  return text
    .split(/[,，]/)
    .map((tag) => tag.trim())
    .filter(Boolean);
}

function selectSnippet(id: string) {
  selectedId.value = id;
  const snippet = store.snippets.find((s) => s.id === id);
  title.value = snippet?.title ?? "";
  content.value = snippet?.content ?? "";
  tagsText.value = snippet?.tags.join(", ") ?? "";
}

function newSnippet() {
  selectedId.value = null;
  title.value = "";
  content.value = "";
  tagsText.value = "";
  saveMessage.value = null;
}

async function save() {
  const trimmed = title.value.trim();
  if (!trimmed) {
    saveMessage.value = "标题不能为空";
    return;
  }
  saving.value = true;
  saveMessage.value = null;
  try {
    const tags = parseTags(tagsText.value);
    const snippet = selectedId.value
      ? await store.update(selectedId.value, { title: trimmed, content: content.value, tags })
      : await store.create(trimmed, content.value, tags);
    selectSnippet(snippet.id);
    saveMessage.value = "已保存";
  } catch (err) {
    saveMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    saving.value = false;
  }
}

async function removeSelected() {
  if (!selectedId.value) return;
  await store.remove(selectedId.value);
  newSnippet();
}
</script>

<template>
  <section class="pane-view">
    <aside class="list-pane">
      <button class="btn primary block" @click="newSnippet">新建速记</button>
      <ul class="select-list">
        <li
          v-for="snippet in store.snippets"
          :key="snippet.id"
          :class="{ active: selectedId === snippet.id }"
          @click="selectSnippet(snippet.id)"
        >
          <span class="select-title">{{ snippet.title }}</span>
          <span class="tags-inline">
            <span v-for="tag in snippet.tags" :key="tag" class="tag">#{{ tag }}</span>
          </span>
        </li>
        <li v-if="store.snippets.length === 0">
          <EmptyState text="暂无速记" />
        </li>
      </ul>
      <p v-if="store.error" class="error">{{ store.error }}</p>
    </aside>

    <div class="editor-pane">
      <input v-model="title" class="input editor-title" placeholder="标题" />
      <input v-model="tagsText" class="input" placeholder="标签，用逗号分隔" />
      <textarea
        v-model="content"
        class="input editor-content"
        placeholder="记录一段内容…"
      ></textarea>
      <div class="editor-footer">
        <span v-if="saveMessage" class="save-message">{{ saveMessage }}</span>
        <button
          v-if="selectedId"
          class="btn ghost danger"
          :disabled="saving"
          @click="removeSelected"
        >
          删除
        </button>
        <button class="btn primary" :disabled="saving" @click="save">
          {{ saving ? "保存中…" : "保存" }}
        </button>
      </div>
    </div>
  </section>
</template>
