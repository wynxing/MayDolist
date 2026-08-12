<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useGithubStore } from "../stores/github";
import { useTodoStore } from "../stores/todo";
import { SIGNAL_FILTER_OPTIONS, signalBadges } from "../signals";
import type {
  ActionSignal,
  GhIssue,
  GhPullRequest,
  RepoSnapshot,
  RepoWatch,
} from "../types/github";

const s = useGithubStore();
const todo = useTodoStore();
const repo = ref("");
const busy = ref(false);
const repoBusy = reactive<Record<string, boolean>>({});
const pinDrafts = reactive<Record<string, string>>({});
const pinBusy = reactive<Record<string, boolean>>({});
const pinErrors = reactive<Record<string, string | null>>({});
const convertState = reactive<
  Record<string, { phase: "idle" | "saving" | "ok" | "error"; message: string }>
>({});

const filters = [
  ["mine", "我的"],
  ["mentioned", "被提及"],
  ["assigned", "被分配"],
  ["involved", "参与"],
  ["all-prs", "全部 PR"],
] as const;

const signalFilters = SIGNAL_FILTER_OPTIONS;

onMounted(() => s.init());

const snapshotMap = computed(() => {
  const map = new Map<string, RepoSnapshot>();
  for (const snap of s.snapshots) map.set(snap.repo, snap);
  return map;
});

function snapFor(watch: RepoWatch) {
  return snapshotMap.value.get(watch.fullName);
}

function visiblePrs(watch: RepoWatch) {
  const snap = snapFor(watch);
  if (!snap) return [];
  const ignored = new Set(
    (watch.ignored ?? [])
      .filter((v) => v.kind === "pr")
      .map((v) => v.number),
  );
  return snap.pullRequests.filter(
    (pr) => !ignored.has(pr.number) && passesSignalFilter(watch, pr.signals),
  );
}

function visibleIssues(watch: RepoWatch) {
  const snap = snapFor(watch);
  if (!snap) return [];
  const ignored = new Set(
    (watch.ignored ?? [])
      .filter((v) => v.kind === "issue")
      .map((v) => v.number),
  );
  return snap.issues.filter(
    (issue) => !ignored.has(issue.number) && passesSignalFilter(watch, issue.signals),
  );
}

/** Empty signal filters keep the legacy behavior (show everything). */
function passesSignalFilter(watch: RepoWatch, signals: ActionSignal[]) {
  const active = (watch.signalFilters ?? []) as ActionSignal[];
  if (!active.length) return true;
  return active.some((signal) => signals.includes(signal));
}

function summary(watch: RepoWatch) {
  const prs = visiblePrs(watch).length;
  const issues = visibleIssues(watch).length;
  const parts: string[] = [];
  if (prs) parts.push(`${prs} PR`);
  if (issues) parts.push(`${issues} Issue`);
  if (!parts.length) return "无条目";
  return parts.join(" · ");
}

function snapshotMeta(watch: RepoWatch) {
  const snap = snapFor(watch);
  if (!snap) return "";
  const fetched = `快照 ${formatTime(snap.fetchedAt)}`;
  const signals = snap.signalsComputedAt
    ? `信号 ${formatTime(snap.signalsComputedAt)}`
    : "旧缓存";
  return `${fetched} · ${signals}`;
}

async function add() {
  if (repo.value) {
    await s.addWatch(repo.value);
    repo.value = "";
  }
}

function addKey(e: KeyboardEvent) {
  if (!e.isComposing) void add();
}

async function refresh() {
  busy.value = true;
  try {
    await s.refresh();
  } finally {
    busy.value = false;
  }
}

async function refreshRepo(repoName: string) {
  repoBusy[repoName] = true;
  try {
    await s.refreshRepo(repoName);
  } finally {
    repoBusy[repoName] = false;
  }
}

function toggle(name: string, current: string[], value: string) {
  const next = current.includes(value)
    ? current.filter((v) => v !== value)
    : [...current, value];
  void s.setFilters(name, next);
}

