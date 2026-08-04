import type { Snippet } from "../types/snippet";
import { call } from "./index";

export const list = () => call<Snippet[]>("snippet_list");

export const create = (title: string, content: string, tags: string[]) =>
  call<Snippet>("snippet_create", { title, content, tags });

export const update = (
  id: string,
  patch: { title?: string; content?: string; tags?: string[] },
) => call<Snippet>("snippet_update", { id, ...patch });

export const remove = (id: string) => call<void>("snippet_delete", { id });
