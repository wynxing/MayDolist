import type { TodoItem, TodoList } from "../types/todo";
import { call } from "./index";

export const list = () => call<TodoList[]>("todo_list");

export const createList = (title: string) =>
  call<TodoList>("todo_create_list", { title });

export const createItem = (listId: string, title: string) =>
  call<TodoItem>("todo_create_item", { listId, title });

export const updateItem = (
  id: string,
  patch: { title?: string; completed?: boolean },
) => call<TodoItem>("todo_update_item", { id, ...patch });

export const softDelete = (id: string) =>
  call<void>("todo_soft_delete", { id });
