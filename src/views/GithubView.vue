<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useGithubStore } from "../stores/github";

const s = useGithubStore();
const repo = ref("");
const busy = ref(false);
const filters = [
  ["mine", "我的"],
  ["mentioned", "被提及"],
  ["assigned", "被分配"],
  ["involved", "参与"],
];

onMounted(() => s.init());

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
</script>

<template>
  <section>
    <div class="auth-card">
      <span class="auth-dot" :class="{ ok: s.auth?.loggedIn }"></span>
      <b>{{ s.auth?.message || "检测 GitHub CLI…" }}</b>
      <small>{{ s.auth?.version }}</small>
    </div>
    <div class="toolbar">
      <input v-model="repo" class="input" placeholder="owner/repo" @keyup.enter="addKey" />
      <button class="btn primary" @click="add">添加仓库</button>
      <button class="btn" :disabled="busy" @click="refresh">
        {{ busy ? "刷新中…" : "全部刷新" }}
      </button>
    </div>
    <p v-if="s.error" class="error">{{ s.error }}</p>
    <article v-for="watch in s.watchlist" :key="watch.fullName" class="snapshot">
      <header class="snapshot-header">
        <h3>{{ watch.fullName }}</h3>
        <button class="btn danger" @click="s.removeWatch(watch.fullName)">移除</button>
      </header>
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
      <template v-if="s.snapshots.find((v) => v.repo === watch.fullName)">
        <p v-if="s.snapshots.find((v) => v.repo === watch.fullName)!.lastError" class="error">
          缓存数据：{{ s.snapshots.find((v) => v.repo === watch.fullName)!.lastError }}
        </p>
        <h4>Pull Requests</h4>
        <button
          v-for="pr in s.snapshots.find((v) => v.repo === watch.fullName)!.pullRequests"
          :key="pr.number"
          class="gh-link"
          @click="s.open(pr.url)"
        >
          #{{ pr.number }} {{ pr.title }} <small>{{ pr.matches.join(" · ") }}</small>
        </button>
        <h4>Issues</h4>
        <button
          v-for="issue in s.snapshots.find((v) => v.repo === watch.fullName)!.issues"
          :key="issue.number"
          class="gh-link"
          @click="s.open(issue.url)"
        >
          #{{ issue.number }} {{ issue.title }} <small>{{ issue.matches.join(" · ") }}</small>
        </button>
      </template>
    </article>
  </section>
</template>
