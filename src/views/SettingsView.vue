<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { call } from "../api";
import * as backupApi from "../api/backup";
import { useSettingsStore } from "../stores/settings";
import { useUpdateStore } from "../stores/update";
import type { BackupInfo } from "../types/backup";

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
const recentBackups = ref<BackupInfo[]>([]);
const includeCache = ref(true);
const dataBusy = ref(false);
const dataMessage = ref("");
const dataError = ref("");

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

const quietEnabled = computed(() => !!settings.config?.quietHours);

onMounted(async () => {
  await settings.init();
  await updater.init();
  trash.value = await call<Trash>("trash_list");
  recentBackups.value = await backupApi.listBackups().catch(() => []);
});

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function dataStamp() {
  const d = new Date();
  const pad = (v: number) => String(v).padStart(2, "0");
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
}

async function refreshBackups() {
  recentBackups.value = await backupApi.listBackups();
}

async function runDataAction(label: string, fn: () => Promise<void>) {
  dataBusy.value = true;
  dataError.value = "";
  dataMessage.value = "";
  try {
    await fn();
  } catch (err) {
    dataError.value = `${label}失败：${err instanceof Error ? err.message : String(err)}`;
  } finally {
    dataBusy.value = false;
  }
}

async function exportData() {
  const target = await saveDialog({
    title: "导出 MayDolist 数据",
    defaultPath: `maydolist-export-${dataStamp()}.zip`,
    filters: [{ name: "MayDolist 数据包", extensions: ["zip"] }],
  });
  if (!target) return;
  await runDataAction("导出", async () => {
    const info = await backupApi.exportData(target, includeCache.value);
    dataMessage.value = `已导出到 ${info.path}（便签 ${info.notes}、待办列表 ${info.todos}${info.githubCache ? `、GitHub 缓存 ${info.githubCache}` : ""}）`;
  });
}

async function importData() {
  const file = await open({
    title: "导入 MayDolist 数据",
    multiple: false,
    directory: false,
    filters: [{ name: "MayDolist 数据包", extensions: ["zip"] }],
  });
  if (!file || Array.isArray(file)) return;
  await runDataAction("导入", async () => {
    const preview = await backupApi.inspectPackage(file);
    const confirmText =
      `即将导入数据包：\n` +
      `- 包格式版本：${preview.packageSchemaVersion}\n` +
      `- 导出应用版本：${preview.appVersion}\n` +
      `- 便签 ${preview.notes} 份、待办列表 ${preview.todos} 份\n` +
      `- GitHub 追踪列表：${preview.hasWatchlist ? "有" : "无"}、缓存 ${preview.githubCache} 份` +
      (preview.skippedCache ? `（将跳过损坏缓存 ${preview.skippedCache} 份）` : "") +
      `\n\n导入会覆盖当前数据，导入前会自动备份现有数据。确定继续？`;
    if (!window.confirm(confirmText)) return;
    const info = await backupApi.importPackage(file);
    dataMessage.value = `导入完成：便签 ${info.notes}、待办列表 ${info.todos}${info.githubCache ? `、GitHub 缓存 ${info.githubCache}` : ""}${info.skippedCache ? `（跳过损坏缓存 ${info.skippedCache} 份）` : ""}；导入前已自动备份到 ${info.backupPath}`;
    trash.value = await call<Trash>("trash_list");
    await refreshBackups();
  });
}

async function createBackup() {
  await runDataAction("创建备份", async () => {
    const info = await backupApi.createBackup();
    dataMessage.value = `备份已创建：${info.path}`;
    await refreshBackups();
  });
}

async function openDataDir() {
  await runDataAction("打开数据目录", async () => {
    await backupApi.openDataDir();
  });
}

function formatCheckTime(value: string | null) {
  return value ? new Date(value).toLocaleString("zh-CN") : "尚未检查";
}

