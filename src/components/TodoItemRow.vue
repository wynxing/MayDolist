<script setup lang="ts">
// One todo row (shared by the pending and completed sections). Editing of
// the title draft is kept local; every write action is emitted to the parent
// so all store mutations stay in one place.
import { ref, watch } from "vue";
import ScheduleEditor from "./ScheduleEditor.vue";
import type { TodoItem, TodoList } from "../types/todo";
import {
  githubSyncClass,
  githubSyncLabel,
  githubSyncTitle,
  isHttpUrl,
  scheduleSummaries,
  sourceLabel,
} from "../todoFormat";

const props = defineProps<{
  item: TodoItem;
  listId: string;
  lists: TodoList[];
  index: number;
  groupCount: number;
  expanded: boolean;
  editing: boolean;
  dropTarget: boolean;
  completing: boolean;
}>();

const emit = defineEmits<{
  "toggle-completed": [];
  "toggle-details": [];
  "start-edit": [];
  "commit-edit": [title: string];
  "cancel-edit": [];
  delete: [];
  "open-source": [];
  "move-to-list": [targetListId: string];
  "move-item": [delta: number];
  error: [message: string];
  dragstart: [event: DragEvent];
  dragover: [event: DragEvent];
  drop: [event: DragEvent];
  dragend: [event: DragEvent];
}>();

const editDraft = ref("");

watch(
  () => props.editing,
  (active) => {
    if (active) editDraft.value = props.item.title;
  },
  { immediate: true }
);

function submitOnEnter(event: KeyboardEvent, action: () => void) {
  if (!event.isComposing) action();
}

function commitEdit() {
  if (!props.editing) return;
  emit("commit-edit", editDraft.value.trim());
}
</script>

<template>
  <li
    class="todo-item"
    :class="{ done: item.completed, 'drop-target': dropTarget, 'is-completing': completing }"
    draggable="true"
    @dragstart="emit('dragstart', $event)"
    @dragover="emit('dragover', $event)"
    @drop="emit('drop', $event)"
    @dragend="emit('dragend', $event)"
  >
    <div class="todo-item-main">
      <input
        :id="`todo-${item.id}`"
        type="checkbox"
        :checked="item.completed"
        @change="emit('toggle-completed')"
      />
      <div class="todo-item-content">
        <input
          v-if="!item.completed && editing"
          v-model="editDraft"
          class="item-edit"
          aria-label="待办标题"
          autofocus
          @keyup.enter="submitOnEnter($event, commitEdit)"
          @keyup.esc="emit('cancel-edit')"
          @blur="commitEdit"
        />
        <label
          v-else
          class="todo-item-title"
          :for="`todo-${item.id}`"
          @dblclick="!item.completed && emit('start-edit')"
        >
          {{ item.title }}
        </label>
        <div v-if="item.source || scheduleSummaries(item).length" class="todo-item-meta">
          <span v-if="item.source" class="todo-source" :title="item.source.url">
            {{ sourceLabel(item.source) }}
          </span>
          <span
            v-if="item.source && githubSyncLabel(item)"
            class="todo-source-state"
            :class="githubSyncClass(item)"
            :title="githubSyncTitle(item)"
          >
            {{ githubSyncLabel(item) }}
          </span>
          <span
            v-for="summary in scheduleSummaries(item)"
            :key="summary"
            class="todo-schedule-summary"
          >
            {{ summary }}
          </span>
        </div>
      </div>
      <button
        class="todo-details-toggle"
        type="button"
        :aria-expanded="expanded"
        :aria-controls="`todo-details-${item.id}`"
        :aria-label="`${expanded ? '收起' : '展开'}待办“${item.title}”的详细设置`"
        @click="emit('toggle-details')"
      >
        <span aria-hidden="true">{{ expanded ? "⌃" : "⌄" }}</span>
      </button>
    </div>
    <div
      v-if="expanded"
      :id="`todo-details-${item.id}`"
      class="todo-item-details"
      :class="{ 'completed-details': item.completed }"
    >
      <ScheduleEditor v-if="!item.completed" :item="item" @error="emit('error', $event)" />
      <div class="todo-item-actions">
        <button
          v-if="!item.completed"
          class="text-action"
          type="button"
          @click="emit('start-edit')"
        >
          编辑
        </button>
        <button
          v-if="item.source && isHttpUrl(item.source.url)"
          class="text-action"
          type="button"
          :title="`打开来源 ${item.source.url}`"
          @click="emit('open-source')"
        >
          打开来源
        </button>
        <template v-if="!item.completed">
          <select
            class="move-select"
            :value="listId"
            :aria-label="`移动${item.title}到其他清单`"
            @change="emit('move-to-list', ($event.target as HTMLSelectElement).value)"
          >
            <option v-for="target in lists" :key="target.id" :value="target.id">
              {{ target.id === listId ? "移动到…" : target.title }}
            </option>
          </select>
          <button
            class="text-action"
            type="button"
            :disabled="index === 0"
            @click="emit('move-item', -1)"
          >
            上移
          </button>
          <button
            class="text-action"
            type="button"
            :disabled="index === groupCount - 1"
            @click="emit('move-item', 1)"
          >
            下移
          </button>
        </template>
        <button class="text-action danger" type="button" @click="emit('delete')">删除</button>
      </div>
    </div>
  </li>
</template>
