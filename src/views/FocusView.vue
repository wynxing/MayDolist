<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import EmptyState from "../components/EmptyState.vue";
import PageHeader from "../components/PageHeader.vue";
import PinMark from "../components/PinMark.vue";
import { openNoteInModule } from "../navigation";
import { useFocusStore } from "../stores/focus";
import { useGithubStore } from "../stores/github";
import { useNoteStore } from "../stores/note";
import { useTodoStore } from "../stores/todo";
import { signalBadges } from "../signals";
import type {
  FocusGithub,
  FocusNote,
  FocusSection,
  FocusTodo,
  FocusTodoSection,
} from "../types/focus";
import type { RepeatRule, TodoSource } from "../types/todo";

const emit = defineEmits<{ navigate: [tab: string] }>();

const focus = useFocusStore();
const todo = useTodoStore();
const note = useNoteStore();
const github = useGithubStore();

const actionError = ref("");

const filterLabels = [
  ["mine", "我的"],
  ["mentioned", "被提及"],
  ["assigned", "被分配"],
  ["involved", "参与"],
  ["all-prs", "全部 PR"],
] as const;

onMounted(() => void focus.init());

const todoSection = computed<FocusTodoSection | null>(() => focus.overview?.todo ?? null);
const shownTodoCount = computed(
  () => todoSection.value?.groups.reduce((sum, group) => sum + group.items.length, 0) ?? 0
);
const noteSection = computed<FocusSection<FocusNote> | null>(() => focus.overview?.note ?? null);
const githubSection = computed<FocusSection<FocusGithub> | null>(
  () => focus.overview?.github ?? null
);

const hasError = computed(
  () =>
    !!todoSection.value?.error ||
    !!noteSection.value?.error ||
    !!githubSection.value?.error ||
    !!focus.error
);

const allEmpty = computed(() => {
  const todoEmpty = todoSection.value && todoSection.value.total === 0 && !todoSection.value.error;
  const noteEmpty = noteSection.value && noteSection.value.total === 0 && !noteSection.value.error;
  const githubEmpty =
    githubSection.value && githubSection.value.total === 0 && !githubSection.value.error;
  return !!todoEmpty && !!noteEmpty && !!githubEmpty;
});

// A notification click asks to highlight one Todo: scroll it into view and
// flash it briefly, then clear the request so a later refresh does not
// re-highlight it.
watch(
  () => focus.focusTodoId,
  async (id) => {
    if (!id) return;
    await nextTick();
    await nextTick();
    const element = document.querySelector<HTMLElement>(`[data-todo-id="${id}"]`);
    element?.scrollIntoView({ behavior: "smooth", block: "center" });
    element?.classList.add("focus-flash");
    window.setTimeout(() => element?.classList.remove("focus-flash"), 4000);
    focus.focusTodoId = null;
  }
);

function go(tab: string) {
  emit("navigate", tab);
}

async function runAction(action: () => Promise<unknown>) {
  actionError.value = "";
  try {
    await action();
  } catch (e) {
    actionError.value = String(e);
  }
}

function completeTodo(item: FocusTodo) {
  void runAction(async () => {
    await todo.toggleItem(item.id, false);
    void focus.refresh();
  });
}

function todoSourceLabel(source: TodoSource) {
  const kind =
    source.type === "github-pr" ? "PR" : source.type === "github-issue" ? "Issue" : source.type;
  return `${kind} ${source.repo}#${source.number}`;
}

function todoGithubSyncLabel(item: FocusTodo) {
  const sync = item.githubSync;
  if (!sync) return "";
  if (sync.syncError) return "同步失败";
  if (sync.state === "merged") return "已合并";
  if (sync.state === "closed") return "已关闭";
  if (sync.state === "unknown") return "状态未知";
  return "";
}

function isHttpUrl(url: string) {
  return /^https?:\/\//i.test(url);
}

function openTodoSource(item: FocusTodo) {
  if (!item.source || !isHttpUrl(item.source.url)) return;
  void runAction(() => github.open(item.source!.url));
}

function openNote(item: FocusNote) {
  openNoteInModule(item.id);
  go("note");
}

function floatNote(item: FocusNote) {
  void runAction(() => note.float(item.id));
}

function openGithub(item: FocusGithub) {
  void runAction(() => github.open(item.url));
}

function githubBadge(item: FocusGithub) {
  return item.kind === "pr" ? "PR" : "Issue";
}

