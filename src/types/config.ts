export interface AppConfig {
  schemaVersion: number;
  dataDir: string;
  hotCorner: "off" | "top-left" | "top-right" | "bottom-left" | "bottom-right";
  hotkey: string;
  quickCaptureHotkey: string;
  quickCaptureEnabled: boolean;
  /** Global hotkey for the command palette window (default `Ctrl+K`). */
  commandPaletteHotkey: string;
  /** Whether the command palette window and its hotkey are enabled. */
  commandPaletteEnabled: boolean;
  theme: "system" | "dark" | "light";
  githubRefreshIntervalMinutes: number;
  /** Days after which an open GitHub item is flagged "长期未更新"; 0 disables. */
  githubStaleDays: number;
  /** Sync linked GitHub sources into Todo metadata. */
  githubSyncEnabled: boolean;
  /** Automatically complete linked Todos when their source closes or merges. */
  githubAutoCompleteTodos: boolean;
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
