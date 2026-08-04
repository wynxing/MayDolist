export interface TodoItem { id:string; title:string; completed:boolean; deleted:boolean; sortOrder:number; createdAt:string; updatedAt:string }
export interface TodoList { schemaVersion:number; id:string; title:string; sortOrder:number; deleted:boolean; createdAt:string; updatedAt:string; items:TodoItem[] }