async function save() {
  if (!settings.config) return;
  try {
    await settings.update(settings.config);
    message.value = "设置已保存";
  } catch (err) {
    message.value = `保存失败：${err instanceof Error ? err.message : String(err)}`;
  }
}

function toggleQuietHours() {
  if (!settings.config) return;
  settings.config.quietHours = quietEnabled.value ? null : { start: "22:00", end: "07:00" };
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
          <span>快速收集快捷键</span>
          <input
            v-model="settings.config.quickCaptureHotkey"
            class="input settings-control"
            :disabled="!settings.config.quickCaptureEnabled"
          />
        </label>

        <div class="settings-row">
          <span>快速收集</span>
          <label class="settings-switch">
            <input v-model="settings.config.quickCaptureEnabled" type="checkbox" />
            <span>启用快速收集窗口（默认 Ctrl+Alt+Space）</span>
          </label>
        </div>

        <label class="settings-row">
          <span>命令面板快捷键</span>
          <input
            v-model="settings.config.commandPaletteHotkey"
            class="input settings-control"
            :disabled="!settings.config.commandPaletteEnabled"
          />
        </label>

        <div class="settings-row">
          <span>命令面板</span>
          <label class="settings-switch">
            <input v-model="settings.config.commandPaletteEnabled" type="checkbox" />
            <span>启用全局命令面板（默认 Ctrl+K）</span>
          </label>
        </div>

        <div class="settings-row">
          <span>
            提醒安静时段
            <small>该时段内到期提醒不弹通知，仅托盘徽标提示</small>
          </span>
          <label class="settings-switch">
            <input :checked="quietEnabled" type="checkbox" @change="toggleQuietHours" />
            <span>启用安静时段</span>
          </label>
        </div>

        <div v-if="quietEnabled && settings.config.quietHours" class="settings-row">
          <span>安静时段</span>
          <span class="settings-control quiet-hours-controls">
            <input v-model="settings.config.quietHours.start" type="time" class="input" aria-label="安静时段开始" />
            <span class="quiet-hours-sep">至</span>
            <input v-model="settings.config.quietHours.end" type="time" class="input" aria-label="安静时段结束" />
          </span>
        </div>

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

        <label class="settings-row">
          <span>
            GitHub 长期未更新阈值
            <small>天，0 为关闭该信号</small>
          </span>
          <input
            v-model.number="settings.config.githubStaleDays"
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
      <h3>数据安全</h3>
      <p class="settings-note">
        数据包为 ZIP 格式，包含配置、待办、便签与 GitHub 追踪列表，不含任何登录凭据；
        导入前会先自动备份当前数据，校验失败不会改动现有数据。
      </p>
      <div class="settings-grid">
        <label class="settings-row">
          <span>
            包含 GitHub 缓存
            <small>导出时附带可重建的 GitHub 快照缓存，离线可查看</small>
          </span>
          <label class="settings-switch">
            <input v-model="includeCache" type="checkbox" />
            <span>导出时包含</span>
          </label>
        </label>
      </div>
      <div class="settings-actions data-actions">
        <button class="btn primary" :disabled="dataBusy" @click="exportData">导出数据</button>
        <button class="btn" :disabled="dataBusy" @click="importData">导入数据</button>
        <button class="btn" :disabled="dataBusy" @click="createBackup">创建备份</button>
        <button class="btn" :disabled="dataBusy" @click="openDataDir">打开数据目录</button>
      </div>
      <p v-if="dataMessage" class="settings-message data-message" role="status">{{ dataMessage }}</p>
      <p v-if="dataError" class="update-error" role="alert">{{ dataError }}</p>
      <div v-if="recentBackups.length" class="backup-list">
        <div class="backup-row" v-for="backup in recentBackups" :key="backup.path">
          <span class="backup-name">{{ backup.name }}</span>
          <span class="backup-meta">{{ backup.createdAt }} · {{ formatSize(backup.size) }}</span>
        </div>
      </div>
      <p v-else class="settings-empty">暂无备份，可点击「创建备份」或通过导入自动生成</p>
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
