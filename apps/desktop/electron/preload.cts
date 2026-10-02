import { contextBridge, ipcRenderer, webUtils } from 'electron';
import type { FileformAPI, Appearance } from '../src/contracts.js';
const api:FileformAPI = {
  chooseFiles:(task)=>ipcRenderer.invoke('fileform:choose-files',task),
  importFiles:(files)=>ipcRenderer.invoke('fileform:import-files',files.map(file=>webUtils.getPathForFile(file))),
  runTask:(ids,task,options)=>ipcRenderer.invoke('fileform:run-task',ids,task,options),
  openResult:(id)=>ipcRenderer.invoke('fileform:open-result',id),
  previewPages:(pages)=>ipcRenderer.invoke('fileform:preview-pages',pages),
  onOpenFile:(callback)=>{
    const listener=()=>callback();
    ipcRenderer.on('fileform:open-request',listener);
    return ()=>ipcRenderer.removeListener('fileform:open-request',listener);
  },
  chooseImage:()=>ipcRenderer.invoke('fileform:choose-image'),
  saveImage:(id,options)=>ipcRenderer.invoke('fileform:save-image',id,options),
  cancel:()=>ipcRenderer.invoke('fileform:cancel'),
  chooseTable:()=>ipcRenderer.invoke('fileform:choose'),
  saveTable:(id,format)=>ipcRenderer.invoke('fileform:save',id,format),
  reveal:(id:string)=>ipcRenderer.invoke('fileform:reveal',id),
  appearance:(mode?:Appearance)=>ipcRenderer.invoke('fileform:appearance',mode)
};
contextBridge.exposeInMainWorld('fileform',api);
