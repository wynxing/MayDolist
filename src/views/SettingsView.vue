<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { call } from "../api";
import { useSettingsStore } from "../stores/settings";
import { useUpdateStore } from "../stores/update";

type TrashRow = { id: string; title: string };
type Trash = {
  todoLists?: TrashRow[];
  todoItems?: TrashRow[];
  notes?: TrashRow[];
};

const settings = useSettingsStore();
const updater = useUpdateStore();
const target = ref("");
const trash = ref<Trash | null>(null);
const message = ref("");

const trashGroups = [
  { kind: "todoList", key: "todoLists", label: "待办列表" },
  { kind: "todoItem", key: "todoItems", label: "待办事项" },
  { kind: "note", key: "notes", label: "便签" },
] as const;

const trashCount = computed(
  () =>
    (trash.value?.todoLists?.length ?? 0) +
    (trash.value?.todoItems?.length ?? 0) +
    (trash.value?.notes?.length ?? 0)
);

onMounted(async () => {
  await settings.init();
  await updater.init();
  trash.value = await call<Trash>("trash_list");
});

function formatCheckTime(value: string | null) {
  return value ? new Date(value).toLocaleString("zh-CN") : "尚未检查";
}

async function save() {
  if (!settings.config) return;
  await settings.update(settings.config);
  message.value = "设置已保存";
}

function previewOpacity(
  key: "mainWindowGlassOpacity" | "floatingNoteGlassOpacity",
  event: Event
) {
  if (!settings.config) return;
  const value = Number((event.target as HTMLInputElement).value);
  settings.config[key] = value;
  void settings.previewGlass(key, value);
}

async function migrate() {
  if (!target.value) return;
  const dataDir = await settings.migrate(target.value);
  if (settings.config) settings.config.dataDir = dataDir;
  target.value = "";
  message.value = "数据目录已迁移";
}

async function action(kind: string, id: string, command: string) {
  await call(command, { kind, id });
  trash.value = await call<Trash>("trash_list");
}

async function permanentlyDelete(kind: string, id: string, title: string) {
  if (!window.confirm(`永久删除“${title}”？此操作不可撤销。`)) return;
  await action(kind, id, "trash_delete_permanently");
}

async function clearTrash() {
  const count = trashCount.value;
  if (!count) return;
  if (!window.confirm(`清空回收站中的 ${count} 项？此操作不可撤销。`)) return;
  await call("trash_clear");
  trash.value = await call<Trash>("trash_list");
  message.value = "回收站已清空";
}
</script>

