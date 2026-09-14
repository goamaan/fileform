import { contextBridge, ipcRenderer } from 'electron';
import type { FileformAPI, Appearance } from '../src/contracts.js';
const api:FileformAPI = {
  chooseTable:()=>ipcRenderer.invoke('fileform:choose'),
  saveJSON:(id:string)=>ipcRenderer.invoke('fileform:save',id),
  reveal:(id:string)=>ipcRenderer.invoke('fileform:reveal',id),
  appearance:(mode?:Appearance)=>ipcRenderer.invoke('fileform:appearance',mode)
};
contextBridge.exposeInMainWorld('fileform',api);
