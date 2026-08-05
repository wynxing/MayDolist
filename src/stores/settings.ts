import { defineStore } from "pinia";
import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as api from "../api/config";
import type { AppConfig } from "../types/config";

export const useSettingsStore = defineStore("settings", () => {
  const config = ref<AppConfig | null>(null);
  const error = ref<string | null>(null);

  let initPromise: Promise<void> | null = null;
  let unlistenSettings: UnlistenFn | null = null;
  let systemTheme: MediaQueryList | null = null;

  const applyTheme = () => {
    const value = config.value?.theme ?? "system";
    const resolved =
      value === "system"
        ? window.matchMedia("(prefers-color-scheme: light)").matches
          ? "light"
          : "dark"
        : value;
    document.documentElement.dataset.theme = resolved;
  };

  const init = () => {
    if (initPromise) return initPromise;

    initPromise = (async () => {
      try {
        systemTheme = window.matchMedia("(prefers-color-scheme: light)");
        systemTheme.addEventListener("change", applyTheme);

        unlistenSettings = await listen<AppConfig>("settings-changed", (event) => {
          config.value = event.payload;
          error.value = null;
          applyTheme();
        });

        config.value = await api.get();
        error.value = null;
        applyTheme();
      } catch (e) {
        error.value = String(e);
        unlistenSettings?.();
        unlistenSettings = null;
        systemTheme?.removeEventListener("change", applyTheme);
        systemTheme = null;
        initPromise = null;
      }
    })();

    return initPromise;
  };

  const update = async (patch: Partial<AppConfig>) => {
    if (!config.value) return;
    config.value = await api.update({ ...config.value, ...patch });
    error.value = null;
    applyTheme();
  };

  return {
    config,
    error,
    init,
    update,
    migrate: api.migrate,
    setAutostart: api.autostart,
  };
});
