<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { call } from "../api";
import * as backupApi from "../api/backup";
import * as githubApi from "../api/github";
import * as noteApi from "../api/note";
import * as paletteApi from "../api/palette";
import * as quickApi from "../api/quick";
import PinMark from "../components/PinMark.vue";
import { useNoteStore } from "../stores/note";
import { useTodoStore } from "../stores/todo";
import type {
  PaletteCommand,
  PaletteGithub,
  PaletteNote,
  PaletteSearchResult,
  PaletteTodo,
} from "../types/palette";

type Mode = "search" | "new-todo" | "new-note";

type Row =
  | { kind: "command"; command: PaletteCommand }
  | { kind: "todo"; todo: PaletteTodo }
  | { kind: "note"; note: PaletteNote }
  | { kind: "github"; github: PaletteGithub };

const mode = ref<Mode>("search");
const query = ref("");
const result = ref<PaletteSearchResult | null>(null);
const selected = ref(0);
const busy = ref(false);
const error = ref("");
const feedback = ref("");
const input = ref<HTMLInputElement | null>(null);

let unlisten: UnlistenFn | null = null;
let searchTimer: number | undefined;
let closeTimer: number | undefined;

const rows = computed<Row[]>(() => {
  const r = result.value;
  if (!r) return [];
  const out: Row[] = [];
  for (const command of r.commands) out.push({ kind: "command", command });
  for (const todo of r.todos) out.push({ kind: "todo", todo });
  for (const note of r.notes) out.push({ kind: "note", note });
  for (const github of r.github) out.push({ kind: "github", github });
  return out;
});

const todoStart = computed(() => result.value?.commands.length ?? 0);
const noteStart = computed(
  () => todoStart.value + (result.value?.todos.length ?? 0)
);
const githubStart = computed(
  () => noteStart.value + (result.value?.notes.length ?? 0)
);

function placeholder() {
  if (mode.value === "new-todo") return "输入待办标题，Enter 保存到收件箱";
  if (mode.value === "new-note") return "输入便签标题，Enter 创建";
  return "输入命令，或搜索 Todo / 便签 / GitHub…";
}

function selectRow(index: number) {
  selected.value = index;
}

function focusInput() {
  error.value = "";
  void nextTick(() => {
    input.value?.focus();
    input.value?.select();
  });
}

function openPalette() {
  resetMode();
  focusInput();
}

function resetMode() {
  mode.value = "search";
  query.value = "";
  feedback.value = "";
  error.value = "";
  clearTimeout(closeTimer);
  void runSearch("");
}

function startMode(next: Mode) {
  mode.value = next;
  query.value = "";
  feedback.value = "";
  error.value = "";
  selected.value = 0;
  result.value = null;
  clearTimeout(closeTimer);
  focusInput();
}

async function runSearch(text: string) {
  busy.value = true;
  error.value = "";
  try {
    result.value = await paletteApi.search(text);
    selected.value = 0;
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

watch(query, (value) => {
  if (mode.value !== "search") return;
  clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => void runSearch(value), 150);
});

watch(selected, async () => {
  await nextTick();
  document
    .querySelector<HTMLElement>(`.palette-row[data-index="${selected.value}"]`)
    ?.scrollIntoView({ block: "nearest" });
});

function onKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    if (rows.value.length) {
      selected.value = (selected.value + 1) % rows.value.length;
    }
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    if (rows.value.length) {
      selected.value =
        (selected.value - 1 + rows.value.length) % rows.value.length;
    }
  } else if (event.key === "Enter") {
    // IME composition must never trigger execution (中文输入法组合输入).
    if (event.isComposing) return;
    if (mode.value !== "search") {
      void submitMode();
      return;
    }
    const row = rows.value[selected.value];
    if (row) void execute(row);
  } else if (event.key === "Escape") {
    if (mode.value !== "search") {
      resetMode();
    } else {
      void paletteApi.hide();
    }
  }
}

