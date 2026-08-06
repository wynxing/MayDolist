<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useGithubStore } from "../stores/github";
import type { GhIssue, GhPullRequest, RepoSnapshot, RepoWatch } from "../types/github";

const s = useGithubStore();
const repo = ref("");
const busy = ref(false);
const pinDrafts = reactive<Record<string, string>>({});
const pinBusy = reactive<Record<string, boolean>>({});
const pinErrors = reactive<Record<string, string | null>>({});

const filters = [
  ["mine", "我的"],
  ["mentioned", "被提及"],
  ["assigned", "被分配"],
  ["involved", "参与"],
  ["all-prs", "全部 PR"],
] as const;

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
  return snap.pullRequests.filter((pr) => !ignored.has(pr.number));
}

function visibleIssues(watch: RepoWatch) {
  const snap = snapFor(watch);
  if (!snap) return [];
  const ignored = new Set(
    (watch.ignored ?? [])
      .filter((v) => v.kind === "issue")
      .map((v) => v.number),
  );
  return snap.issues.filter((issue) => !ignored.has(issue.number));
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

function toggle(name: string, current: string[], value: string) {
  const next = current.includes(value)
    ? current.filter((v) => v !== value)
    : [...current, value];
  void s.setFilters(name, next);
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

          <h4>Pull Requests</h4>
          <div
            v-for="pr in visiblePrs(watch)"
            :key="'pr-' + pr.number"
            class="gh-item-row"
            :class="{ dimmed: isClosed(pr.state) }"
          >
            <button class="gh-link" type="button" @click="s.open(pr.url)">
              <span>#{{ pr.number }} {{ pr.title }}</span>
              <small>{{ matchLabel(pr.matches) }}</small>
            </button>
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
              <span>#{{ issue.number }} {{ issue.title }}</span>
              <small>{{ matchLabel(issue.matches) }}</small>
            </button>
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
