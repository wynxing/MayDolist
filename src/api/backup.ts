import { call } from "./index";
import type { BackupInfo, ExportInfo, ImportInfo, PackagePreview } from "../types/backup";

export const exportData = (target: string, includeCache: boolean) =>
  call<ExportInfo>("backup_export", { target, includeCache });

export const inspectPackage = (path: string) => call<PackagePreview>("backup_inspect", { path });

export const importPackage = (path: string) => call<ImportInfo>("backup_import", { path });

export const createBackup = () => call<BackupInfo>("backup_create");

export const listBackups = () => call<BackupInfo[]>("backup_list");

export const openDataDir = () => call<void>("backup_open_data_dir");
