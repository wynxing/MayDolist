import type { Note } from "../types/note";
import { call } from "./index";

export const list = () => call<Note[]>("note_list");

export const create = (title: string, content: string) =>
  call<Note>("note_create", { title, content });

export const update = (id: string, patch: { title?: string; content?: string }) =>
  call<Note>("note_update", { id, ...patch });
