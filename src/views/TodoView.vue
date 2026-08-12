<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import EmptyState from "../components/EmptyState.vue";
import { open } from "../api/github";
import type { TodoScheduleInput } from "../api/todo";
import { useTodoStore } from "../stores/todo";
import {
  advanceAfterAction,
  enterTriage,
  isTriageDone,
  reconcileTriage,
  triageDueDate,
  triageKeyToAction,
  triagePending,
  triageRemainingCount,
  type TriageAction,
} from "../triage";
import type { RepeatRule, TodoItem, TodoList, TodoSource } from "../types/todo";

const store = useTodoStore();
const actionError = ref("");
const newList = ref("");
const drafts = ref<Record<string, string>>({});
const completedOpen = ref<Record<string, boolean>>({});
const editingItemId = ref<string | null>(null);
const editingListId = ref<string | null>(null);
const editDraft = ref("");
const dragListId = ref<string | null>(null);
const dropListId = ref<string | null>(null);
const dragItem = ref<{ from: string; id: string } | null>(null);
const dropItemId = ref<string | null>(null);

const hasLists = computed(() => store.lists.length > 0);

function pendingItems(list: TodoList) {
  return list.items.filter((item) => !item.completed);
}

function completedItems(list: TodoList) {
  return list.items.filter((item) => item.completed);
}

async function addList() {
  const title = newList.value.trim();
  if (!title) return;
  await store.createList(title);
  newList.value = "";
}

async function addItem(listId: string) {
  const title = drafts.value[listId]?.trim();
  if (!title) return;
  await store.createItem(listId, title);
  drafts.value[listId] = "";
}

function submitOnEnter(event: KeyboardEvent, action: () => void) {
  if (!event.isComposing) action();
}

function startItemEdit(item: TodoItem) {
  editingItemId.value = item.id;
  editDraft.value = item.title;
}

async function commitItemEdit(item: TodoItem) {
  if (editingItemId.value !== item.id) return;
  const title = editDraft.value.trim();
  editingItemId.value = null;
  if (title && title !== item.title) await store.renameItem(item.id, title);
}

function cancelItemEdit() {
  editingItemId.value = null;
  editDraft.value = "";
}

function startListEdit(list: TodoList) {
  editingListId.value = list.id;
  editDraft.value = list.title;
}

async function commitListEdit(list: TodoList) {
  if (editingListId.value !== list.id) return;
  const title = editDraft.value.trim();
  editingListId.value = null;
  if (title && title !== list.title) await store.updateList(list.id, { title });
}

function cancelListEdit() {
  editingListId.value = null;
  editDraft.value = "";
}

async function deleteList(list: TodoList) {
  if (!window.confirm(`将清单“${list.title}”移入回收站？`)) return;
  await store.updateList(list.id, { deleted: true });
}

async function deleteItem(item: TodoItem) {
  if (!window.confirm(`将待办“${item.title}”移入回收站？`)) return;
  await store.softDelete(item.id);
}

/* ------------------------------------------------------------------ *
 * Inbox triage mode (#28): a pure view mode that shows one pending
 * inbox item at a time. All actions reuse existing commands / services
 * (todo_update_item / todo_move_item / todo_soft_delete), so no new
 * write path and no new persisted field exists.
 * ------------------------------------------------------------------ */
const inboxList = computed(() => {
  const byKind = store.lists.find((list) => list.kind === "inbox");
  if (byKind) return byKind;
  return store.lists.find((list) => list.title === "收件箱") ?? null;
});
const inboxPending = computed(() => triagePending(inboxList.value?.items ?? []));

const triageActive = ref(false);
const triageTotalIds = ref<string[]>([]);
const triageRemainingIds = ref<string[]>([]);
const triageCurrentId = ref<string | null>(null);
const triageError = ref("");
const triageBusy = ref(false);
const movePickerOpen = ref(false);
const moveTargetListId = ref("");
const triageCardEl = ref<HTMLElement | null>(null);

const triageCurrent = computed(() => {
  if (!triageCurrentId.value) return null;
  return inboxPending.value.find((item) => item.id === triageCurrentId.value) ?? null;
});
const triageRemaining = computed(() =>
  triageRemainingCount(triageRemainingIds.value, inboxPending.value)
);
const triageDone = computed(() => isTriageDone(triageRemainingIds.value, inboxPending.value));
const triageProgress = computed(() => {
  if (triageTotalIds.value.length === 0) return 100;
  return Math.round((1 - triageRemaining.value / triageTotalIds.value.length) * 100);
});
const triageMoveTargets = computed(() =>
  store.lists.filter((list) => list.id !== inboxList.value?.id)
);

