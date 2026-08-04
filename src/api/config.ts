import type { AppConfig } from "../types/config";
import { call } from "./index";

export const getConfig = () => call<AppConfig>("get_config");

export const getDataDir = () => call<string>("get_data_dir");

export const setConfig = (config: AppConfig) =>
  call<AppConfig>("set_config", { config });
