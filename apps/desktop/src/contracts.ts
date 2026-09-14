export type Appearance = 'system'|'light'|'dark';
export type TableOutput = 'json'|'csv'|'tsv';
export interface SourceFile { id:string; name:string; bytes:number; rows:number; columns:number; outputs:TableOutput[]; scalarTypesBecomeText:boolean }
export interface SavedFile { id:string; name:string; bytes:number; rows:number }
export interface ImagePreview { width:number; height:number; rgba:number[] }
export interface ImageSource { id:string; name:string; bytes:number; width:number; height:number; hasAlpha:boolean; canConvert:boolean; preview:ImagePreview|null }
export interface ImageSavedFile { id:string; name:string; bytes:number; width:number; height:number }
export interface FileformAPI {
  chooseImage():Promise<ImageSource|null>;
  saveImage(sourceID:string):Promise<ImageSavedFile|null>;
  cancel():Promise<void>;
  chooseTable():Promise<SourceFile|null>;
  saveTable(sourceID:string,format:TableOutput):Promise<SavedFile|null>;
  reveal(savedID:string):Promise<void>;
  appearance(mode?:Appearance):Promise<{mode:Appearance;dark:boolean}>;
}
declare global { interface Window { fileform:FileformAPI } }
