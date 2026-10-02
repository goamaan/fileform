export type Appearance = 'system'|'light'|'dark';
export type TableOutput = 'json'|'csv'|'tsv';
export interface SourceFile { id:string; name:string; bytes:number; rows:number; columns:number; outputs:TableOutput[]; scalarTypesBecomeText:boolean }
export interface SavedFile { id:string; name:string; bytes:number; rows:number }
export interface PixelCrop { x:number; y:number; width:number; height:number }
export interface ImageExportOptions { format:'png'|'jpeg'|'tiff';background?:'white'|'black';quality?:number;crop?:PixelCrop;maxDimension?:number;maxBytes?:number;minimumQuality?:number }
export interface ImagePreview { width:number; height:number; rgba:number[] }
export interface ImageSource { id:string; name:string; bytes:number; width:number; height:number; hasAlpha:boolean; canConvert:boolean; preview:ImagePreview|null }
export interface ImageSavedFile { id:string; name:string; bytes:number; width:number; height:number;quality:number|null }
export interface FileformAPI {
  onOpenFile(callback:()=>void):()=>void;
  chooseImage():Promise<ImageSource|null>;
  saveImage(sourceID:string,options:ImageExportOptions):Promise<ImageSavedFile|null>;
  cancel():Promise<void>;
  chooseTable():Promise<SourceFile|null>;
  saveTable(sourceID:string,format:TableOutput):Promise<SavedFile|null>;
  reveal(savedID:string):Promise<void>;
  appearance(mode?:Appearance):Promise<{mode:Appearance;dark:boolean}>;
}
declare global { interface Window { fileform:FileformAPI } }

export interface AudioTrack {index:number;codec:string;channels:number;sampleRate:number;language?:string}
export interface AssetSource {
  id:string;name:string;bytes:number;family:import('./tasks.js').FileFamily;
  table?:SourceFile;image?:ImageSource;pages?:number;duration?:number;audioTracks?:AudioTrack[];
}
export interface TaskOptions {
  format:string;maxDimension?:number;maxBytes?:number;quality?:number;minimumQuality?:number;
  background?:'white'|'black';crop?:PixelCrop;pages?:string;splitEvery?:number;splitAfter?:number[];
  start?:string;end?:string;audioStream?:number;muteAudio?:boolean;fast?:boolean;dpi?:number;lossy?:boolean;
  pageOrder?:PageChoice[];
}
export interface PageChoice {sourceID:string;pageIndex:number;rotation:number}
export interface PageThumbnail {sourceID:string;pageIndex:number;dataURL:string;width:number;height:number}
export interface MediaPreviewOptions {audioOnly:boolean;audioStream?:number;muteAudio?:boolean}
export interface MediaPreview {
 id:string;url:string;kind:'audio'|'video';duration:{ticks:number;timescale:number};poster?:string;
 waveform?:{sampleRate:number;sampleCount:number;samplesPerBucket:number;channels:{minimum:number[];maximum:number[]}[]};
}
export interface TaskResult {id:string;name:string;bytes:number;summary:string;warnings:string[];folder:boolean}
export interface FileformAPI {
  chooseFiles(task?:import('./tasks.js').TaskID):Promise<AssetSource[]>;
  importFiles(files:File[]):Promise<AssetSource[]>;
  runTask(ids:string[],task:import('./tasks.js').TaskID,options:TaskOptions):Promise<TaskResult|null>;
  openResult(id:string):Promise<void>;
  previewPages(pages:{sourceID:string;pageIndex:number}[]):Promise<PageThumbnail[]>;
  previewMedia(id:string,options:MediaPreviewOptions):Promise<MediaPreview>;
  releaseMediaPreview(id:string):Promise<void>;
}
