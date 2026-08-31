// Presentation helpers shared by the todo views: date formatting, repeat
// labels, GitHub-source labels and schedule summaries. Pure functions only.

import type { TodoScheduleInput } from "./api/todo";
import type { RepeatRule, TodoItem, TodoSource } from "./types/todo";

export function isHttpUrl(url: string) {
  return /^https?:\/\//i.test(url);
}

function pad2(value: number) {
  return String(value).padStart(2, "0");
}

export function localDateValue(value: string | null | undefined) {
  if (!value) return "";
  const d = new Date(value.length === 10 ? `${value}T00:00:00` : value);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

export function localDateTimeValue(value: string | null | undefined) {
  if (!value) return "";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}T${pad2(
    d.getHours()
  )}:${pad2(d.getMinutes())}`;
}

export function shortDateValue(value: string | null | undefined) {
  const local = localDateValue(value);
  if (!local) return "";
  const [, month, day] = local.split("-");
  return `${Number(month)}/${Number(day)}`;
}

export function shortDateTimeValue(value: string | null | undefined) {
  const local = localDateTimeValue(value);
  if (!local) return "";
  const [date, time] = local.split("T");
  return `${shortDateValue(date)} ${time}`;
}

export function repeatLabel(rule: RepeatRule) {
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

export function scheduleSummaries(item: TodoItem) {
  const summaries: string[] = [];
  if (item.dueDate) summaries.push(`到期 ${shortDateValue(item.dueDate)}`);
  if (item.remindAt) summaries.push(`提醒 ${shortDateTimeValue(item.remindAt)}`);
  if (item.repeat) summaries.push(`重复 ${repeatLabel(item.repeat)}`);
  return summaries;
}

export function scheduleOf(item: TodoItem): TodoScheduleInput {
  return {
    dueDate: item.dueDate ?? undefined,
    remindAt: item.remindAt ?? undefined,
    repeat: item.repeat ?? undefined,
    repeatUntil: item.repeatUntil ?? undefined,
  };
}

export function sourceLabel(source: TodoSource) {
  const kind =
    source.type === "github-pr" ? "PR" : source.type === "github-issue" ? "Issue" : source.type;
  return `${kind} ${source.repo}#${source.number}`;
}

export function githubSyncLabel(item: TodoItem) {
  const sync = item.githubSync;
  if (!sync) return "";
  if (sync.syncError) return "同步失败";
  if (sync.state === "merged") return sync.autoCompletionUndoneAt ? "已撤销自动完成" : "已合并";
  if (sync.state === "closed") return sync.autoCompletionUndoneAt ? "已撤销自动完成" : "已关闭";
  if (sync.state === "unknown") return "状态未知";
  if (item.completed && sync.autoCompletedAt) return "来源已重新打开";
  return "";
}

export function githubSyncClass(item: TodoItem) {
  const sync = item.githubSync;
  if (!sync) return "";
  if (sync.syncError || sync.state === "unknown") return "error";
  if (sync.state === "merged") return sync.autoCompletionUndoneAt ? "reopened" : "merged";
  if (sync.state === "closed") return sync.autoCompletionUndoneAt ? "reopened" : "closed";
  if (item.completed && sync.autoCompletedAt) return "reopened";
  return "";
}

export function githubSyncTitle(item: TodoItem) {
  const sync = item.githubSync;
  if (!sync) return "";
  if (sync.syncError) return `GitHub 来源同步失败：${sync.syncError}`;
  if (sync.state === "merged") {
    if (sync.autoCompletionUndoneAt) return "已撤销自动完成，来源仍处于已合并状态";
    return sync.autoCompletedAt ? "来源已合并，待办已自动完成" : "来源已合并";
  }
  if (sync.state === "closed") {
    if (sync.autoCompletionUndoneAt) return "已撤销自动完成，来源仍处于已关闭状态";
    return sync.autoCompletedAt ? "来源已关闭，待办已自动完成" : "来源已关闭";
  }
  if (item.completed && sync.autoCompletedAt) return "来源已重新打开，待办保持已完成";
  return "GitHub 来源已同步";
}
