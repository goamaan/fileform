import { contextBridge, ipcRenderer } from 'electron';
import type { FileformAPI, Appearance } from '../src/contracts.js';
const api:FileformAPI = {
  chooseImage:()=>ipcRenderer.invoke('fileform:choose-image'),
  saveImage:(id,format,background,quality,crop)=>ipcRenderer.invoke('fileform:save-image',id,format,background,quality,crop),
  cancel:()=>ipcRenderer.invoke('fileform:cancel'),
  chooseTable:()=>ipcRenderer.invoke('fileform:choose'),
  saveTable:(id,format)=>ipcRenderer.invoke('fileform:save',id,format),
  reveal:(id:string)=>ipcRenderer.invoke('fileform:reveal',id),
  appearance:(mode?:Appearance)=>ipcRenderer.invoke('fileform:appearance',mode)
};
contextBridge.exposeInMainWorld('fileform',api);
