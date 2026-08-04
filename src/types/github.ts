export interface GhAuthStatus{state:string;loggedIn:boolean;user:string|null;version:string|null;message:string}
export interface RepoWatch{fullName:string;filters:string[]}
export interface GhIssue{number:number;title:string;state:string;url:string;updatedAt:string;kind:string;matches:string[]}
export interface GhPullRequest{number:number;title:string;state:string;draft:boolean;url:string;updatedAt:string;matches:string[]}
export interface RepoSnapshot{schemaVersion:number;repo:string;fetchedAt:string;lastSuccessAt:string|null;lastError:string|null;issues:GhIssue[];pullRequests:GhPullRequest[]}
