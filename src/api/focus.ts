import type { FocusOverview } from "../types/focus";
import { call } from "./index";

export const overview = () => call<FocusOverview>("focus_overview");
