import type{TodoFromGithubResult,TodoItem,TodoList,TodoSource,RepeatRule}from"../types/todo";import{call}from"./index";
/** Optional due / reminder / repeat fields (mirrors Rust `TodoScheduleInput`). */
export interface TodoScheduleInput {
  dueDate?: string | null;
  remindAt?: string | null;
  repeat?: RepeatRule | null;
  repeatUntil?: string | null;
}
export const list=(includeDeleted=false)=>call<TodoList[]>("todo_list",{includeDeleted});export const createList=(title:string)=>call<TodoList>("todo_create_list",{title});export const updateList=(id:string,patch:{title?:string;deleted?:boolean})=>call<TodoList>("todo_update_list",{id,...patch});export const reorderLists=(ids:string[])=>call<TodoList[]>("todo_reorder_lists",{ids});export const createItem=(listId:string,title:string,source?:TodoSource|null,schedule?:TodoScheduleInput)=>call<TodoItem>("todo_create_item",{listId,title,...(source?{source}:{}),...(schedule?{schedule}:{})});export const createFromGithub=(input:{kind:string;repo:string;number:number;title:string;url:string})=>call<TodoFromGithubResult>("todo_create_from_github",input);export const updateItem=(id:string,patch:{title?:string;completed?:boolean;deleted?:boolean;schedule?:TodoScheduleInput})=>call<TodoItem>("todo_update_item",{id,patch});export const moveItem=(id:string,targetListId:string,index:number)=>call<TodoItem>("todo_move_item",{id,targetListId,index});export const reorderItems=(listId:string,ids:string[])=>call<TodoList>("todo_reorder_items",{listId,ids});export const softDelete=(id:string)=>call<void>("todo_soft_delete",{id});
