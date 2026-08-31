import { call } from "./index";
import type { UpdateRuntimeInfo } from "../types/update";

export const runtimeInfo = () => call<UpdateRuntimeInfo>("update_runtime_info");
