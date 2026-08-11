<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { openNoteInModule } from "../navigation";
import { useFocusStore } from "../stores/focus";
import { useGithubStore } from "../stores/github";
import { useNoteStore } from "../stores/note";
import { useTodoStore } from "../stores/todo";
import type { FocusGithub, FocusNote, FocusSection, FocusTodo } from "../types/focus";

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

const todoSection = computed<FocusSection<FocusTodo> | null>(
  () => focus.overview?.todo ?? null
);
const noteSection = computed<FocusSection<FocusNote> | null>(
  () => focus.overview?.note ?? null
);
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
</script>

<template>
  <section class="focus-view" aria-labelledby="focus-heading">
    <header class="focus-topbar">
      <div>
        <h1 id="focus-heading">今日焦点</h1>
        <p>未完成待办（收件箱优先）· 置顶 / 最近更新便签 · GitHub 需要行动的条目</p>
      </div>
      <button class="btn" type="button" :disabled="focus.loading" @click="focus.refresh()">
        {{ focus.loading ? "刷新中…" : "刷新" }}
      </button>
    </header>

    <p v-if="focus.error" class="error" role="alert">
      聚合数据加载失败：{{ focus.error }}（保留上次内容）
    </p>
    <p v-if="actionError" class="error" role="alert">{{ actionError }}</p>

    <div v-if="!focus.overview" class="focus-loading" role="status">
      {{ focus.loading ? "正在聚合各模块数据…" : "暂无数据" }}
    </div>

    <template v-else>
      <p v-if="focus.loading" class="focus-refreshing" role="status">正在刷新…</p>

      <div v-if="allEmpty && !hasError" class="focus-all-empty">
        今天没有需要处理的事情：没有未完成待办、置顶 / 最近便签，也没有需要行动的 GitHub 条目。
      </div>

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

          <ul v-if="todoSection?.items.length" class="focus-list">
            <li v-for="item in todoSection.items" :key="item.id" class="focus-item">
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
              </div>
            </li>
          </ul>
          <p v-else-if="!todoSection?.error" class="focus-empty">暂无未完成待办</p>
          <p v-if="todoSection && todoSection.total > todoSection.items.length" class="focus-more">
            还有 {{ todoSection.total - todoSection.items.length }} 项，进入待办查看
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
                  {{ item.pinned ? "📌 " : "" }}{{ item.title }}
                </span>
                <small v-if="item.preview" class="focus-item-preview">{{ item.preview }}</small>
                <small class="focus-item-meta">{{ formatTime(item.updatedAt) }}</small>
              </button>
              <button class="btn ghost compact" type="button" title="悬浮" @click="floatNote(item)">
                悬浮
              </button>
            </li>
          </ul>
          <p v-else-if="!noteSection?.error" class="focus-empty">
            暂无置顶或最近更新的便签
          </p>
        </section>

        <section class="focus-section glass-card" aria-labelledby="focus-github-title">
          <header class="focus-section-header">
            <div class="focus-section-heading">
              <h2 id="focus-github-title">GitHub</h2>
              <span v-if="githubSection && !githubSection.error">
                {{ githubSection.total }} 条需要行动
              </span>
            </div>
            <button class="btn ghost compact" type="button" @click="go('github')">进入 GitHub</button>
          </header>

          <p v-if="githubSection?.error" class="error" role="alert">
            部分 GitHub 数据加载失败：{{ githubSection.error }}
          </p>
          <p v-if="githubSection?.offlineCache" class="focus-cache-hint" role="status">
            {{ githubSection.items.length ? "离线 / 未登录：展示本地缓存" : "离线 / 未登录：暂无缓存" }}
          </p>

          <ul v-if="githubSection?.items.length" class="focus-list">
            <li v-for="item in githubSection.items" :key="`${item.repo}-${item.kind}-${item.number}`" class="focus-item">
              <button class="focus-item-main" type="button" @click="openGithub(item)">
                <span class="focus-item-title" :title="item.title">
                  <span class="focus-badge" :class="item.kind">{{ githubBadge(item) }}</span>
                  {{ item.pinned ? "📌 " : "" }}#{{ item.number }} {{ item.title }}
                </span>
                <small class="focus-item-meta">{{ item.repo }} · {{ formatTime(item.updatedAt) }}</small>
                <small v-if="githubSource(item)" class="focus-item-source">{{ githubSource(item) }}</small>
              </button>
              <button
                class="btn ghost compact"
                type="button"
                :title="`在浏览器打开 #${item.number}`"
                @click="openGithub(item)"
              >
                打开
              </button>
            </li>
          </ul>
          <p v-else-if="!githubSection?.error" class="focus-empty">
            暂无需要行动的 GitHub 条目
          </p>
          <p v-if="githubSection && githubSection.total > githubSection.items.length" class="focus-more">
            还有 {{ githubSection.total - githubSection.items.length }} 项，进入 GitHub 查看
          </p>
        </section>
      </div>
    </template>
  </section>
</template>
