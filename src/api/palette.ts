import { call } from "./index";
import type { PaletteSearchResult } from "../types/palette";

export const search = (query: string) =>
  call<PaletteSearchResult>("palette_search", { query });

export const hide = () => call<void>("palette_hide");
