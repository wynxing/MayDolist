import { defineStore } from "pinia";
import { ref } from "vue";
import { getConfig, getDataDir, setConfig } from "../api/config";
import type { AppConfig } from "../types/config";

export const useSettingsStore = defineStore("settings", () => {
  const config = ref<AppConfig | null>(null);
  const dataDir = ref<string | null>(null);
  const error = ref<string | null>(null);
  const loading = ref(false);

  async function init() {
    if (config.value) return;
    loading.value = true;
    error.value = null;
    try {
      config.value = await getConfig();
      dataDir.value = await getDataDir();
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    } finally {
      loading.value = false;
    }
  }

  async function update(patch: Partial<AppConfig>) {
    if (!config.value) throw new Error("config not loaded");
    const next = { ...config.value, ...patch };
    config.value = await setConfig(next);
  }

  return { config, dataDir, error, loading, init, update };
});
