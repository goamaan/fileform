import React,{useEffect,useState} from 'react';
import {createRoot} from 'react-dom/client';
import type {Appearance,AssetSource,TaskOptions,TaskResult} from './contracts';
import {tasks,availableTasksForAssets,type TaskID} from './tasks';
import {ImageWorkspace} from './image-workspace';
import {PdfPageEditor} from './pdf-page-editor';
import {MediaEditor} from './media-editor';
import {useOpenFile} from './use-open-file';
import './style.css';
const bytes=(value:number)=>value<1000?value+' B':new Intl.NumberFormat(undefined,{style:'unit',unit:value>=1e6?'megabyte':'kilobyte',maximumFractionDigits:1}).format(value/(value>=1e6?1e6:1e3));
function FileIcon(){return <svg width="28" height="32" viewBox="0 0 28 32" fill="none" aria-hidden="true"><path d="M6 1h11l9 9v18a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V4a3 3 0 0 1 3-3Z" stroke="currentColor"/><path d="M17 1v9h9M8 17h12M8 22h8" stroke="currentColor"/></svg>}
function App(){
 const [files,setFiles]=useState<AssetSource[]>([]);const [selected,setSelected]=useState<string[]>([]);
 const [task,setTask]=useState<TaskID>('image.convert');const [options,setOptions]=useState<TaskOptions>({format:'png'});
 const [result,setResult]=useState<TaskResult|null>(null);const [busy,setBusy]=useState(false);const [childBusy,setChildBusy]=useState(false);const [cancelling,setCancelling]=useState(false);const [error,setError]=useState('');const [dragging,setDragging]=useState(false);const [query,setQuery]=useState('');
 const [appearance,setAppearance]=useState<Appearance>('system');
 const active=files.filter(file=>selected.includes(file.id));const available=availableTasksForAssets(active);const definition=available.find(v=>v.id===task);
 const first=active[0];const isTrim=task==='audio.trim'||task==='video.trim';const isMedia=task.startsWith('audio.')||task.startsWith('video.');
 const applyTheme=async(mode?:Appearance)=>{const state=await window.fileform.appearance(mode);setAppearance(state.mode);document.documentElement.dataset.theme=state.dark?'dark':'light';};
 useEffect(()=>{void applyTheme().catch(()=>setError('Appearance could not be loaded.'));const media=matchMedia('(prefers-color-scheme: dark)');const changed=()=>void applyTheme().catch(()=>{});media.addEventListener('change',changed);return()=>media.removeEventListener('change',changed);},[]);
 async function run(work:()=>Promise<void>){setBusy(true);setError('');try{await work();}catch(e){setError(e instanceof Error?e.message.replace(/^Error invoking remote method '[^']+': Error: /,''):'The file could not be processed.');}finally{setBusy(false);setCancelling(false);}}
 function selectTask(value:TaskID,source:AssetSource|undefined=first){
  const entry=tasks.find(v=>v.id===value)!;const preservePages=['pdf.combine','pdf.images'].includes(task)&&['pdf.combine','pdf.images'].includes(value);setTask(value);setResult(null);setOptions(previous=>({format:value==='table.convert'?source?.table?.outputs[0]??entry.outputFormats[0]:entry.outputFormats[0],start:'0',end:String(source?.duration??0),dpi:144,quality:value==='pdf.compress'?80:85,minimumQuality:50,splitEvery:1,pageOrder:preservePages?previous.pageOrder:undefined}));
 }
 function adopt(imported:AssetSource[],preferred?:TaskID){
  if(!imported.length)return;
  setFiles(previous=>{const map=new Map(previous.map(v=>[v.id,v]));for(const file of imported)map.set(file.id,file);return [...map.values()];});
  const inserting=!preferred&&task==='pdf.combine'&&active.length>0&&imported.every(v=>v.family==='pdf'||v.family==='image');
  setSelected(previous=>inserting?[...new Set([...previous,...imported.map(v=>v.id)])]:imported.map(v=>v.id));setResult(null);
  if(inserting)return;
  const actions=availableTasksForAssets(imported);const chosen=actions.find(v=>v.id===preferred)??actions.find(v=>v.id===imported[0].family+'.convert')??actions[0];
  if(preferred&&!actions.some(v=>v.id===preferred))setError('These files cannot use “'+tasks.find(v=>v.id===preferred)!.label+'”. Choose an available action.');
  if(chosen)selectTask(chosen.id,imported[0]);
 }
 const choose=()=>run(async()=>adopt(await window.fileform.chooseFiles()));
 useOpenFile(!busy&&!childBusy,choose);
 const patch=(value:Partial<TaskOptions>)=>{setOptions(previous=>({...previous,...value}));setResult(null);};
 const number=(value:string)=>value===''?undefined:Number(value);
 const cancel=()=>{setCancelling(true);void window.fileform.cancel().catch(()=>{setCancelling(false);setError('Cancellation could not be requested.');});};
 const emptyActions=tasks.filter(v=>!query||v.label.toLowerCase().includes(query.toLowerCase()));
 return <main className="app-shell unified-shell" onDragOver={event=>{event.preventDefault();if(!busy&&!childBusy)setDragging(true);}} onDragLeave={event=>{if(!event.currentTarget.contains(event.relatedTarget as Node))setDragging(false);}} onDrop={event=>{event.preventDefault();setDragging(false);if(busy||childBusy)return;const dropped=Array.from(event.dataTransfer.files);if(dropped.length)void run(async()=>adopt(await window.fileform.importFiles(dropped)));}}>
  <header><div className="brand"><span className="brand-mark" aria-hidden="true">F</span>Fileform</div><div className="header-controls"><button disabled={busy||childBusy} onClick={choose}>Add files <kbd>Ctrl / ⌘ O</kbd></button><select aria-label="Appearance" value={appearance} onChange={e=>void applyTheme(e.target.value as Appearance)}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></div></header>
  {!files.length?<section className="start-screen"><div className="heading"><div><h1>Your files. Your next step.</h1><p>Convert, edit, and organize locally.</p></div></div><button className={'dropzone '+(dragging?'dragging':'')} disabled={busy} onClick={choose}><FileIcon/><strong>{busy?'Reading files…':'Drop files here'}</strong><span>or choose files</span></button><div className="task-heading"><h2>Start with an action</h2><input aria-label="Search actions" placeholder="Find an action…" value={query} onChange={e=>setQuery(e.target.value)}/></div><div className="task-shortcuts">{emptyActions.map(entry=><button key={entry.id} disabled={busy} onClick={()=>run(async()=>adopt(await window.fileform.chooseFiles(entry.id),entry.id))}>{entry.label}<span aria-hidden="true">↗</span></button>)}</div>{error&&<p className="error" role="alert">{error}</p>}</section>:<div className="workbench">
   <aside className="file-list"><div className="list-heading"><h2>Files <span>{files.length}</span></h2><button disabled={busy||childBusy} onClick={choose}>+</button></div>{files.map(file=><label className={'source-row '+(selected.includes(file.id)?'selected':'')} key={file.id}><input type="checkbox" disabled={busy||childBusy} checked={selected.includes(file.id)} onChange={e=>{const ids=e.target.checked?[...selected,file.id]:selected.filter(v=>v!==file.id);setSelected(ids);setResult(null);const choices=availableTasksForAssets(files.filter(v=>ids.includes(v.id)));if(!choices.some(v=>v.id===task)&&choices[0])selectTask(choices[0].id,files.find(v=>v.id===ids[0]));}}/><FileIcon/><div><strong>{file.name}</strong><span>{file.family.toUpperCase()} · {bytes(file.bytes)}</span></div><button type="button" aria-label={'Remove '+file.name} disabled={busy||childBusy} onClick={event=>{event.preventDefault();setFiles(previous=>previous.filter(v=>v.id!==file.id));setSelected(previous=>previous.filter(v=>v!==file.id));}}>×</button></label>)}<p className="local-note">Originals stay unchanged.</p></aside>
   <section className="task-workspace"><div className="heading"><h1>{active.length===1?first.name:active.length+' files'}</h1>{active.length===1&&first&&<span className="format">{first.pages?first.pages+' pages':first.duration?first.duration.toFixed(1)+' seconds':first.table?first.table.rows+(first.table.rows===1?' row':' rows'):''}</span>}</div>
    <div className="action-bar"><label>Action<select aria-label="Action" disabled={busy||childBusy||!active.length} value={definition?task:''} onChange={e=>selectTask(e.target.value as TaskID)}>{!definition&&<option value="">Choose compatible files</option>}{available.map(v=><option key={v.id} value={v.id}>{v.label}</option>)}</select></label></div>
    {definition&&(task==='pdf.combine'||task==='pdf.images'||task==='pdf.split'&&options.splitAfter!==undefined)&&<PdfPageEditor key={task==='pdf.split'?'split-markers':'editable-pages'} sources={active} disabled={busy} onChange={pages=>patch({pageOrder:pages})} onBusyChange={setChildBusy} markers={task==='pdf.split'?options.splitAfter:undefined} onMarkersChange={splitAfter=>patch({splitAfter})}/>}
    {task==='image.convert'&&first?.image&&active.length===1?<ImageWorkspace hidden={false} selectedSource={first.image} onBusyChange={setChildBusy} onChoose={choose}/>:definition?<div className={isMedia?'media-workspace-layout':undefined}>
     {isMedia&&first&&<MediaEditor key={first.id} source={first} audioOnly={task.startsWith('audio.')} trim={isTrim} options={options} disabled={busy} onChange={patch} onBusyChange={setChildBusy}/>}
     <div className="task-panel"><fieldset className="option-grid" disabled={busy||childBusy}><label>Output<select aria-label="Output format" disabled={busy} value={options.format} onChange={e=>patch({format:e.target.value})}>{(task==='table.convert'?first?.table?.outputs??[]:definition.outputFormats).map(value=><option key={value} value={value}>{value.toUpperCase()}</option>)}</select></label>
      {task==='pdf.split'&&<label>Split by<select aria-label="PDF split mode" value={options.splitAfter!==undefined?'markers':options.pages!==undefined?'ranges':'every'} onChange={e=>patch({splitAfter:e.target.value==='markers'?[]:undefined,pages:e.target.value==='ranges'?'':undefined,splitEvery:e.target.value==='every'?1:undefined})}><option value="every">Page intervals</option><option value="ranges">Page ranges</option><option value="markers">Page markers</option></select></label>}
      {(task==='pdf.extract-images'||task==='pdf.split'&&options.pages!==undefined)&&<label>{task==='pdf.split'?'Page groups':'Pages'}<input aria-label="Page selection" placeholder={task==='pdf.split'?'1-3;4-6':'All pages, or 3,1-2'} value={options.pages??''} onChange={e=>patch({pages:e.target.value})}/></label>}
      {task==='pdf.split'&&options.pages===undefined&&options.splitAfter===undefined&&<label>Pages per part<input aria-label="Pages per split part" type="number" min="1" max="1000" value={options.splitEvery??1} onChange={e=>patch({splitEvery:number(e.target.value)})}/></label>}
      {task==='pdf.images'&&<label>Resolution<input aria-label="PDF export DPI" type="number" min="36" max="600" value={options.dpi??144} onChange={e=>patch({dpi:number(e.target.value)})}/>DPI</label>}
      {task==='pdf.compress'&&<label className="check-option"><input type="checkbox" checked={options.lossy??false} onChange={e=>patch({lossy:e.target.checked})}/>Recompress images</label>}
      {(task==='pdf.compress'&&options.lossy||task==='pdf.images'&&options.format==='jpeg')&&<label>Quality<input aria-label="Quality" type="number" min="5" max="100" value={options.quality??85} onChange={e=>patch({quality:number(e.target.value)})}/></label>}
      {(task==='pdf.compress'||task==='audio.convert'||task==='video.convert')&&<label>Size limit (bytes)<input aria-label="Maximum output bytes" type="number" min="1" placeholder="Optional" value={options.maxBytes??''} onChange={e=>patch({maxBytes:number(e.target.value)})}/></label>}
      {task==='video.convert'&&<label>Longest edge (px)<input aria-label="Maximum video dimension" type="number" min="2" placeholder="Original size" value={options.maxDimension??''} onChange={e=>patch({maxDimension:number(e.target.value)})}/></label>}
      {isMedia&&!!first?.audioTracks?.length&&!options.muteAudio&&<label>Audio track<select aria-label="Audio track" value={options.audioStream??''} onChange={e=>patch({audioStream:number(e.target.value)})}><option value="">{first.audioTracks.length===1?'Default track':'Choose a track'}</option>{first.audioTracks.map(v=><option value={v.index} key={v.index}>{v.language??'Track '+v.index} · {v.codec.toUpperCase()} · {v.channels} channels</option>)}</select></label>}
      {isTrim&&<><label>From (seconds)<input aria-label="Trim start" value={options.start??'0'} onChange={e=>patch({start:e.target.value})}/></label><label>To (seconds)<input aria-label="Trim end" value={options.end??''} onChange={e=>patch({end:e.target.value})}/></label><label className="check-option"><input type="checkbox" checked={options.fast??false} onChange={e=>patch({fast:e.target.checked,format:task==='audio.trim'&&e.target.checked?'m4a':options.format})}/>Fast packet copy</label></>}
      {task==='video.trim'&&<label className="check-option"><input type="checkbox" checked={options.muteAudio??false} onChange={e=>patch({muteAudio:e.target.checked,audioStream:undefined})}/>Mute audio</label>}
     </fieldset>
     {task==='table.convert'&&first?.table?.scalarTypesBecomeText&&<p className="task-note">JSON values become text cells; null becomes an empty cell.</p>}
     {task==='text.ocr'&&<p className="task-note">English recognition is available in this preview.</p>}
     {(task==='pdf.combine'||task==='pdf.split')&&<p className="task-note">Creates new documents. Bookmarks, metadata, and form behavior may change.</p>}
     {task==='pdf.compress'&&options.lossy&&<p className="task-note">Image recompression is lossy; unsupported images remain unchanged.</p>}
     <div className="actions"><span>{active.length} {active.length===1?'file':'files'} selected</span><button className="primary" disabled={busy||childBusy||!active.length||!!options.pageOrder&&(options.pageOrder.length===0||options.pageOrder.length>1000)} onClick={()=>run(async()=>{const saved=await window.fileform.runTask(active.map(v=>v.id),task,options);if(saved)setResult(saved);})}>{busy?'Processing…':'Save result…'}</button></div></div>
    </div>:<p className="task-note">Select files to see their available actions.</p>}
    {error&&<p className="error" role="alert">{error}</p>}
    {result&&<article className="result" role="status"><span aria-hidden="true">✓</span><div><h2>{result.name}</h2><p>{result.summary} · {bytes(result.bytes)}</p></div>{result.warnings.length>0&&<details className="result-notes"><summary>Output notes</summary>{result.warnings.map((note,index)=><p key={index}>{note}</p>)}</details>}{result.id&&<><button onClick={()=>run(()=>window.fileform.openResult(result.id))}>Open</button><button onClick={()=>run(()=>window.fileform.reveal(result.id))}>Show in folder</button></>}</article>}
   </section>
  </div>}
  {busy&&<div className="progress" role="status"><span>Processing locally…</span><button disabled={cancelling} onClick={cancel}>{cancelling?'Cancelling…':'Cancel'}</button></div>}
  <footer><span>Local processing · Free and open source</span><span>Fileform Preview</span></footer>
 </main>;
}
createRoot(document.getElementById('root')!).render(<App/>);
