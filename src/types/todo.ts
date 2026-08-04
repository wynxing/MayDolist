/** Mirrors Rust `models::todo::TodoItem`. */
export interface TodoItem {
  id: string;
  title: string;
  completed: boolean;
  deleted: boolean;
  createdAt: string;
}

/** Mirrors Rust `models::todo::TodoList`. */
export interface TodoList {
  id: string;
  title: string;
  items: TodoItem[];
}
