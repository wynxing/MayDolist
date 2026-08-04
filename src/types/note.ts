export interface WindowBounds{x:number;y:number;width:number;height:number}
export interface Note{schemaVersion:number;id:string;title:string;content:string;tags:string[];color:string;pinned:boolean;floating:boolean;collapsed:boolean;alwaysOnTop:boolean;windowBounds:WindowBounds|null;deleted:boolean;createdAt:string;updatedAt:string}
export type NotePatch=Partial<Omit<Note,"id"|"schemaVersion"|"createdAt"|"updatedAt">>
