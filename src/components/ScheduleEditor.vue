<script setup lang="ts">
// Due-date / reminder / repeat editor for a single todo item. Write errors
// are reported to the parent so it can show them in the shared error slot.
import { useTodoStore } from "../stores/todo";
import type { RepeatRule, TodoItem } from "../types/todo";
import type { TodoScheduleInput } from "../api/todo";
import { localDateValue, localDateTimeValue, scheduleOf } from "../todoFormat";

const props = defineProps<{ item: TodoItem }>();
const emit = defineEmits<{ error: [message: string] }>();

const store = useTodoStore();

async function patchSchedule(schedule: TodoScheduleInput) {
  emit("error", "");
  try {
    await store.patchItem(props.item.id, { schedule });
  } catch (err) {
    emit("error", String(err));
  }
}

async function setDueDate(value: string) {
  const schedule = scheduleOf(props.item);
  schedule.dueDate = value || undefined;
  if (!schedule.dueDate) schedule.remindAt = undefined;
  await patchSchedule(schedule);
}

async function setRemindAt(value: string) {
  const schedule = scheduleOf(props.item);
  schedule.remindAt = value ? new Date(value).toISOString() : undefined;
  await patchSchedule(schedule);
}

async function setRepeat(value: string) {
  const schedule = scheduleOf(props.item);
  schedule.repeat = (value || undefined) as RepeatRule | undefined;
  if (!schedule.repeat) schedule.repeatUntil = undefined;
  await patchSchedule(schedule);
}
</script>

<template>
  <div class="todo-item-schedule">
    <label>
      <span>到期日</span>
      <input
        type="date"
        :value="localDateValue(item.dueDate)"
        @change="setDueDate(($event.target as HTMLInputElement).value)"
      />
    </label>
    <label>
      <span>提醒时间</span>
      <input
        type="datetime-local"
        :value="localDateTimeValue(item.remindAt)"
        title="需先设置到期日"
        :disabled="!item.dueDate"
        @change="setRemindAt(($event.target as HTMLInputElement).value)"
      />
    </label>
    <label>
      <span>重复规则</span>
      <select
        :value="item.repeat ?? ''"
        @change="setRepeat(($event.target as HTMLSelectElement).value)"
      >
        <option value="">不重复</option>
        <option value="daily">每天</option>
        <option value="weekly">每周</option>
        <option value="biweekly">每两周</option>
        <option value="monthly">每月</option>
      </select>
    </label>
  </div>
</template>
