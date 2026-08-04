<script setup lang="ts">
import { onMounted, ref } from "vue";
import EmptyState from "../components/EmptyState.vue";
import { useTodoStore } from "../stores/todo";

const store = useTodoStore();
const newListTitle = ref("");
const newItemTitles = ref<Record<string, string>>({});
const editingItemId = ref<string | null>(null);
const editDraft = ref("");

onMounted(() => void store.init());

async function addList() {
  const title = newListTitle.value.trim();
  if (!title) return;
  await store.createList(title);
  newListTitle.value = "";
}

async function addItem(listId: string) {
  const title = (newItemTitles.value[listId] ?? "").trim();
  if (!title) return;
  await store.createItem(listId, title);
  newItemTitles.value[listId] = "";
}

function startEdit(itemId: string, title: string) {
  editingItemId.value = itemId;
  editDraft.value = title;
}

async function commitEdit(itemId: string) {
  if (editingItemId.value !== itemId) return;
  const title = editDraft.value.trim();
  editingItemId.value = null;
  if (!title) return;
  await store.renameItem(itemId, title);
}

function cancelEdit() {
  editingItemId.value = null;
}
</script>

<template>
  <section class="todo-view">
    <header class="toolbar">
      <input
        v-model="newListTitle"
        class="input"
        placeholder="新建列表名称"
        @keyup.enter="addList"
      />
      <button class="btn primary" @click="addList">新建列表</button>
    </header>

    <p v-if="store.error" class="error">{{ store.error }}</p>

    <div class="columns">
      <article v-for="list in store.lists" :key="list.id" class="column">
        <h3 class="column-title">{{ list.title }}</h3>
        <div class="column-add">
          <input
            v-model="newItemTitles[list.id]"
            class="input"
            placeholder="添加待办…"
            @keyup.enter="addItem(list.id)"
          />
        </div>
        <ul class="items">
          <li
            v-for="item in list.items"
            :key="item.id"
            class="item"
            :class="{ done: item.completed }"
          >
            <input
              type="checkbox"
              :checked="item.completed"
              @change="store.toggleItem(item.id, item.completed)"
            />
            <input
              v-if="editingItemId === item.id"
              v-model="editDraft"
              class="input item-edit"
              @keyup.enter="commitEdit(item.id)"
              @keyup.esc="cancelEdit"
              @blur="commitEdit(item.id)"
            />
            <span v-else class="item-title">{{ item.title }}</span>
            <span class="item-actions">
              <button class="btn ghost" @click="startEdit(item.id, item.title)">编辑</button>
              <button class="btn ghost danger" @click="store.softDelete(item.id)">删除</button>
            </span>
          </li>
          <li v-if="list.items.length === 0" class="empty">暂无待办</li>
        </ul>
      </article>
      <EmptyState v-if="store.lists.length === 0" text="还没有列表，先新建一个" />
    </div>
  </section>
</template>
