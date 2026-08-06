import { defineStore } from "pinia";
import { ref } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as api from "../api/config";
import type { AppConfig } from "../types/config";

export const useSettingsStore = defineStore("settings", () => {
  const config = ref<AppConfig | null>(null);
  const error = ref<string | null>(null);

  let initPromise: Promise<void> | null = null;
  let unlistenSettings: UnlistenFn | null = null;
  let unlistenGlassPreview: UnlistenFn | null = null;
  let systemTheme: MediaQueryList | null = null;

  const isFloatingWindow = new URLSearchParams(location.search).has("note");

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

  const applyGlass = () => {
    const value = config.value
      ? isFloatingWindow
        ? config.value.floatingNoteGlassOpacity
        : config.value.mainWindowGlassOpacity
      : undefined;
    document.documentElement.dataset.window = isFloatingWindow ? "floating" : "main";
    if (value !== undefined) {
      document.documentElement.style.setProperty("--glass-opacity", String(value));
    }
  };

  const appliesToWindow = (
    key: "mainWindowGlassOpacity" | "floatingNoteGlassOpacity"
  ) => (key === "floatingNoteGlassOpacity") === isFloatingWindow;

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
          applyGlass();
        });
        unlistenGlassPreview = await listen<{
          key: "mainWindowGlassOpacity" | "floatingNoteGlassOpacity";
          opacity: number;
        }>("glass-preview", (event) => {
          if (appliesToWindow(event.payload.key)) {
            document.documentElement.style.setProperty(
              "--glass-opacity",
              String(event.payload.opacity)
            );
          }
        });

        config.value = await api.get();
        error.value = null;
        applyTheme();
        applyGlass();
      } catch (e) {
        error.value = String(e);
        unlistenSettings?.();
        unlistenSettings = null;
        unlistenGlassPreview?.();
        unlistenGlassPreview = null;
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
    applyGlass();
  };

  /** Live preview of a glass opacity value without persisting it. */
  const previewGlass = async (
    key: "mainWindowGlassOpacity" | "floatingNoteGlassOpacity",
    opacity: number
  ) => {
    if (appliesToWindow(key)) {
      document.documentElement.style.setProperty("--glass-opacity", String(opacity));
    }
    await emit("glass-preview", { key, opacity });
  };

  return {
    config,
    error,
    init,
    update,
    previewGlass,
    isFloatingWindow,
    migrate: api.migrate,
    setAutostart: api.autostart,
  };
});