function toggleSignalFilter(name: string, current: string[], value: ActionSignal) {
  const next = current.includes(value)
    ? current.filter((v) => v !== value)
    : [...current, value];
  void s.setSignalFilters(name, next);
}

function toggleCollapsed(watch: RepoWatch) {
  void s.setCollapsed(watch.fullName, !watch.collapsed);
}

function matchLabel(matches: string[]) {
  return matches
    .map((m) => {
      if (m === "pinned") return "手动";
      if (m === "all-prs") return "全部PR";
      const found = filters.find((f) => f[0] === m);
      return found ? found[1] : m;
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

async function ignorePr(repoName: string, pr: GhPullRequest) {
  await s.ignoreItem(repoName, pr.number, "pr");
}

async function ignoreIssue(repoName: string, issue: GhIssue) {
  await s.ignoreItem(repoName, issue.number, "issue");
}

function parsePinInput(raw: string): number | null {
  const m = raw.trim().match(/^#?(\d+)$/);
  if (!m) return null;
  return Number(m[1]);
}

async function pinFromInput(repoName: string) {
  const raw = pinDrafts[repoName] ?? "";
  const number = parsePinInput(raw);
  if (number == null) {
    pinErrors[repoName] = "请输入 #号，例如 #123";
    return;
  }
  pinBusy[repoName] = true;
  pinErrors[repoName] = null;
  try {
    await s.pinItem(repoName, number);
    pinDrafts[repoName] = "";
  } catch (e) {
    pinErrors[repoName] = String(e);
  } finally {
    pinBusy[repoName] = false;
  }
}

function pinKey(repoName: string, e: KeyboardEvent) {
  if (!e.isComposing) void pinFromInput(repoName);
}

function isClosed(state: string) {
  return state !== "open";
}

function convertKey(repoName: string, kind: string, number: number) {
  return `${repoName}:${kind}:${number}`;
}

async function convertToTodo(
  repoName: string,
  kind: "pr" | "issue",
  item: { number: number; title: string; url: string },
) {
  const key = convertKey(repoName, kind, item.number);
  // Ignore rapid double-clicks while a request is in flight.
  if (convertState[key]?.phase === "saving") return;
  convertState[key] = { phase: "saving", message: "" };
  try {
    const result = await todo.createFromGithub({
      kind: kind === "pr" ? "github-pr" : "github-issue",
      repo: repoName,
      number: item.number,
      title: item.title,
      url: item.url,
    });
    convertState[key] = { phase: "ok", message: `已转为 Todo：${result.title}` };
    window.setTimeout(() => {
      if (convertState[key]?.phase === "ok") {
        convertState[key] = { phase: "idle", message: "" };
      }
    }, 4000);
  } catch (err) {
    convertState[key] = { phase: "error", message: String(err) };
    window.setTimeout(() => {
      if (convertState[key]?.phase === "error") {
        convertState[key] = { phase: "idle", message: "" };
      }
    }, 6000);
  }
}

function convertLabel(key: string) {
  const state = convertState[key];
  if (state?.phase === "saving") return "保存中…";
  if (state?.phase === "ok") return "已转入";
  if (state?.phase === "error") return "重试";
  return "转为 Todo";
}
</script>

<template>
  <section class="gh-panel">
    <div class="auth-card">
      <span class="auth-dot" :class="{ ok: s.auth?.loggedIn }"></span>
      <b>{{ s.auth?.message || "检测 GitHub CLI…" }}</b>
      <small>{{ s.auth?.version }}</small>
    </div>
    <div class="toolbar">
      <input
        v-model="repo"
        class="input"
        placeholder="owner/repo"
        @keyup.enter="addKey"
      />
      <button class="btn primary" @click="add">添加仓库</button>
      <button class="btn" :disabled="busy" @click="refresh">
        {{ busy ? "刷新中…" : "全部刷新" }}
      </button>
    </div>
    <p v-if="s.error" class="error">{{ s.error }}</p>

    <article
      v-for="watch in s.watchlist"
      :key="watch.fullName"
      class="snapshot"
      :class="{ collapsed: watch.collapsed }"
    >
      <header class="snapshot-header">
        <button
          type="button"
          class="gh-accordion-toggle"
          :aria-expanded="!watch.collapsed"
          @click="toggleCollapsed(watch)"
        >
          <span class="gh-chevron" aria-hidden="true">{{
            watch.collapsed ? "▸" : "▾"
          }}</span>
          <h3>{{ watch.fullName }}</h3>
          <span class="snapshot-meta">{{ summary(watch) }}</span>
          <small v-if="snapshotMeta(watch)" class="snapshot-time">{{ snapshotMeta(watch) }}</small>
        </button>
        <button
          class="btn"
          :disabled="repoBusy[watch.fullName]"
          :title="`刷新 ${watch.fullName}`"
          @click="refreshRepo(watch.fullName)"
        >
          {{ repoBusy[watch.fullName] ? "刷新中…" : "刷新" }}
        </button>
        <button class="btn danger" @click="s.removeWatch(watch.fullName)">移除</button>
      </header>

      <template v-if="!watch.collapsed">
        <div class="filter-row">
          <label v-for="f in filters" :key="f[0]">
            <input
              type="checkbox"
              :checked="watch.filters.includes(f[0])"
              @change="toggle(watch.fullName, watch.filters, f[0])"
            />
            {{ f[1] }}
          </label>
        </div>

        <div class="filter-row gh-signal-row">
          <span class="gh-signal-label">行动信号</span>
          <label v-for="f in signalFilters" :key="f[0]">
            <input
              type="checkbox"
              :checked="(watch.signalFilters ?? []).includes(f[0])"
              @change="toggleSignalFilter(watch.fullName, watch.signalFilters ?? [], f[0])"
            />
            {{ f[1] }}
          </label>
          <button
            v-if="(watch.signalFilters ?? []).length"
            class="btn ghost compact gh-clear-signals"
            type="button"
            title="清除行动信号过滤，恢复默认列表"
            @click="s.setSignalFilters(watch.fullName, [])"
          >
            清除
          </button>
        </div>

        <div class="gh-pin-row">
          <input
            v-model="pinDrafts[watch.fullName]"
            class="input"
            placeholder="#123 手动关注"
            :disabled="pinBusy[watch.fullName]"
            @keyup.enter="pinKey(watch.fullName, $event)"
          />
          <button
            class="btn"
            :disabled="pinBusy[watch.fullName]"
            @click="pinFromInput(watch.fullName)"
          >
            {{ pinBusy[watch.fullName] ? "添加中…" : "添加" }}
          </button>
        </div>
        <p v-if="pinErrors[watch.fullName]" class="error">
          {{ pinErrors[watch.fullName] }}
        </p>

        <template v-if="snapFor(watch)">
          <p v-if="snapFor(watch)!.lastError" class="error">
            缓存数据：{{ snapFor(watch)!.lastError }}
          </p>
          <p v-if="!snapFor(watch)!.signalsComputedAt" class="gh-empty">
            旧缓存：无行动信号字段，刷新后自动补全
          </p>

          <h4>Pull Requests</h4>
          <div
            v-for="pr in visiblePrs(watch)"
            :key="'pr-' + pr.number"
            class="gh-item-row"
            :class="{ dimmed: isClosed(pr.state) }"
          >
            <button class="gh-link" type="button" @click="s.open(pr.url)">
              <span class="gh-title-block">
                <span class="gh-title">#{{ pr.number }} {{ pr.title }}</span>
                <span class="gh-item-meta-row">
                  <span class="gh-signals">
                    <span
                      v-for="b in signalBadges(pr.signals)"
                      :key="b.key"
                      class="gh-signal-badge"
                      :class="b.key"
                    >
                      {{ b.label }}
                    </span>
                  </span>
                  <small class="gh-link-meta">
                    {{ matchLabel(pr.matches) }} · 更新 {{ formatTime(pr.updatedAt) }}
                  </small>
                </span>
              </span>
            </button>
            <button
              class="btn ghost gh-convert"
              type="button"
              title="转为 Todo（收件箱）"
              :disabled="convertState[convertKey(watch.fullName, 'pr', pr.number)]?.phase === 'saving'"
              @click="convertToTodo(watch.fullName, 'pr', pr)"
            >
              {{ convertLabel(convertKey(watch.fullName, 'pr', pr.number)) }}
            </button>
            <small
              v-if="convertState[convertKey(watch.fullName, 'pr', pr.number)]?.phase === 'ok'"
              class="gh-convert-ok"
              :title="convertState[convertKey(watch.fullName, 'pr', pr.number)]!.message"
            >
              {{ convertState[convertKey(watch.fullName, 'pr', pr.number)]!.message }}
            </small>
            <small
              v-else-if="convertState[convertKey(watch.fullName, 'pr', pr.number)]?.phase === 'error'"
              class="error gh-convert-error"
              :title="convertState[convertKey(watch.fullName, 'pr', pr.number)]!.message"
            >
              {{ convertState[convertKey(watch.fullName, 'pr', pr.number)]!.message }}
            </small>
            <button
              class="btn ghost gh-ignore"
              type="button"
              title="忽略"
              @click="ignorePr(watch.fullName, pr)"
            >
              忽略
            </button>
          </div>
          <p v-if="!visiblePrs(watch).length" class="gh-empty">暂无 PR，可用 #号 手动添加</p>

          <h4>Issues</h4>
          <div
            v-for="issue in visibleIssues(watch)"
            :key="'issue-' + issue.number"
            class="gh-item-row"
            :class="{ dimmed: isClosed(issue.state) }"
          >
            <button class="gh-link" type="button" @click="s.open(issue.url)">
              <span class="gh-title-block">
                <span class="gh-title">#{{ issue.number }} {{ issue.title }}</span>
                <span class="gh-item-meta-row">
                  <span class="gh-signals">
                    <span
                      v-for="b in signalBadges(issue.signals)"
                      :key="b.key"
                      class="gh-signal-badge"
                      :class="b.key"
                    >
                      {{ b.label }}
                    </span>
                  </span>
                  <small class="gh-link-meta">
                    {{ matchLabel(issue.matches) }} · 更新 {{ formatTime(issue.updatedAt) }}
                  </small>
                </span>
              </span>
            </button>
            <button
              class="btn ghost gh-convert"
              type="button"
              title="转为 Todo（收件箱）"
              :disabled="convertState[convertKey(watch.fullName, 'issue', issue.number)]?.phase === 'saving'"
              @click="convertToTodo(watch.fullName, 'issue', issue)"
            >
              {{ convertLabel(convertKey(watch.fullName, 'issue', issue.number)) }}
            </button>
            <small
              v-if="convertState[convertKey(watch.fullName, 'issue', issue.number)]?.phase === 'ok'"
              class="gh-convert-ok"
              :title="convertState[convertKey(watch.fullName, 'issue', issue.number)]!.message"
            >
              {{ convertState[convertKey(watch.fullName, 'issue', issue.number)]!.message }}
            </small>
            <small
              v-else-if="convertState[convertKey(watch.fullName, 'issue', issue.number)]?.phase === 'error'"
              class="error gh-convert-error"
              :title="convertState[convertKey(watch.fullName, 'issue', issue.number)]!.message"
            >
              {{ convertState[convertKey(watch.fullName, 'issue', issue.number)]!.message }}
            </small>
            <button
              class="btn ghost gh-ignore"
              type="button"
              title="忽略"
              @click="ignoreIssue(watch.fullName, issue)"
            >
              忽略
            </button>
          </div>
          <p v-if="!visibleIssues(watch).length" class="gh-empty">
            暂无 Issue，可用 #号 手动添加
          </p>
        </template>
        <p v-else class="gh-empty">尚未拉取数据，点「全部刷新」或添加 #号</p>
      </template>
    </article>
  </section>
</template>