async function execute(row: Row) {
  if (busy.value) return;
  switch (row.kind) {
    case "command":
      await runCommand(row.command.id);
      break;
    case "todo":
      await goTab("todo");
      break;
    case "note":
      await openNote(row.note.id);
      break;
    case "github":
      await runAction(() => githubApi.open(row.github.url), "已打开 GitHub 条目");
      break;
  }
}

async function runCommand(id: string) {
  switch (id) {
    case "go-focus":
      await goTab("focus");
      break;
    case "go-todo":
      await goTab("todo");
      break;
    case "go-note":
      await goTab("note");
      break;
    case "go-github":
      await goTab("github");
      break;
    case "go-settings":
      await goTab("settings");
      break;
    case "new-todo":
      startMode("new-todo");
      break;
    case "new-note":
      startMode("new-note");
      break;
    case "backup-now":
      await runAction(() => backupApi.createBackup(), "备份已创建");
      break;
    case "open-data-dir":
      await runAction(() => backupApi.openDataDir(), "已打开数据目录");
      break;
  }
}

async function runAction(action: () => Promise<unknown>, okMessage: string) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await action();
    feedback.value = okMessage;
    clearTimeout(closeTimer);
    closeTimer = window.setTimeout(() => void paletteApi.hide(), 700);
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function goTab(tab: string, extra?: { noteId?: string }) {
  if (busy.value) return;
  busy.value = true;
  try {
    await call("app_show_main");
    await emit("command-palette-navigate", { tab, ...extra });
    await paletteApi.hide();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function openNote(id: string) {
  await goTab("note", { noteId: id });
}

async function submitMode() {
  const text = query.value.trim();
  if (!text || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    if (mode.value === "new-todo") {
      // Reuse the existing capture command: plain text lands in the inbox.
      await quickApi.submit(text);
      feedback.value = "已保存到收件箱";
      clearTimeout(closeTimer);
      closeTimer = window.setTimeout(() => void paletteApi.hide(), 700);
    } else {
      const note = await noteApi.create(text);
      await goTab("note", { noteId: note.id });
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function completeTodo(todo: PaletteTodo) {
  await runAction(async () => {
    await useTodoStore().toggleItem(todo.id, false);
    void runSearch(query.value);
  }, "已完成");
}

async function pinNote(note: PaletteNote) {
  await runAction(async () => {
    await useNoteStore().update(note.id, { pinned: true });
    void runSearch(query.value);
  }, "已置顶");
}

function githubLabel(github: PaletteGithub) {
  return github.kind === "pr" ? "PR" : "Issue";
}

function formatTime(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

onMounted(async () => {
  unlisten = await listen("command-palette-open", openPalette);
  openPalette();
});

onBeforeUnmount(() => {
  unlisten?.();
  unlisten = null;
  clearTimeout(searchTimer);
  clearTimeout(closeTimer);
});
</script>

<template>
  <div class="palette" @keydown="onKeydown">
    <header class="palette-header">
      <span class="palette-prefix" aria-hidden="true">⌘</span>
      <input
        ref="input"
        v-model="query"
        class="palette-input"
        :placeholder="placeholder()"
        autocomplete="off"
        spellcheck="false"
        :disabled="busy"
      />
      <button
        class="palette-close"
        type="button"
        aria-label="关闭命令面板"
        title="关闭（Esc）"
        @click="paletteApi.hide()"
      >
        ×
      </button>
    </header>

    <div class="palette-body">
      <p v-if="error" class="palette-error" role="alert">{{ error }}</p>
      <p v-if="feedback" class="palette-feedback" role="status">{{ feedback }}</p>

      <template v-if="mode === 'search'">
        <section v-if="result?.commands.length" class="palette-section">
          <h3 class="palette-heading">命令</h3>
          <ul class="palette-list">
            <li
              v-for="(command, i) in result.commands"
              :key="command.id"
              class="palette-row"
              :class="{ selected: selected === i }"
              :data-index="i"
              @mouseenter="selectRow(i)"
              @click="execute({ kind: 'command', command })"
            >
              <span class="palette-row-title">{{ command.label }}</span>
              <span class="palette-row-hint">{{ command.hint }}</span>
            </li>
          </ul>
        </section>

        <section v-if="result?.todos.length" class="palette-section">
          <h3 class="palette-heading">待办</h3>
          <ul class="palette-list">
            <li
              v-for="(todo, i) in result.todos"
              :key="todo.id"
              class="palette-row"
              :class="{ selected: selected === todoStart + i }"
              :data-index="todoStart + i"
              @mouseenter="selectRow(todoStart + i)"
              @click="goTab('todo')"
            >
              <span class="palette-row-title" :title="todo.title">
                {{ todo.title }}
              </span>
              <span class="palette-row-meta">
                {{ todo.inbox ? "收件箱" : todo.listTitle }}
                <span v-if="todo.source" class="palette-badge">
                  {{ todo.source.type === "github-pr" ? "PR" : "Issue" }}
                  {{ todo.source.repo }}#{{ todo.source.number }}
                </span>
              </span>
              <span class="palette-row-actions">
                <button
                  class="palette-action"
                  type="button"
                  title="完成 Todo"
                  @click.stop="completeTodo(todo)"
                >
                  完成
                </button>
                <button
                  v-if="todo.source"
                  class="palette-action"
                  type="button"
                  title="打开来源"
                  @click.stop="githubApi.open(todo.source!.url)"
                >
                  打开来源
                </button>
              </span>
            </li>
          </ul>
        </section>

        <section v-if="result?.notes.length" class="palette-section">
          <h3 class="palette-heading">便签</h3>
          <ul class="palette-list">
            <li
              v-for="(note, i) in result.notes"
              :key="note.id"
              class="palette-row"
              :class="{ selected: selected === noteStart + i }"
              :data-index="noteStart + i"
              @mouseenter="selectRow(noteStart + i)"
              @click="openNote(note.id)"
            >
              <span class="palette-row-title" :title="note.title">
                <PinMark :on="note.pinned" />{{ note.title }}
              </span>
              <span class="palette-row-meta">
                {{ note.preview || "（空内容）" }}
              </span>
              <span class="palette-row-actions">
                <button
                  class="palette-action"
                  type="button"
                  title="置顶便签"
                  @click.stop="pinNote(note)"
                >
                  置顶
                </button>
              </span>
            </li>
          </ul>
        </section>

        <section v-if="result?.github.length" class="palette-section">
          <h3 class="palette-heading">
            GitHub
            <span v-if="result.githubOffline" class="palette-offline">离线缓存</span>
          </h3>
          <ul class="palette-list">
            <li
              v-for="(github, i) in result.github"
              :key="`${github.repo}-${github.kind}-${github.number}`"
              class="palette-row"
              :class="{ selected: selected === githubStart + i }"
              :data-index="githubStart + i"
              @mouseenter="selectRow(githubStart + i)"
              @click="githubApi.open(github.url)"
            >
              <span class="palette-row-title" :title="github.title">
                <span class="palette-badge" :class="github.kind">
                  {{ githubLabel(github) }}
                </span>
                {{ github.repo }}#{{ github.number }} {{ github.title }}
              </span>
              <span class="palette-row-meta">
                更新 {{ formatTime(github.updatedAt) }}
              </span>
            </li>
          </ul>
        </section>

        <p
          v-if="mode === 'search' && query && rows.length === 0 && !busy"
          class="palette-empty"
        >
          没有匹配结果：试试命令名、Todo 标题、便签内容或 GitHub 仓库 / #编号。
        </p>
      </template>

      <p v-else class="palette-empty">
        {{
          mode === "new-todo"
            ? "输入标题后按 Enter 保存到收件箱；Esc 返回命令列表"
            : "输入标题后按 Enter 创建便签；Esc 返回命令列表"
        }}
      </p>
    </div>

    <footer class="palette-footer">
      <span>
        {{
          mode === "search"
            ? query
              ? "输入即搜索（防抖 150ms）"
              : "输入命令名或关键词"
            : "Enter 确认 · Esc 返回"
        }}
      </span>
      <span class="palette-keys">↑ ↓ 选择 · Enter 执行 · Esc 关闭</span>
    </footer>
  </div>
</template>
