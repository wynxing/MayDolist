/** Mirrors Rust `models::config::AppConfig` (keep in sync). */
export interface AppConfig {
  schemaVersion: number;
  dataDir: string | null;
  hotCorner: string;
  hotkey: string;
  theme: string;
  githubRefreshIntervalMinutes: number;
}
