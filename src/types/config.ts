export interface AppConfig {
  schemaVersion: number;
  dataDir: string;
  hotCorner: "off" | "top-left" | "top-right" | "bottom-left" | "bottom-right";
  hotkey: string;
  quickCaptureHotkey: string;
  quickCaptureEnabled: boolean;
  theme: "system" | "dark" | "light";
  githubRefreshIntervalMinutes: number;
  autostart: boolean;
  firstRun: boolean;
  mainWindowGlassOpacity: number;
  floatingNoteGlassOpacity: number;
}
