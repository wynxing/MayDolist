<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import ConfirmBar from "../components/ConfirmBar.vue";
import EmptyState from "../components/EmptyState.vue";
import PageHeader from "../components/PageHeader.vue";
import TriageMode from "../components/TriageMode.vue";
import TodoListCard from "../components/TodoListCard.vue";
import { open } from "../api/github";
import { useTodoStore } from "../stores/todo";
import { triagePending } from "../triage";
import { isHttpUrl } from "../todoFormat";
import type { TodoItem, TodoList } from "../types/todo";

const store = useTodoStore();
const actionError = ref("");
const newList = ref("");
const editingItemId = ref<string | null>(null);
const expandedItemId = ref<string | null>(null);
const expandedItemListId = ref<string | null>(null);
const expandedItemCompleted = ref<boolean | null>(null);
const editingListId = ref<string | null>(null);
const dragListId = ref<string | null>(null);
const dropListId = ref<string | null>(null);
const dragItem = ref<{ from: string; id: string } | null>(null);
const dropItemId = ref<string | null>(null);
const pendingConfirm = ref<{ kind: "list" | "item"; id: string; title: string } | null>(null);
const completingIds = ref<Set<string>>(new Set());
const triageActive = ref(false);

const hasLists = computed(() => store.lists.length > 0);

// Memoized pending/completed partition: built once per store.lists change
// instead of re-filtering on every template function call.
const partitionedItems = computed(() => {
  const map = new Map<string, { pending: TodoItem[]; completed: TodoItem[] }>();
  for (const list of store.lists) {
    map.set(list.id, {
      pending: list.items.filter((item) => !item.completed),
      completed: list.items.filter((item) => item.completed),
    });
  }
  return map;
});

const emptyItems: TodoItem[] = [];

function pendingItems(list: TodoList) {
  return partitionedItems.value.get(list.id)?.pending ?? emptyItems;
}

function completedItems(list: TodoList) {
  return partitionedItems.value.get(list.id)?.completed ?? emptyItems;
}

const inboxList = computed(() => {
  const byKind = store.lists.find((list) => list.kind === "inbox");
  if (byKind) return byKind;
  return store.lists.find((list) => list.title === "收件箱") ?? null;
});
const inboxPending = computed(() => triagePending(inboxList.value?.items ?? []));

function closeItemDetails(itemId?: string) {
  if (itemId && expandedItemId.value !== itemId) return;
  expandedItemId.value = null;
  expandedItemListId.value = null;
  expandedItemCompleted.value = null;
}

function toggleItemDetails(listId: string, item: TodoItem) {
  if (expandedItemId.value === item.id) {
    closeItemDetails();
    return;
  }
  expandedItemId.value = item.id;
  expandedItemListId.value = listId;
  expandedItemCompleted.value = item.completed;
}

async function addList() {
  const title = newList.value.trim();
  if (!title) return;
  await store.createList(title);
  newList.value = "";
}

function findItem(itemId: string) {
  for (const list of store.lists) {
    const item = list.items.find((candidate) => candidate.id === itemId);
    if (item) return item;
  }
  return null;
}

async function commitItemEdit(itemId: string, title: string) {
  if (editingItemId.value !== itemId) return;
  editingItemId.value = null;
  const item = findItem(itemId);
  if (item && title && title !== item.title) await store.renameItem(itemId, title);
}

function startListEdit(list: TodoList) {
  editingListId.value = list.id;
}

async function commitListEdit(list: TodoList, title: string) {
  if (editingListId.value !== list.id) return;
  editingListId.value = null;
  if (title && title !== list.title) await store.updateList(list.id, { title });
}

function deleteList(list: TodoList) {
  pendingConfirm.value = { kind: "list", id: list.id, title: list.title };
}

function deleteItem(item: TodoItem) {
  pendingConfirm.value = { kind: "item", id: item.id, title: item.title };
}

function pendingForList(list: TodoList) {
  if (!pendingConfirm.value) return false;
  if (pendingConfirm.value.kind === "list") return pendingConfirm.value.id === list.id;
  return list.items.some((item) => item.id === pendingConfirm.value!.id);
}

async function confirmPending() {
  const pending = pendingConfirm.value;
  if (!pending) return;
  pendingConfirm.value = null;
  if (pending.kind === "list") {
    await store.updateList(pending.id, { deleted: true });
    return;
  }
  await store.softDelete(pending.id);
  closeItemDetails(pending.id);
}

