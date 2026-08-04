<script setup lang="ts">
import { onMounted, ref } from "vue";
import EmptyState from "../components/EmptyState.vue";
import { useGithubStore } from "../stores/github";

const store = useGithubStore();
const newRepo = ref("");
const refreshing = ref(false);

onMounted(() => void store.init());

async function addRepo() {
  const repo = newRepo.value.trim();
  if (!repo) return;
  await store.addWatch(repo);
  newRepo.value = "";
}

async function refresh() {
  refreshing.value = true;
  try {
    await store.refresh();
  } finally {
    refreshing.value = false;
  }
}
</script>

<template>
  <section class="github-view">
    <div class="toolbar">
      <div v-if="store.auth" class="auth-card">
        <span class="auth-dot" :class="{ ok: store.auth.loggedIn }"></span>
        <span v-if="store.auth.loggedIn">已登录：{{ store.auth.user }}</span>
        <span v-else>未登录</span>
        <span class="auth-message">{{ store.auth.message }}</span>
      </div>
      <div class="watch-add">
        <input
          v-model="newRepo"
          class="input"
          placeholder="owner/repo，如 microsoft/vscode"
          @keyup.enter="addRepo"
        />
        <button class="btn primary" @click="addRepo">添加仓库</button>
        <button class="btn ghost" :disabled="refreshing" @click="refresh">
          {{ refreshing ? "刷新中…" : "刷新" }}
        </button>
      </div>
    </div>

    <p v-if="store.error" class="error">{{ store.error }}</p>

    <div class="watch-chips">
      <span v-for="watch in store.watchlist" :key="watch.fullName" class="chip">
        {{ watch.fullName }}
        <button class="chip-remove" title="移除" @click="store.removeWatch(watch.fullName)">
          ×
        </button>
      </span>
      <span v-if="store.watchlist.length === 0" class="muted">尚未跟踪任何仓库</span>
    </div>

    <div v-if="store.snapshots.length === 0" class="snapshots-empty">
      <EmptyState text="点击「刷新」拉取快照" />
    </div>

    <div class="snapshots">
      <article v-for="snapshot in store.snapshots" :key="snapshot.repo" class="snapshot">
        <header class="snapshot-header">
          <h3>{{ snapshot.repo }}</h3>
          <span class="snapshot-meta">更新于 {{ snapshot.fetchedAt.slice(11, 19) }}</span>
        </header>

        <div class="snapshot-section">
          <h4>Pull Requests</h4>
          <ul v-if="snapshot.pullRequests.length" class="gh-items">
            <li v-for="pr in snapshot.pullRequests" :key="pr.number">
              <a :href="pr.url" target="_blank" rel="noreferrer">
                #{{ pr.number }} {{ pr.draft ? "[draft] " : "" }}{{ pr.title }}
              </a>
              <span class="state" :class="pr.state">{{ pr.state }}</span>
            </li>
          </ul>
          <p v-else class="muted">无</p>
        </div>

        <div class="snapshot-section">
          <h4>Issues</h4>
          <ul v-if="snapshot.issues.length" class="gh-items">
            <li v-for="issue in snapshot.issues" :key="issue.number">
              <a :href="issue.url" target="_blank" rel="noreferrer">
                #{{ issue.number }} {{ issue.title }}
              </a>
              <span class="state" :class="issue.state">{{ issue.state }}</span>
            </li>
          </ul>
          <p v-else class="muted">无</p>
        </div>
      </article>
    </div>
  </section>
</template>
