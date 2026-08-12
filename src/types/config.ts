export interface AppConfig {
  schemaVersion: number;
  dataDir: string;
  hotCorner: "off" | "top-left" | "top-right" | "bottom-left" | "bottom-right";
  hotkey: string;
  quickCaptureHotkey: string;
  quickCaptureEnabled: boolean;
  theme: "system" | "dark" | "light";
  githubRefreshIntervalMinutes: number;
  /** Days after which an open GitHub item is flagged "长期未更新"; 0 disables. */
  githubStaleDays: number;
  /** Optional quiet window for due reminders; `null` keeps reminders always on. */
  quietHours: QuietHours | null;
  autostart: boolean;
  firstRun: boolean;
  mainWindowGlassOpacity: number;
  floatingNoteGlassOpacity: number;
}

/** Local `HH:MM` (24h) quiet window; may cross midnight. */
export interface QuietHours {
  start: string;
  end: string;
}