function repeatLabel(rule: RepeatRule) {
  switch (rule) {
    case "daily":
      return "每天";
    case "weekly":
      return "每周";
    case "biweekly":
      return "每两周";
    case "monthly":
      return "每月";
  }
}

function startTriage() {
  if (!inboxList.value) return;
  const state = enterTriage(inboxPending.value);
  triageTotalIds.value = [...state.remainingIds];
  triageRemainingIds.value = state.remainingIds;
  triageCurrentId.value = state.currentId;
  triageError.value = "";
  triageActive.value = true;
  void nextTick(() => triageCardEl.value?.focus());
}

function exitTriage() {
  triageActive.value = false;
  triageRemainingIds.value = [];
  triageCurrentId.value = null;
  movePickerOpen.value = false;
  triageError.value = "";
}

function advanceTriage() {
  const state = advanceAfterAction(
    triageRemainingIds.value,
    triageCurrentId.value,
    inboxPending.value
  );
  triageRemainingIds.value = state.remainingIds;
  triageCurrentId.value = state.currentId;
  void nextTick(() => triageCardEl.value?.focus());
}

async function runTriageAction(action: TriageAction) {
  if (!triageCurrent.value || triageBusy.value) return;
  const item = triageCurrent.value;
  triageError.value = "";
  if (action === "move") {
    moveTargetListId.value = triageMoveTargets.value[0]?.id ?? "";
    movePickerOpen.value = true;
    return;
  }
  triageBusy.value = true;
  try {
    if (action === "today" || action === "later") {
      const schedule = scheduleOf(item);
      schedule.dueDate = triageDueDate(new Date(), action === "today" ? 0 : 3);
      await store.patchItem(item.id, { schedule });
    } else if (action === "complete") {
      await store.patchItem(item.id, { completed: true });
    } else if (action === "delete") {
      await store.softDelete(item.id);
    }
    advanceTriage();
  } catch (err) {
    // 写盘失败：停留在当前条目并显示错误，不静默丢失。
    triageError.value = String(err);
  } finally {
    triageBusy.value = false;
  }
}

async function confirmMove() {
  const item = triageCurrent.value;
  const target = store.lists.find((list) => list.id === moveTargetListId.value);
  if (!item || !target || triageBusy.value) return;
  triageError.value = "";
  triageBusy.value = true;
  try {
    await store.moveItem(item.id, target.id, target.items.length);
    movePickerOpen.value = false;
    advanceTriage();
  } catch (err) {
    triageError.value = String(err);
  } finally {
    triageBusy.value = false;
  }
}

function onTriageKeydown(event: KeyboardEvent) {
  if (!triageActive.value) return;
  if (event.key === "Escape") {
    event.preventDefault();
    if (movePickerOpen.value) {
      movePickerOpen.value = false;
    } else {
      exitTriage();
    }
    return;
  }
  if (movePickerOpen.value) return;
  const action = triageKeyToAction(
    event.key,
    event.isComposing || event.keyCode === 229,
    event.keyCode
  );
  if (!action) return;
  event.preventDefault();
  void runTriageAction(action);
}

// 处理过程中列表被其他窗口修改时，光标与计数自动校正（基于稳定 id）。
watch(inboxPending, () => {
  if (!triageActive.value) return;
  const state = reconcileTriage(
    triageRemainingIds.value,
    triageCurrentId.value,
    inboxPending.value
  );
  triageRemainingIds.value = state.remainingIds;
  triageCurrentId.value = state.currentId;
});

onMounted(() => {
  store.init();
  window.addEventListener("keydown", onTriageKeydown);
});
onBeforeUnmount(() => window.removeEventListener("keydown", onTriageKeydown));

function sourceLabel(source: TodoSource) {
  const kind =
    source.type === "github-pr" ? "PR" : source.type === "github-issue" ? "Issue" : source.type;
  return `${kind} ${source.repo}#${source.number}`;
}

function isHttpUrl(url: string) {
  return /^https?:\/\//i.test(url);
}

function pad2(value: number) {
  return String(value).padStart(2, "0");
}

