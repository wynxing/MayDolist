<script setup lang="ts">
import { onMounted, ref } from "vue";
import { call } from "../api";
import { useSettingsStore } from "../stores/settings";

type TrashRow = { id: string; title: string };
type Trash = {
  todoLists?: TrashRow[];
  todoItems?: TrashRow[];
  notes?: TrashRow[];
};

const settings = useSettingsStore();
const target = ref("");
const trash = ref<Trash | null>(null);
const message = ref("");

const trashGroups = [
  { kind: "todoList", key: "todoLists", label: "待办列表" },
  { kind: "todoItem", key: "todoItems", label: "待办事项" },
  { kind: "note", key: "notes", label: "便签" },
] as const;

onMounted(async () => {
  await settings.init();
  trash.value = await call<Trash>("trash_list");
});

async function save() {
  if (!settings.config) return;
  await settings.update(settings.config);
  message.value = "设置已保存";
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
      <h3>数据目录</h3>
      <p class="settings-path">{{ settings.config.dataDir }}</p>
      <div class="settings-migrate">
        <input v-model="target" class="input" placeholder="输入新的绝对目录" />
        <button class="btn" :disabled="!target" @click="migrate">迁移</button>
      </div>
    </div>

    <div class="settings-section">
      <h3>回收站</h3>
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
              <button class="btn" @click="action(group.kind, row.id, 'trash_restore')">恢复</button>
              <button
                class="btn danger"
                @click="action(group.kind, row.id, 'trash_delete_permanently')"
              >
                永久删除
              </button>
            </div>
          </div>
        </template>
        <p
          v-if="trash && trashGroups.every((group) => !(trash?.[group.key]?.length))"
          class="settings-empty"
        >
          回收站为空
        </p>
      </div>
    </div>
  </section>
</template>