<template>
  <section v-if="settings.config" class="settings glass-card">
    <header class="settings-header">
      <div>
        <h2>设置</h2>
        <p>调整 MayDolist 的外观与后台行为</p>
      </div>
      <span v-if="message" class="settings-message" role="status">{{ message }}</span>
    </header>

    <div class="settings-section">
      <h3>常规设置</h3>
      <div class="settings-grid">
        <label class="settings-row">
          <span>主题</span>
          <select v-model="settings.config.theme" class="input settings-control">
            <option value="system">跟随系统</option>
            <option value="dark">深色</option>
            <option value="light">浅色</option>
          </select>
        </label>

        <label class="settings-row">
          <span>热角</span>
          <select v-model="settings.config.hotCorner" class="input settings-control">
            <option value="off">关闭</option>
            <option value="top-left">左上</option>
            <option value="top-right">右上</option>
            <option value="bottom-left">左下</option>
            <option value="bottom-right">右下</option>
          </select>
        </label>

        <label class="settings-row">
          <span>全局快捷键</span>
          <input v-model="settings.config.hotkey" class="input settings-control" />
        </label>

        <label class="settings-row">
          <span>
            GitHub 刷新间隔
            <small>分钟，0 为关闭</small>
          </span>
          <input
            v-model.number="settings.config.githubRefreshIntervalMinutes"
            type="number"
            min="0"
            class="input settings-control"
          />
        </label>

        <div class="settings-row">
          <span>开机自启</span>
          <label class="settings-switch">
            <input
              v-model="settings.config.autostart"
              type="checkbox"
              @change="settings.setAutostart(settings.config!.autostart)"
            />
            <span>启动 Windows 时运行</span>
          </label>
        </div>
      </div>
      <div class="settings-actions">
        <button class="btn primary" @click="save">应用设置</button>
      </div>
    </div>

    <div class="settings-section">
      <h3>玻璃透明度</h3>
      <p class="settings-note">仅调整玻璃背景层的不透明度，不影响文字与控件。40% 为最低可用值。</p>
      <div class="settings-grid">
        <label class="settings-row">
          <span>主面板透明度</span>
          <span class="settings-control slider-control">
            <input
              type="range"
              min="0.4"
              max="1"
              step="0.01"
              :value="settings.config.mainWindowGlassOpacity"
              aria-label="主面板玻璃透明度"
              @input="previewOpacity('mainWindowGlassOpacity', $event)"
            />
            <b class="slider-value">{{ Math.round(settings.config.mainWindowGlassOpacity * 100) }}%</b>
          </span>
        </label>

        <label class="settings-row">
          <span>
            悬浮便签透明度
            <small>统一作用于全部悬浮便签</small>
          </span>
          <span class="settings-control slider-control">
            <input
              type="range"
              min="0.4"
              max="1"
              step="0.01"
              :value="settings.config.floatingNoteGlassOpacity"
              aria-label="悬浮便签玻璃透明度"
              @input="previewOpacity('floatingNoteGlassOpacity', $event)"
            />
            <b class="slider-value">{{ Math.round(settings.config.floatingNoteGlassOpacity * 100) }}%</b>
          </span>
        </label>
      </div>
      <div class="settings-actions">
        <button class="btn primary" @click="save">应用设置</button>
      </div>
    </div>

    <div class="settings-section">
      <div class="update-heading">
        <div>
          <h3>关于与更新</h3>
          <p class="settings-note">安装版可安全下载并安装签名更新；便携版仅跳转到 GitHub Release。</p>
        </div>
        <span class="update-version">v{{ updater.runtime?.currentVersion ?? "—" }}</span>
      </div>
      <div class="update-summary">
        <div><small>运行方式</small><strong>{{ updater.runtime?.portable ? "便携版 / 开发版" : "NSIS 安装版" }}</strong></div>
        <div><small>上次检查</small><strong>{{ formatCheckTime(updater.lastCheckAt) }}</strong></div>
        <div><small>状态</small><strong>{{ updater.status === "checking" ? "正在检查" : updater.status === "up-to-date" ? "已是最新版" : updater.status === "available" ? `发现 v${updater.available?.version}` : updater.status === "downloading" ? "正在下载" : updater.status === "ready-to-restart" ? "等待重启" : updater.status === "failed" ? "检查失败" : "尚未检查" }}</strong></div>
      </div>
      <div v-if="updater.available" class="update-release">
        <strong>MayDolist v{{ updater.available.version }}</strong>
        <small v-if="updater.available.date">{{ new Date(updater.available.date).toLocaleDateString("zh-CN") }}</small>
        <p>{{ updater.available.body || "此版本没有发行说明。" }}</p>
      </div>
      <p v-if="updater.status === 'downloading'" class="settings-note" role="status">正在下载更新{{ updater.downloadPercent === null ? "…" : `：${updater.downloadPercent}%` }}</p>
      <p v-if="updater.error" class="update-error" role="alert">{{ updater.error }}</p>
      <div class="settings-actions update-actions">
        <button class="btn" :disabled="updater.busy" @click="updater.checkForUpdates(true)">{{ updater.status === "checking" ? "检查中…" : "检查更新" }}</button>
        <button v-if="updater.available && !updater.runtime?.portable" class="btn primary" :disabled="updater.busy" @click="updater.install">{{ updater.status === "downloading" ? "正在安装…" : "下载并安装" }}</button>
        <button v-if="updater.available || updater.runtime?.portable" class="btn" @click="updater.openRelease">打开 Release</button>
        <button v-if="updater.status === 'ready-to-restart'" class="btn primary" @click="updater.relaunch">重启并完成更新</button>
      </div>
    </div>

    <div class="settings-section">
      <h3>数据目录</h3>
      <p class="settings-path">{{ settings.config.dataDir }}</p>
      <div class="settings-migrate">
        <input v-model="target" class="input" placeholder="输入新的绝对目录" />
        <button class="btn" :disabled="!target" @click="migrate">迁移</button>
      </div>
    </div>

    <div class="settings-section">
      <div class="trash-heading">
        <h3>回收站</h3>
        <button
          v-if="trashCount > 0"
          class="btn danger compact"
          @click="clearTrash"
        >
          清空回收站 ({{ trashCount }})
        </button>
      </div>
      <div class="trash-list">
        <template v-for="group in trashGroups" :key="group.kind">
          <div
            v-for="row in trash?.[group.key] ?? []"
            :key="row.id"
            class="trash-row"
          >
            <span class="trash-kind">{{ group.label }}</span>
            <span class="trash-title">{{ row.title }}</span>
            <div class="trash-actions">
              <button class="btn compact" @click="action(group.kind, row.id, 'trash_restore')">恢复</button>
              <button
                class="btn danger compact"
                @click="permanentlyDelete(group.kind, row.id, row.title)"
              >
                永久删除
              </button>
            </div>
          </div>
        </template>
        <p
          v-if="trash && trashCount === 0"
          class="settings-empty"
        >
          回收站为空
        </p>
      </div>
    </div>
  </section>
</template>
