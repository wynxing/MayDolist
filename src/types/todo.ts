/** Mirrors Rust `models::todo::TodoSource`; `type` is `"github-issue"` or `"github-pr"`. */
export interface TodoSource { type:string; repo:string; number:number; url:string }
export interface TodoItem { id:string; title:string; completed:boolean; deleted:boolean; sortOrder:number; createdAt:string; updatedAt:string; source?:TodoSource|null }
export interface TodoList { schemaVersion:number; id:string; title:string; sortOrder:number; deleted:boolean; createdAt:string; updatedAt:string; items:TodoItem[] }
/** Result of converting a GitHub PR / issue into a Todo (Rust `TodoFromGithubResult`). */
export interface TodoFromGithubResult { sourceType:string; id:string; title:string; repo:string; number:number; targetListId:string }