function githubSource(item: FocusGithub) {
  return item.matches
    .map((match) => {
      if (match === "pinned") return "手动";
      const found = filterLabels.find((label) => label[0] === match);
      return found ? found[1] : match;
    })
    .join(" · ");
}

function githubSignals(item: FocusGithub) {
  return signalBadges(item.signals);
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

function formatDue(iso: string | null | undefined) {
  if (!iso) return "";
  const date = new Date(iso.length === 10 ? `${iso}T00:00:00` : iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
}

function repeatLabel(repeat: RepeatRule | null | undefined) {
  if (!repeat) return "";
  const labels: Record<RepeatRule, string> = {
    daily: "每天重复",
    weekly: "每周重复",
    biweekly: "每两周重复",
    monthly: "每月重复",
  };
  return labels[repeat];
}
</script>

<template>
  <section class="focus-view" aria-labelledby="focus-heading">
    <PageHeader
      heading-id="focus-heading"
      title="今日焦点"
      subtitle="未完成待办（收件箱优先）· 置顶 / 最近更新便签 · GitHub 需要行动的条目"
    >
      <template #actions>
        <button class="btn" type="button" :disabled="focus.loading" @click="focus.refresh()">
          {{ focus.loading ? "刷新中…" : "刷新" }}
        </button>
      </template>
    </PageHeader>

    <p v-if="focus.error" class="error" role="alert">
      聚合数据加载失败：{{ focus.error }}（保留上次内容）
    </p>
    <p v-if="actionError" class="error" role="alert">{{ actionError }}</p>

    <div v-if="!focus.overview" class="focus-loading" role="status">
      {{ focus.loading ? "正在聚合各模块数据…" : "暂无数据" }}
    </div>

    <template v-else>
      <p v-if="focus.loading" class="focus-refreshing" role="status">正在刷新…</p>

      <EmptyState
        v-if="allEmpty && !hasError"
        title="今天很安静"
        text="没有未完成待办、置顶便签，也没有需要行动的 GitHub 条目。用快速收集记下下一件事。"
      />

      <div class="focus-sections">
        <section class="focus-section glass-card" aria-labelledby="focus-todo-title">
          <header class="focus-section-header">
            <div class="focus-section-heading">
              <h2 id="focus-todo-title">待办</h2>
              <span v-if="todoSection && !todoSection.error">{{ todoSection.total }} 项未完成</span>
            </div>
            <button class="btn ghost compact" type="button" @click="go('todo')">进入待办</button>
          </header>

          <p v-if="todoSection?.error" class="error" role="alert">
            待办加载失败：{{ todoSection.error }}
          </p>

          <div v-if="todoSection?.groups.length" class="focus-todo-groups">
            <section
              v-for="group in todoSection.groups"
              :key="group.key"
              class="focus-todo-group"
              :class="{ overdue: group.key === 'overdue' }"
            >
              <header class="focus-todo-group-header">
                <h3>{{ group.title }}</h3>
                <span>{{ group.count }} 项</span>
              </header>
              <ul class="focus-list">
                <li
                  v-for="item in group.items"
                  :key="item.id"
                  class="focus-item"
                  :class="{ 'focus-item-overdue': group.key === 'overdue' }"
                  :data-todo-id="item.id"
                >
                  <input
                    type="checkbox"
                    :aria-label="`完成待办：${item.title}`"
                    title="完成"
                    @change="completeTodo(item)"
                  />
                  <div class="focus-item-body">
                    <span class="focus-item-title" :title="item.title">{{ item.title }}</span>
                    <small class="focus-item-meta" :class="{ inbox: item.inbox }">
                      {{ item.inbox ? "收件箱" : item.listTitle }}
                    </small>
                    <small v-if="item.source" class="focus-item-source" :title="item.source.url">
                      {{ todoSourceLabel(item.source) }}
                    </small>
                    <small
                      v-if="item.source && todoGithubSyncLabel(item)"
                      class="focus-item-source focus-item-source-state"
                      :class="item.githubSync?.state"
                      :title="item.githubSync?.syncError || 'GitHub 来源状态已同步'"
                    >
                      {{ todoGithubSyncLabel(item) }}
                    </small>
                    <small v-if="item.dueDate" class="focus-item-due">
                      截止 {{ formatDue(item.dueDate) }}
                    </small>
                    <small v-if="item.repeat" class="focus-item-repeat">
                      {{ repeatLabel(item.repeat) }}
                    </small>
                  </div>
                  <button
                    v-if="item.source && isHttpUrl(item.source.url)"
                    class="btn ghost compact row-actions"
                    type="button"
                    :title="`在浏览器打开来源 ${item.source.url}`"
                    @click="openTodoSource(item)"
                  >
                    打开来源
                  </button>
                </li>
              </ul>
            </section>
          </div>
          <p v-else-if="!todoSection?.error" class="focus-empty">暂无未完成待办</p>
          <p v-if="todoSection && todoSection.total > shownTodoCount" class="focus-more">
            还有 {{ todoSection.total - shownTodoCount }} 项，进入待办查看
          </p>
        </section>

        <section class="focus-section glass-card" aria-labelledby="focus-note-title">
          <header class="focus-section-header">
            <div class="focus-section-heading">
              <h2 id="focus-note-title">便签</h2>
              <span v-if="noteSection && !noteSection.error">置顶 / 最近更新</span>
            </div>
            <button class="btn ghost compact" type="button" @click="go('note')">进入便签</button>
          </header>

          <p v-if="noteSection?.error" class="error" role="alert">
            便签加载失败：{{ noteSection.error }}
          </p>

          <ul v-if="noteSection?.items.length" class="focus-list">
            <li v-for="item in noteSection.items" :key="item.id" class="focus-item">
              <button class="focus-item-main" type="button" @click="openNote(item)">
                <span class="focus-item-title" :title="item.title">
                  <PinMark :on="item.pinned" />{{ item.title }}
                </span>
                <small v-if="item.preview" class="focus-item-preview">{{ item.preview }}</small>
                <small class="focus-item-meta">{{ formatTime(item.updatedAt) }}</small>
              </button>
              <button
                class="btn ghost compact row-actions"
                type="button"
                title="悬浮"
                @click="floatNote(item)"
              >
                悬浮
              </button>
            </li>
          </ul>
          <p v-else-if="!noteSection?.error" class="focus-empty">暂无置顶或最近更新的便签</p>
        </section>

        <section class="focus-section glass-card" aria-labelledby="focus-github-title">
          <header class="focus-section-header">
            <div class="focus-section-heading">
              <h2 id="focus-github-title">GitHub</h2>
              <span v-if="githubSection && !githubSection.error">
                {{ githubSection.total }} 条需要行动
              </span>
            </div>
            <button class="btn ghost compact" type="button" @click="go('github')">
              进入 GitHub
            </button>
          </header>

          <p v-if="githubSection?.error" class="error" role="alert">
            部分 GitHub 数据加载失败：{{ githubSection.error }}
          </p>
          <p v-if="githubSection?.offlineCache" class="focus-cache-hint" role="status">
            {{
              githubSection.items.length ? "离线 / 未登录：展示本地缓存" : "离线 / 未登录：暂无缓存"
            }}
          </p>

          <ul v-if="githubSection?.items.length" class="focus-list">
            <li
              v-for="item in githubSection.items"
              :key="`${item.repo}-${item.kind}-${item.number}`"
              class="focus-item"
            >
              <button class="focus-item-main" type="button" @click="openGithub(item)">
                <span class="focus-item-title" :title="item.title">
                  <span class="focus-badge" :class="item.kind">{{ githubBadge(item) }}</span>
                  <span
                    v-for="b in githubSignals(item)"
                    :key="b.key"
                    class="gh-signal-badge focus-signal-badge"
                    :class="b.key"
                  >
                    {{ b.label }}
                  </span>
                  <PinMark v-if="item.pinned" on />#{{ item.number }} {{ item.title }}
                </span>
                <small class="focus-item-meta">
                  {{ item.repo }} · 更新 {{ formatTime(item.updatedAt) }}
                </small>
                <small v-if="githubSource(item)" class="focus-item-source">{{
                  githubSource(item)
                }}</small>
              </button>
              <button
                class="btn ghost compact row-actions"
                type="button"
                :title="`在浏览器打开 #${item.number}`"
                @click="openGithub(item)"
              >
                打开
              </button>
            </li>
          </ul>
          <p v-else-if="!githubSection?.error" class="focus-empty">暂无需要行动的 GitHub 条目</p>
          <p
            v-if="githubSection && githubSection.total > githubSection.items.length"
            class="focus-more"
          >
            还有 {{ githubSection.total - githubSection.items.length }} 项，进入 GitHub 查看
          </p>
        </section>
      </div>
    </template>
  </section>
</template>