async function toggleItemCompleted(item: TodoItem) {
  if (!item.completed) {
    completingIds.value = new Set(completingIds.value).add(item.id);
    await new Promise((resolve) => window.setTimeout(resolve, 160));
  }
  try {
    await store.toggleItem(item.id, item.completed);
    closeItemDetails(item.id);
  } finally {
    const next = new Set(completingIds.value);
    next.delete(item.id);
    completingIds.value = next;
  }
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

// If another window removes or changes the completion state of the expanded item,
// close stale details. Already-completed items can still be expanded normally.
watch(
  () => {
    if (!expandedItemId.value) return null;
    for (const list of store.lists) {
      const item = list.items.find((candidate) => candidate.id === expandedItemId.value);
      if (item) return { id: item.id, listId: list.id, completed: item.completed };
    }
    return null;
  },
  (item) => {
    if (!expandedItemId.value) return;
    if (
      !item ||
      item.listId !== expandedItemListId.value ||
      item.completed !== expandedItemCompleted.value
    ) {
      closeItemDetails();
    }
  }
);

onMounted(() => {
  store.init();
});

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
  closeItemDetails(itemId);
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
  if (targetIndex >= 0) {
    void store.moveItem(dragged.id, targetListId, targetIndex);
  }
}

function onItemDragEnd() {
  dragItem.value = null;
  dropItemId.value = null;
}
</script>

<template>
  <section class="todo-view" aria-labelledby="todo-heading">
    <PageHeader heading-id="todo-heading" title="我的待办" subtitle="把今天要做的事收进清单。">
      <template #actions>
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
            @click="triageActive = true"
          >
            处理模式
          </button>
        </div>
      </template>
    </PageHeader>

    <p v-if="store.error" class="error" role="alert">{{ store.error }}</p>
    <p v-if="actionError" class="error" role="alert">{{ actionError }}</p>

    <TriageMode v-if="triageActive" @exit="triageActive = false" />

    <template v-else>
      <div v-if="hasLists" class="todo-groups">
        <TodoListCard
          v-for="(list, listIndex) in store.lists"
          :key="list.id"
          :list="list"
          :list-index="listIndex"
          :total-lists="store.lists.length"
          :lists="store.lists"
          :pending="pendingItems(list)"
          :completed="completedItems(list)"
          :editing-list-title="editingListId === list.id"
          :editing-item-id="editingItemId"
          :expanded-item-id="expandedItemId"
          :dragging-list="dragListId === list.id"
          :drop-list-target="dropListId === list.id"
          :drop-item-id="dropItemId"
          :completing-ids="completingIds"
          @start-list-edit="startListEdit(list)"
          @commit-list-edit="(title) => commitListEdit(list, title)"
          @cancel-list-edit="editingListId = null"
          @delete-list="deleteList(list)"
          @move-list="(delta) => moveList(list.id, delta)"
          @add-item="(title) => store.createItem(list.id, title)"
          @toggle-item-completed="toggleItemCompleted"
          @toggle-item-details="(item) => toggleItemDetails(list.id, item)"
          @start-item-edit="editingItemId = $event"
          @commit-item-edit="commitItemEdit"
          @cancel-item-edit="editingItemId = null"
          @delete-item="deleteItem"
          @open-source="openSource"
          @move-to-list="moveToList"
          @move-item="(itemId, delta) => moveItem(list.id, itemId, delta)"
          @error="actionError = $event"
          @list-dragstart="(event) => onListDragStart(event, list.id)"
          @list-dragover="(event) => onListDragOver(event, list.id)"
          @list-drop="onListDrop(list.id)"
          @list-dragend="onListDragEnd"
          @item-dragstart="(event, itemId) => onItemDragStart(event, list.id, itemId)"
          @item-dragover="onItemDragOver"
          @item-drop="(event, itemId) => onItemDrop(event, list.id, itemId)"
          @item-dragend="onItemDragEnd"
        >
          <template #confirm>
            <ConfirmBar
              v-if="pendingConfirm && pendingForList(list)"
              :message="
                pendingConfirm.kind === 'list'
                  ? `将清单“${pendingConfirm.title}”移入回收站？`
                  : `将待办“${pendingConfirm.title}”移入回收站？`
              "
              confirm-label="移入回收站"
              danger
              @confirm="confirmPending"
              @cancel="pendingConfirm = null"
            />
          </template>
        </TodoListCard>
      </div>

      <EmptyState v-else title="还没有清单" text="先新建一个，把第一件事记下来。" />
    </template>
  </section>
</template>
