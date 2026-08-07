import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { call } from "../api";
import * as api from "../api/update";
import type {
  AvailableUpdate,
  UpdateRuntimeInfo,
  UpdateStatus,
} from "../types/update";

const CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const LAST_CHECK_KEY = "maydolist.update.lastCheckAt";
const LAST_RESULT_KEY = "maydolist.update.lastResult";

export const useUpdateStore = defineStore("update", () => {
  const runtime = ref<UpdateRuntimeInfo | null>(null);
  const status = ref<UpdateStatus>("idle");
  const available = ref<AvailableUpdate | null>(null);
  const lastCheckAt = ref<string | null>(localStorage.getItem(LAST_CHECK_KEY));
  const lastResult = ref<string | null>(localStorage.getItem(LAST_RESULT_KEY));
  const error = ref<string | null>(null);
  const downloadedBytes = ref(0);
  const downloadTotal = ref<number | null>(null);
  let pendingUpdate: Update | null = null;
  let initPromise: Promise<void> | null = null;

  const busy = computed(() =>
    ["checking", "downloading"].includes(status.value)
  );
  const downloadPercent = computed(() =>
    downloadTotal.value && downloadTotal.value > 0
      ? Math.min(100, Math.round((downloadedBytes.value / downloadTotal.value) * 100))
      : null
  );

  const rememberResult = (result: string) => {
    const now = new Date().toISOString();
    lastCheckAt.value = now;
    lastResult.value = result;
    localStorage.setItem(LAST_CHECK_KEY, now);
    localStorage.setItem(LAST_RESULT_KEY, result);
  };

  const checkForUpdates = async (manual = true) => {
    if (busy.value) return;
    if (!manual && lastCheckAt.value) {
      const elapsed = Date.now() - Date.parse(lastCheckAt.value);
      if (Number.isFinite(elapsed) && elapsed < CHECK_INTERVAL_MS) return;
    }

    status.value = "checking";
    error.value = null;
    try {
      await pendingUpdate?.close();
      pendingUpdate = await check({ timeout: 15_000 });
      if (!pendingUpdate) {
        available.value = null;
        status.value = "up-to-date";
        rememberResult("up-to-date");
        return;
      }
      available.value = {
        currentVersion: pendingUpdate.currentVersion,
        version: pendingUpdate.version,
        date: pendingUpdate.date,
        body: pendingUpdate.body,
      };
      status.value = "available";
      rememberResult(`available:${pendingUpdate.version}`);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      status.value = "failed";
      rememberResult("failed");
    }
  };

  const init = () => {
    if (initPromise) return initPromise;
    initPromise = (async () => {
      runtime.value = await api.runtimeInfo();
      await checkForUpdates(false);
    })().catch((cause) => {
      error.value = cause instanceof Error ? cause.message : String(cause);
      status.value = "failed";
      initPromise = null;
    });
    return initPromise;
  };

  const install = async () => {
    if (!pendingUpdate || busy.value || runtime.value?.portable) return;
    status.value = "downloading";
    error.value = null;
    downloadedBytes.value = 0;
    downloadTotal.value = null;
    try {
      const onEvent = (event: DownloadEvent) => {
        if (event.event === "Started") {
          downloadTotal.value = event.data.contentLength ?? null;
        } else if (event.event === "Progress") {
          downloadedBytes.value += event.data.chunkLength;
        }
      };
      await pendingUpdate.downloadAndInstall(onEvent, { timeout: 120_000 });
      status.value = "ready-to-restart";
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      status.value = "failed";
    }
  };

  const openRelease = () => {
    const url = available.value
      ? `https://github.com/wynxing/MayDolist/releases/tag/v${available.value.version}`
      : runtime.value?.releaseUrl;
    return url ? call<void>("open_external", { url }) : Promise.resolve();
  };

  return {
    runtime,
    status,
    available,
    lastCheckAt,
    lastResult,
    error,
    busy,
    downloadPercent,
    init,
    checkForUpdates,
    install,
    openRelease,
    relaunch,
  };
});
