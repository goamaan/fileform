export type Appearance = 'system'|'light'|'dark';
export interface SourceFile { id:string; name:string; bytes:number; rows:number; columns:number }
export interface SavedFile { id:string; name:string; bytes:number; rows:number }
export interface FileformAPI {
  chooseTable():Promise<SourceFile|null>;
  saveJSON(sourceID:string):Promise<SavedFile|null>;
  reveal(savedID:string):Promise<void>;
  appearance(mode?:Appearance):Promise<{mode:Appearance;dark:boolean}>;
}
declare global { interface Window { fileform:FileformAPI } }
