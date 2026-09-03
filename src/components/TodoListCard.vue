<script setup lang="ts">
// One list card: header (drag / rename / reorder / delete), the add-item
// form, pending items and the collapsible completed section. All store
// writes are emitted to the parent.
import { ref, watch } from "vue";
import TodoItemRow from "./TodoItemRow.vue";
import type { TodoItem, TodoList } from "../types/todo";

const props = defineProps<{
  list: TodoList;
  listIndex: number;
  totalLists: number;
  lists: TodoList[];
  pending: TodoItem[];
  completed: TodoItem[];
  editingListTitle: boolean;
  editingItemId: string | null;
  expandedItemId: string | null;
  draggingList: boolean;
  dropListTarget: boolean;
  dropItemId: string | null;
  completingIds: Set<string>;
}>();

const emit = defineEmits<{
  "start-list-edit": [];
  "commit-list-edit": [title: string];
  "cancel-list-edit": [];
  "delete-list": [];
  "move-list": [delta: number];
  "add-item": [title: string];
  "toggle-item-completed": [item: TodoItem];
  "toggle-item-details": [item: TodoItem];
  "start-item-edit": [itemId: string];
  "commit-item-edit": [itemId: string, title: string];
  "cancel-item-edit": [];
  "delete-item": [item: TodoItem];
  "open-source": [item: TodoItem];
  "move-to-list": [itemId: string, targetListId: string];
  "move-item": [itemId: string, delta: number];
  error: [message: string];
  "list-dragstart": [event: DragEvent];
  "list-dragover": [event: DragEvent];
  "list-drop": [event: DragEvent];
  "list-dragend": [];
  "item-dragstart": [event: DragEvent, itemId: string];
  "item-dragover": [event: DragEvent, itemId: string | undefined];
  "item-drop": [event: DragEvent, itemId: string | undefined];
  "item-dragend": [];
}>();

const addDraft = ref("");
const completedOpen = ref(false);
const titleDraft = ref("");

watch(
  () => props.editingListTitle,
  (active) => {
    if (active) titleDraft.value = props.list.title;
  },
  { immediate: true }
);

function submitOnEnter(event: KeyboardEvent, action: () => void) {
  if (!event.isComposing) action();
}

function commitListEdit() {
  if (!props.editingListTitle) return;
  emit("commit-list-edit", titleDraft.value.trim());
}

function submitAddItem() {
  const title = addDraft.value.trim();
  if (!title) return;
  emit("add-item", title);
  addDraft.value = "";
}
</script>

<template>
  <article
    class="todo-group glass-card"
    :class="{ dragging: draggingList, 'drop-target': dropListTarget }"
    @dragover.self="emit('list-dragover', $event)"
    @drop.self="emit('list-drop', $event)"
  >
    <header class="todo-group-header">
      <button
        class="drag-handle"
        type="button"
        draggable="true"
        aria-label="拖动清单排序"
        title="拖动清单排序"
        @dragstart="emit('list-dragstart', $event)"
        @dragend="emit('list-dragend')"
      >
        拖动
      </button>
      <input
        v-if="editingListTitle"
        v-model="titleDraft"
        class="input list-title-edit"
        aria-label="清单名称"
        autofocus
        @keyup.enter="submitOnEnter($event, commitListEdit)"
        @keyup.esc="emit('cancel-list-edit')"
        @blur="commitListEdit"
      />
      <div v-else class="todo-group-heading">
        <h2>{{ list.title }}</h2>
        <span>{{ pending.length }} 项未完成</span>
      </div>
      <div class="group-actions">
        <button class="btn ghost compact" type="button" @click="emit('start-list-edit')">
          重命名
        </button>
        <button
          class="btn ghost compact"
          type="button"
          :disabled="listIndex === 0"
          @click="emit('move-list', -1)"
        >
          上移
        </button>
        <button
          class="btn ghost compact"
          type="button"
          :disabled="listIndex === totalLists - 1"
          @click="emit('move-list', 1)"
        >
          下移
        </button>
        <button class="btn ghost compact danger" type="button" @click="emit('delete-list')">
          删除
        </button>
      </div>
    </header>

    <slot name="confirm" />

    <form class="todo-add-row" @submit.prevent="submitAddItem">
      <label class="sr-only" :for="`new-item-${list.id}`">添加到 {{ list.title }}</label>
      <input
        :id="`new-item-${list.id}`"
        v-model="addDraft"
        class="input"
        placeholder="添加待办，按 Enter 保存"
      />
      <button class="btn primary" type="submit" :disabled="!addDraft.trim()">添加</button>
    </form>

    <ul class="todo-items" :aria-label="`${list.title}中的未完成待办`">
      <TodoItemRow
        v-for="(item, itemIndex) in pending"
        :key="item.id"
        :item="item"
        :list-id="list.id"
        :lists="lists"
        :index="itemIndex"
        :group-count="pending.length"
        :expanded="expandedItemId === item.id"
        :editing="editingItemId === item.id"
        :drop-target="dropItemId === item.id"
        :completing="completingIds.has(item.id)"
        @toggle-completed="emit('toggle-item-completed', item)"
        @toggle-details="emit('toggle-item-details', item)"
        @start-edit="emit('start-item-edit', item.id)"
        @commit-edit="emit('commit-item-edit', item.id, $event)"
        @cancel-edit="emit('cancel-item-edit')"
        @delete="emit('delete-item', item)"
        @open-source="emit('open-source', item)"
        @move-to-list="emit('move-to-list', item.id, $event)"
        @move-item="emit('move-item', item.id, $event)"
        @error="emit('error', $event)"
        @dragstart="emit('item-dragstart', $event, item.id)"
        @dragover="emit('item-dragover', $event, item.id)"
        @drop="emit('item-drop', $event, item.id)"
        @dragend="emit('item-dragend')"
      />
      <li
        v-if="pending.length === 0"
        class="todo-empty-drop"
        @dragover="emit('item-dragover', $event, undefined)"
        @drop="emit('item-drop', $event, undefined)"
      >
        {{ completed.length ? "这一组已经全部完成" : "暂无待办，从上方添加一项" }}
      </li>
    </ul>

    <section v-if="completed.length" class="completed-section">
      <button
        class="completed-toggle"
        type="button"
        :aria-expanded="completedOpen"
        @click="completedOpen = !completedOpen"
      >
        <span>已完成</span>
        <span>{{ completed.length }} 项 · {{ completedOpen ? "收起" : "展开" }}</span>
      </button>
      <ul v-if="completedOpen" class="todo-items completed-items">
        <TodoItemRow
          v-for="item in completed"
          :key="item.id"
          :item="item"
          :list-id="list.id"
          :lists="lists"
          :index="0"
          :group-count="completed.length"
          :expanded="expandedItemId === item.id"
          :editing="false"
          :drop-target="dropItemId === item.id"
          :completing="completingIds.has(item.id)"
          @toggle-completed="emit('toggle-item-completed', item)"
          @toggle-details="emit('toggle-item-details', item)"
          @delete="emit('delete-item', item)"
          @open-source="emit('open-source', item)"
          @error="emit('error', $event)"
          @dragstart="emit('item-dragstart', $event, item.id)"
          @dragover="emit('item-dragover', $event, item.id)"
          @drop="emit('item-drop', $event, item.id)"
          @dragend="emit('item-dragend')"
        />
      </ul>
    </section>
  </article>
</template>
