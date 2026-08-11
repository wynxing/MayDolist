/** Content overview of a data package shown before import is confirmed. */
export interface PackagePreview {
  packageSchemaVersion: number;
  appVersion: string;
  createdAt: string;
  hasConfig: boolean;
  hasWatchlist: boolean;
  notes: number;
  todos: number;
  githubCache: number;
  /** Rebuildable cache files that failed validation and were skipped. */
  skippedCache: number;
}

export interface ExportInfo {
  path: string;
  notes: number;
  todos: number;
  githubCache: number;
}

export interface ImportInfo {
  path: string;
  backupPath: string;
  notes: number;
  todos: number;
  githubCache: number;
  skippedCache: number;
}

export interface BackupInfo {
  name: string;
  path: string;
  size: number;
  createdAt: string;
}