function localDateValue(value: string | null | undefined) {
  if (!value) return "";
  const d = new Date(value.length === 10 ? `${value}T00:00:00` : value);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

function localDateTimeValue(value: string | null | undefined) {
  if (!value) return "";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}T${pad2(
    d.getHours()
  )}:${pad2(d.getMinutes())}`;
}

function scheduleOf(item: TodoItem): TodoScheduleInput {
  return {
    dueDate: item.dueDate ?? null,
    remindAt: item.remindAt ?? null,
    repeat: item.repeat ?? null,
    repeatUntil: item.repeatUntil ?? null,
  };
}

async function patchSchedule(item: TodoItem, schedule: TodoScheduleInput) {
  actionError.value = "";
  try {
    await store.patchItem(item.id, { schedule });
  } catch (err) {
    actionError.value = String(err);
  }
}

async function setDueDate(item: TodoItem, value: string) {
  const schedule = scheduleOf(item);
  schedule.dueDate = value || null;
  if (!schedule.dueDate) schedule.remindAt = null;
  await patchSchedule(item, schedule);
}

async function setRemindAt(item: TodoItem, value: string) {
  const schedule = scheduleOf(item);
  schedule.remindAt = value ? new Date(value).toISOString() : null;
  await patchSchedule(item, schedule);
}

async function setRepeat(item: TodoItem, value: string) {
  const schedule = scheduleOf(item);
  schedule.repeat = (value || null) as RepeatRule | null;
  if (!schedule.repeat) schedule.repeatUntil = null;
  await patchSchedule(item, schedule);
}

async function openSource(item: TodoItem) {
  if (!item.source || !isHttpUrl(item.source.url)) return;
  actionError.value = "";
  try {
    await open(item.source.url);
  } catch (err) {
    actionError.value = String(err);
  }
}

async function moveList(id: string, delta: number) {
  const ids = store.lists.map((list) => list.id);
  const from = ids.indexOf(id);
  const to = from + delta;
  if (to < 0 || to >= ids.length) return;
  [ids[from], ids[to]] = [ids[to], ids[from]];
  await store.reorderLists(ids);
}

async function moveItem(listId: string, id: string, delta: number) {
  const list = store.lists.find((value) => value.id === listId);
  if (!list) return;
  const current = list.items.find((item) => item.id === id);
  if (!current) return;
  const visibleIds = list.items
    .filter((item) => item.completed === current.completed)
    .map((item) => item.id);
  const visibleFrom = visibleIds.indexOf(id);
  const visibleTo = visibleFrom + delta;
  if (visibleTo < 0 || visibleTo >= visibleIds.length) return;
  const ids = list.items.map((item) => item.id);
  const from = ids.indexOf(id);
  const to = ids.indexOf(visibleIds[visibleTo]);
  [ids[from], ids[to]] = [ids[to], ids[from]];
  await store.reorderItems(listId, ids);
}

async function moveToList(itemId: string, targetListId: string) {
  const target = store.lists.find((list) => list.id === targetListId);
  if (!target) return;
  await store.moveItem(itemId, targetListId, target.items.length);
}

function onListDragStart(event: DragEvent, id: string) {
  dragListId.value = id;
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

function onListDragOver(event: DragEvent, targetId: string) {
  if (!dragListId.value || dragListId.value === targetId) return;
  event.preventDefault();
  dropListId.value = targetId;
}

function onListDrop(targetId: string) {
  const from = dragListId.value;
  onListDragEnd();
  if (!from || from === targetId) return;
  const ids = store.lists.map((list) => list.id);
  const fromIndex = ids.indexOf(from);
  const toIndex = ids.indexOf(targetId);
  if (fromIndex < 0 || toIndex < 0) return;
  ids.splice(fromIndex, 1);
  ids.splice(toIndex, 0, from);
  void store.reorderLists(ids);
}

function onListDragEnd() {
  dragListId.value = null;
  dropListId.value = null;
}

function onItemDragStart(event: DragEvent, listId: string, itemId: string) {
  event.stopPropagation();
  dragItem.value = { from: listId, id: itemId };
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

function onItemDragOver(event: DragEvent, targetItemId?: string) {
  if (!dragItem.value) return;
  event.preventDefault();
  event.stopPropagation();
  dropItemId.value = targetItemId ?? null;
}

function onItemDrop(event: DragEvent, targetListId: string, targetItemId?: string) {
  const dragged = dragItem.value;
  onItemDragEnd();
  if (!dragged) return;
  event.preventDefault();
  event.stopPropagation();
  const targetList = store.lists.find((list) => list.id === targetListId);
  if (!targetList) return;

  if (dragged.from === targetListId) {
    if (!targetItemId || targetItemId === dragged.id) return;
    const ids = targetList.items.map((item) => item.id).filter((id) => id !== dragged.id);
    const insertAt = ids.indexOf(targetItemId);
    if (insertAt < 0) return;
    ids.splice(insertAt, 0, dragged.id);
    void store.reorderItems(targetListId, ids);
    return;
  }

  const targetIndex = targetItemId
    ? targetList.items.findIndex((item) => item.id === targetItemId)
    : targetList.items.length;
  if (targetIndex >= 0) void store.moveItem(dragged.id, targetListId, targetIndex);
}

function onItemDragEnd() {
  dragItem.value = null;
  dropItemId.value = null;
}
</script>

<template>
  <section class="todo-view" aria-labelledby="todo-heading">
    <header class="todo-topbar">
      <div>
        <h1 id="todo-heading">我的待办</h1>
        <p>把今天要做的事收进清单。</p>
      </div>
      <div class="todo-topbar-actions">
        <form class="todo-create-list" @submit.prevent="addList">
          <label class="sr-only" for="new-list">新清单名称</label>
          <input id="new-list" v-model="newList" class="input" placeholder="新建清单" />
          <button class="btn primary" type="submit">新建清单</button>
        </form>
        <button
          v-if="!triageActive && inboxList"
          class="btn primary"
          type="button"
          :disabled="inboxPending.length === 0"
          title="进入收件箱逐条处理模式"
          @click="startTriage"
        >
          处理模式
        </button>
      </div>
    </header>

    <p v-if="store.error" class="error" role="alert">{{ store.error }}</p>
    <p v-if="actionError" class="error" role="alert">{{ actionError }}</p>

    <section v-if="triageActive" class="triage" aria-label="收件箱处理模式">
      <header class="triage-header">
        <div class="triage-heading">
          <h2>收件箱处理模式</h2>
          <p>剩余 {{ triageRemaining }} 条 · 共 {{ triageTotalIds.length }} 条</p>
        </div>
        <button class="btn ghost compact" type="button" @click="exitTriage">退出（Esc）</button>
      </header>

      <div
        class="triage-progress"
        role="progressbar"
        aria-label="处理进度"
        aria-valuemin="0"
        aria-valuemax="100"
        :aria-valuenow="triageProgress"
      >
        <div class="triage-progress-fill" :style="{ width: `${triageProgress}%` }"></div>
      </div>

      <p v-if="triageError" class="error" role="alert">{{ triageError }}</p>
      <p v-if="!inboxList" class="error" role="alert">收件箱已不存在，已退出处理模式。</p>

      <div v-if="triageDone" class="triage-done">
        <p class="triage-done-title">收件箱已清空</p>
        <p v-if="inboxPending.length > 0" class="triage-done-hint">
          其他窗口或周期任务新增的条目保留在列表中，退出后可继续处理。
        </p>
        <button class="btn primary" type="button" @click="exitTriage">返回列表</button>
      </div>

      <template v-else-if="triageCurrent">
        <article ref="triageCardEl" class="triage-card glass-card" tabindex="0">
          <p class="triage-card-title">{{ triageCurrent.title }}</p>
          <div class="triage-card-meta">
            <span
              v-if="triageCurrent.source"
              class="todo-source"
              :title="triageCurrent.source.url"
            >
              {{ sourceLabel(triageCurrent.source) }}
            </span>
            <span v-if="triageCurrent.dueDate" class="triage-due">
              到期 {{ localDateValue(triageCurrent.dueDate) }}
            </span>
            <span v-if="triageCurrent.repeat" class="triage-repeat">
              重复 · {{ repeatLabel(triageCurrent.repeat) }}
            </span>
          </div>
        </article>

        <div class="triage-actions" role="group" aria-label="处理动作">
          <button
            class="btn"
            type="button"
            :disabled="triageBusy"
            @click="runTriageAction('today')"
          >
            <kbd>1</kbd> 今天做
          </button>
          <button
            class="btn"
            type="button"
            :disabled="triageBusy"
            @click="runTriageAction('later')"
          >
            <kbd>2</kbd> 稍后做
          </button>
          <button
            class="btn"
            type="button"
            :disabled="triageBusy"
            @click="runTriageAction('move')"
          >
            <kbd>3</kbd> 转列表
          </button>
          <button
            class="btn"
            type="button"
            :disabled="triageBusy"
            @click="runTriageAction('complete')"
          >
            <kbd>4</kbd> 完成
          </button>
          <button
            class="btn danger"
            type="button"
            :disabled="triageBusy"
            @click="runTriageAction('delete')"
          >
            <kbd>5</kbd> 删除
          </button>
        </div>

        <div v-if="movePickerOpen" class="triage-move-picker">
          <label class="triage-move-label" for="triage-move-target">转到列表</label>
          <select id="triage-move-target" v-model="moveTargetListId" class="input">
            <option v-for="target in triageMoveTargets" :key="target.id" :value="target.id">
              {{ target.title }}
            </option>
          </select>
          <button
            class="btn primary"
            type="button"
            :disabled="!moveTargetListId || triageBusy"
            @click="confirmMove"
          >
            确认
          </button>
          <button class="btn ghost" type="button" @click="movePickerOpen = false">取消</button>
        </div>
        <p v-else class="triage-hint">
          按 <kbd>1</kbd>–<kbd>5</kbd> 执行动作，按 <kbd>Esc</kbd> 退出；完成可取消、删除可在回收站恢复。
        </p>
      </template>
    </section>

    <template v-else>
      <div v-if="hasLists" class="todo-groups">
      <article
        v-for="(list, listIndex) in store.lists"
        :key="list.id"
        class="todo-group glass-card"
        :class="{ dragging: dragListId === list.id, 'drop-target': dropListId === list.id }"
        @dragover.self="onListDragOver($event, list.id)"
        @drop.self="onListDrop(list.id)"
      >
        <header class="todo-group-header">
          <button
            class="drag-handle"
            type="button"
            draggable="true"
            aria-label="拖动清单排序"
            title="拖动清单排序"
            @dragstart="onListDragStart($event, list.id)"
            @dragend="onListDragEnd"
          >
            拖动
          </button>
          <input
            v-if="editingListId === list.id"
            v-model="editDraft"
            class="input list-title-edit"
            aria-label="清单名称"
            autofocus
            @keyup.enter="submitOnEnter($event, () => commitListEdit(list))"
            @keyup.esc="cancelListEdit"
            @blur="commitListEdit(list)"
          />
          <div v-else class="todo-group-heading">
            <h2>{{ list.title }}</h2>
            <span>{{ pendingItems(list).length }} 项未完成</span>
          </div>
          <div class="group-actions">
            <button class="btn ghost compact" type="button" @click="startListEdit(list)">重命名</button>
            <button
              class="btn ghost compact"
              type="button"
              :disabled="listIndex === 0"
              @click="moveList(list.id, -1)"
            >
              上移
            </button>
            <button
              class="btn ghost compact"
              type="button"
              :disabled="listIndex === store.lists.length - 1"
              @click="moveList(list.id, 1)"
            >
              下移
            </button>
            <button class="btn ghost compact danger" type="button" @click="deleteList(list)">删除</button>
          </div>
        </header>

        <form class="todo-add-row" @submit.prevent="addItem(list.id)">
          <label class="sr-only" :for="`new-item-${list.id}`">添加到 {{ list.title }}</label>
          <input
            :id="`new-item-${list.id}`"
            v-model="drafts[list.id]"
            class="input"
            placeholder="添加待办，按 Enter 保存"
          />
          <button class="btn primary" type="submit" :disabled="!drafts[list.id]?.trim()">添加</button>
        </form>

        <ul class="todo-items" :aria-label="`${list.title}中的未完成待办`">
          <li
            v-for="(item, itemIndex) in pendingItems(list)"
            :key="item.id"
            class="todo-item"
            :class="{ 'drop-target': dropItemId === item.id }"
            draggable="true"
            @dragstart="onItemDragStart($event, list.id, item.id)"
            @dragover="onItemDragOver($event, item.id)"
            @drop="onItemDrop($event, list.id, item.id)"
            @dragend="onItemDragEnd"
          >
            <input
              :id="`todo-${item.id}`"
              type="checkbox"
              :checked="item.completed"
              @change="store.toggleItem(item.id, item.completed)"
            />
            <input
              v-if="editingItemId === item.id"
              v-model="editDraft"
              class="item-edit"
              aria-label="待办标题"
              autofocus
              @keyup.enter="submitOnEnter($event, () => commitItemEdit(item))"
              @keyup.esc="cancelItemEdit"
              @blur="commitItemEdit(item)"
            />
            <label v-else class="todo-item-title" :for="`todo-${item.id}`" @dblclick="startItemEdit(item)">
              {{ item.title }}
            </label>
            <span v-if="item.source" class="todo-source" :title="item.source.url">
              {{ sourceLabel(item.source) }}
            </span>
            <div class="todo-item-schedule">
              <input
                type="date"
                :value="localDateValue(item.dueDate)"
                :aria-label="`${item.title} 到期日`"
                title="到期日"
                @change="setDueDate(item, ($event.target as HTMLInputElement).value)"
              />
              <input
                type="datetime-local"
                :value="localDateTimeValue(item.remindAt)"
                :aria-label="`${item.title} 提醒时间`"
                title="提醒时间（需设置到期日）"
                :disabled="!item.dueDate"
                @change="setRemindAt(item, ($event.target as HTMLInputElement).value)"
              />
              <select
                :value="item.repeat ?? ''"
                :aria-label="`${item.title} 重复规则`"
                title="重复规则"
                @change="setRepeat(item, ($event.target as HTMLSelectElement).value)"
              >
                <option value="">不重复</option>
                <option value="daily">每天</option>
                <option value="weekly">每周</option>
                <option value="biweekly">每两周</option>
                <option value="monthly">每月</option>
              </select>
            </div>
            <div class="todo-item-actions">
              <button class="text-action" type="button" @click="startItemEdit(item)">编辑</button>
              <button
                v-if="item.source && isHttpUrl(item.source.url)"
                class="text-action"
                type="button"
                :title="`打开来源 ${item.source.url}`"
                @click="openSource(item)"
              >
                打开来源
              </button>
              <select
                class="move-select"
                :value="list.id"
                :aria-label="`移动${item.title}到其他清单`"
                @change="moveToList(item.id, ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="target in store.lists" :key="target.id" :value="target.id">
                  {{ target.id === list.id ? "移动到…" : target.title }}
                </option>
              </select>
              <button
                class="text-action"
                type="button"
                :disabled="itemIndex === 0"
                @click="moveItem(list.id, item.id, -1)"
              >
                上移
              </button>
              <button
                class="text-action"
                type="button"
                :disabled="itemIndex === pendingItems(list).length - 1"
                @click="moveItem(list.id, item.id, 1)"
              >
                下移
              </button>
              <button class="text-action danger" type="button" @click="deleteItem(item)">删除</button>
            </div>
          </li>
          <li
            v-if="pendingItems(list).length === 0"
            class="todo-empty-drop"
            @dragover="onItemDragOver($event)"
            @drop="onItemDrop($event, list.id)"
          >
            {{ completedItems(list).length ? "这一组已经全部完成" : "暂无待办，从上方添加一项" }}
          </li>
        </ul>

        <section v-if="completedItems(list).length" class="completed-section">
          <button
            class="completed-toggle"
            type="button"
            :aria-expanded="completedOpen[list.id] === true"
            @click="completedOpen[list.id] = !completedOpen[list.id]"
          >
            <span>已完成</span>
            <span>{{ completedItems(list).length }} 项 · {{ completedOpen[list.id] ? "收起" : "展开" }}</span>
          </button>
          <ul v-if="completedOpen[list.id]" class="todo-items completed-items">
            <li
              v-for="item in completedItems(list)"
              :key="item.id"
              class="todo-item done"
              :class="{ 'drop-target': dropItemId === item.id }"
              draggable="true"
              @dragstart="onItemDragStart($event, list.id, item.id)"
              @dragover="onItemDragOver($event, item.id)"
              @drop="onItemDrop($event, list.id, item.id)"
              @dragend="onItemDragEnd"
            >
              <input
                :id="`todo-${item.id}`"
                type="checkbox"
                :checked="item.completed"
                @change="store.toggleItem(item.id, item.completed)"
              />
              <label class="todo-item-title" :for="`todo-${item.id}`">{{ item.title }}</label>
              <span v-if="item.source" class="todo-source" :title="item.source.url">
                {{ sourceLabel(item.source) }}
              </span>
              <div class="todo-item-actions">
                <button
                  v-if="item.source && isHttpUrl(item.source.url)"
                  class="text-action"
                  type="button"
                  :title="`打开来源 ${item.source.url}`"
                  @click="openSource(item)"
                >
                  打开来源
                </button>
                <button class="text-action danger" type="button" @click="deleteItem(item)">删除</button>
              </div>
            </li>
          </ul>
        </section>
      </article>
      </div>

      <EmptyState v-else text="还没有清单，先新建一个，把第一件事记下来。" />
    </template>
  </section>
</template>
