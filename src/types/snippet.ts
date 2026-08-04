/** Mirrors Rust `models::snippet::Snippet`. */
export interface Snippet {
  id: string;
  title: string;
  content: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}
