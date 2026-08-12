import type { TodoSource } from "./todo";

/** Mirrors Rust `models::palette::PaletteCommand`. */
export interface PaletteCommand {
  id: string;
  label: string;
  hint: string;
  keywords: string[];
}

/** One incomplete Todo matched by the palette search. */
export interface PaletteTodo {
  id: string;
  title: string;
  listId: string;
  listTitle: string;
  inbox: boolean;
  updatedAt: string;
  source: TodoSource | null;
  dueDate: string | null;
}

/** One Note matched by the palette search (title or full-text content). */
export interface PaletteNote {
  id: string;
  title: string;
  preview: string;
  pinned: boolean;
  floating: boolean;
  updatedAt: string;
}

/** One cached GitHub issue / PR matched by the palette search. */
export interface PaletteGithub {
  kind: "pr" | "issue";
  repo: string;
  number: number;
  title: string;
  url: string;
  updatedAt: string;
}

export interface PaletteSearchResult {
  query: string;
  commands: PaletteCommand[];
  todos: PaletteTodo[];
  notes: PaletteNote[];
  github: PaletteGithub[];
  /** True when GitHub results come from a stale / offline local cache. */
  githubOffline: boolean;
}
