export type UpdateStatus =
  "idle" | "checking" | "up-to-date" | "available" | "downloading" | "ready-to-restart" | "failed";

export interface UpdateRuntimeInfo {
  currentVersion: string;
  portable: boolean;
  releaseUrl: string;
}

export interface AvailableUpdate {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
}
