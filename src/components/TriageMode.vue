<script setup lang="ts">
/* ------------------------------------------------------------------ *
 * Inbox triage mode (#28): a pure view mode that shows one pending
 * inbox item at a time. All actions reuse existing commands / services
 * (todo_update_item / todo_move_item / todo_soft_delete), so no new
 * write path and no new persisted field exists.
 * ------------------------------------------------------------------ */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useTodoStore } from "../stores/todo";
import { useSettingsStore } from "../stores/settings";
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
import { localDateValue, repeatLabel, scheduleOf, sourceLabel } from "../todoFormat";

const emit = defineEmits<{ exit: [] }>();

const store = useTodoStore();
const settings = useSettingsStore();

const inboxList = computed(() => {
  const byKind = store.lists.find((list) => list.kind === "inbox");
  if (byKind) return byKind;
  return store.lists.find((list) => list.title === "收件箱") ?? null;
});
const inboxPending = computed(() => triagePending(inboxList.value?.items ?? []));

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

function exitTriage() {
  movePickerOpen.value = false;
  triageError.value = "";
  emit("exit");
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
      const laterDays = settings.config?.triageLaterDays ?? 3;
      schedule.dueDate = triageDueDate(new Date(), action === "today" ? 0 : laterDays);
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
  const state = reconcileTriage(
    triageRemainingIds.value,
    triageCurrentId.value,
    inboxPending.value
  );
  triageRemainingIds.value = state.remainingIds;
  triageCurrentId.value = state.currentId;
});

onMounted(() => {
  const state = enterTriage(inboxPending.value);
  triageTotalIds.value = [...state.remainingIds];
  triageRemainingIds.value = state.remainingIds;
  triageCurrentId.value = state.currentId;
  window.addEventListener("keydown", onTriageKeydown);
  void nextTick(() => triageCardEl.value?.focus());
});
onBeforeUnmount(() => window.removeEventListener("keydown", onTriageKeydown));
</script>

<template>
  <section class="triage" aria-label="收件箱处理模式">
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
      <Transition name="triage-slide" mode="out-in">
        <article
          :key="triageCurrent.id"
          ref="triageCardEl"
          class="triage-card glass-card"
          tabindex="0"
        >
          <p class="triage-card-title">{{ triageCurrent.title }}</p>
          <div class="triage-card-meta">
            <span v-if="triageCurrent.source" class="todo-source" :title="triageCurrent.source.url">
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
      </Transition>

      <div class="triage-actions" role="group" aria-label="处理动作">
        <button class="btn" type="button" :disabled="triageBusy" @click="runTriageAction('today')">
          <kbd>1</kbd> 今天做
        </button>
        <button class="btn" type="button" :disabled="triageBusy" @click="runTriageAction('later')">
          <kbd>2</kbd> 稍后做
        </button>
        <button class="btn" type="button" :disabled="triageBusy" @click="runTriageAction('move')">
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
        按 <kbd>1</kbd>–<kbd>5</kbd> 执行动作，按
        <kbd>Esc</kbd> 退出；完成可取消、删除可在回收站恢复。
      </p>
    </template>
  </section>
</template>
