// SPDX-License-Identifier: Apache-2.0
import {useEffect,useRef,useState} from 'react';
import type {AssetSource,ImagePreview,PageChoice,PageThumbnail} from './contracts';

type Entry=PageChoice&{key:string};
const entriesFor=(source:AssetSource):Entry[]=>Array.from({length:source.pages??1},(_,pageIndex)=>({sourceID:source.id,pageIndex,rotation:0,key:crypto.randomUUID()}));
function ImageThumbnail({preview}:{preview:ImagePreview}){
  const canvas=useRef<HTMLCanvasElement>(null);
  useEffect(()=>{const context=canvas.current?.getContext('2d');context?.putImageData(new ImageData(new Uint8ClampedArray(preview.rgba),preview.width,preview.height),0,0);},[preview]);
  return <canvas ref={canvas} width={preview.width} height={preview.height} aria-label="Image page preview"/>;
}

export function PdfPageEditor({sources,disabled,onChange,onBusyChange,markers,onMarkersChange}:{
  sources:AssetSource[];disabled:boolean;onChange:(pages:PageChoice[])=>void;onBusyChange:(busy:boolean)=>void;
  markers?:number[];onMarkersChange?:(markers:number[])=>void;
}){
  const [entries,setEntries]=useState<Entry[]>([]);const [selected,setSelected]=useState<Set<string>>(new Set());
  const [past,setPast]=useState<Entry[][]>([]);const [future,setFuture]=useState<Entry[][]>([]);
  const [batch,setBatch]=useState(0);const [preparing,setPreparing]=useState(false);const [error,setError]=useState('');
  const [retry,setRetry]=useState(0);const [thumbnails,setThumbnails]=useState<Map<string,PageThumbnail>>(new Map());
  const known=useRef(new Set<string>());const thumbnailsRef=useRef(thumbnails);thumbnailsRef.current=thumbnails;
  const changed=useRef(onChange);changed.current=onChange;
  const sourceKey=sources.map(v=>v.id).join('|');const editable=markers===undefined;
  useEffect(()=>{
    const ids=new Set(sources.map(v=>v.id));
    const added=sources.filter(v=>!known.current.has(v.id)).flatMap(entriesFor);
    setEntries(previous=>[...previous.filter(v=>ids.has(v.sourceID)),...added]);
    known.current=ids;setPast([]);setFuture([]);setError('');
  },[sourceKey]);
  useEffect(()=>{if(editable)changed.current(entries.map(({sourceID,pageIndex,rotation})=>({sourceID,pageIndex,rotation})));},[entries,editable]);
  const lastBatch=Math.max(0,Math.ceil(entries.length/12)-1);const currentBatch=Math.min(batch,lastBatch);
  const visible=entries.slice(currentBatch*12,(currentBatch+1)*12);
  const thumbnailKey=(v:Pick<PageChoice,'sourceID'|'pageIndex'>)=>v.sourceID+':'+v.pageIndex;
  const visibleKey=visible.map(thumbnailKey).join('|');
  useEffect(()=>{
    const seen=new Set<string>();const needed=visible.filter(v=>sources.find(s=>s.id===v.sourceID)?.family==='pdf'&&!thumbnailsRef.current.has(thumbnailKey(v))&&!seen.has(thumbnailKey(v))&&(seen.add(thumbnailKey(v)),true));
    if(!needed.length)return;
    let current=true;setPreparing(true);onBusyChange(true);setError('');
    window.fileform.previewPages(needed.map(v=>({sourceID:v.sourceID,pageIndex:v.pageIndex}))).then(result=>{
      if(!current)return;
      setThumbnails(previous=>{const next=new Map(previous);for(const thumb of result)next.set(thumbnailKey(thumb),thumb);while(next.size>96)next.delete(next.keys().next().value!);return next;});
    }).catch(e=>{if(current)setError(e instanceof Error?e.message.replace(/^Error invoking remote method '[^']+': Error: /,''):'Page previews could not be prepared.');}).finally(()=>{if(current)setPreparing(false);onBusyChange(false);});
    return()=>{current=false;};
  },[visibleKey,retry]);
  const locked=disabled||preparing;
  function commit(next:Entry[]){setPast(previous=>[...previous.slice(-31),entries]);setFuture([]);setEntries(next);}
  function undo(){if(past.length){setFuture(previous=>[entries,...previous]);setEntries(past[past.length-1]);setPast(previous=>previous.slice(0,-1));}}
  function redo(){if(future.length){setPast(previous=>[...previous.slice(-31),entries]);setEntries(future[0]);setFuture(previous=>previous.slice(1));}}
  function move(direction:-1|1){
    const next=[...entries];const indices=direction===-1?next.map((_,i)=>i):next.map((_,i)=>i).reverse();
    for(const i of indices){const target=i+direction;if(selected.has(next[i].key)&&target>=0&&target<next.length&&!selected.has(next[target].key))[next[i],next[target]]=[next[target],next[i]];}
    if(next.some((v,i)=>v!==entries[i]))commit(next);
  }
  function drop(key:string,target:string){
    const moving=entries.find(v=>v.key===key);const index=entries.findIndex(v=>v.key===target);if(!moving||index<0||key===target)return;
    const next=entries.filter(v=>v.key!==key);next.splice(next.findIndex(v=>v.key===target),0,moving);commit(next);
  }
  const selection=entries.filter(v=>selected.has(v.key));
  return <section className="page-editor" aria-label={editable?'PDF page editor':'PDF split markers'} onKeyDown={event=>{
    if(locked||!editable||!(event.metaKey||event.ctrlKey)||event.key.toLowerCase()!=='z'||['INPUT','TEXTAREA','SELECT'].includes((event.target as HTMLElement).tagName))return;
    event.preventDefault();event.shiftKey?redo():undo();
  }}>
    <div className="page-toolbar"><h2>{editable?'Pages':'Split markers'} <span>{entries.length}</span></h2>
      {editable&&<div className="page-edit-actions"><button disabled={locked||!past.length} onClick={undo}>Undo</button><button disabled={locked||!future.length} onClick={redo}>Redo</button><button disabled={locked||!selection.length} onClick={()=>move(-1)}>Move earlier</button><button disabled={locked||!selection.length} onClick={()=>move(1)}>Move later</button><button disabled={locked||!selection.length} onClick={()=>commit(entries.map(v=>selected.has(v.key)?{...v,rotation:(v.rotation+90)%360}:v))}>Rotate</button><button disabled={locked||!selection.length||entries.length+selection.length>1000} onClick={()=>commit(entries.flatMap(v=>selected.has(v.key)?[v,{...v,key:crypto.randomUUID()}]:[v]))}>Duplicate</button><button disabled={locked||!selection.length} onClick={()=>commit(entries.filter(v=>!selected.has(v.key)))}>Remove pages</button></div>}
    </div>
    {editable&&<div className="page-selection"><button disabled={locked} onClick={()=>setSelected(new Set(entries.map(v=>v.key)))}>Select all</button><button disabled={locked||!selection.length} onClick={()=>setSelected(new Set())}>Clear selection</button><span>{selection.length} selected</span><button disabled={locked} onClick={()=>{commit(sources.flatMap(entriesFor));setSelected(new Set());}}>Reset pages</button></div>}
    {!entries.length&&<p className="task-note">No pages remain. Reset pages or add a file.</p>}
    {editable&&entries.length>1000&&<p className="error" role="alert">Select at most 1,000 output pages.</p>}
    <div className="page-grid">{visible.map((entry,index)=>{
      const position=currentBatch*12+index;const source=sources.find(v=>v.id===entry.sourceID);const thumbnail=thumbnails.get(thumbnailKey(entry));
      return <article key={entry.key} className={'page-card '+(selected.has(entry.key)?'selected':'')} draggable={editable&&!locked}
        onDragStart={event=>{event.dataTransfer.setData('application/x-fileform-page',entry.key);event.dataTransfer.effectAllowed='move';}}
        onDragOver={event=>{if(editable&&!locked&&Array.from(event.dataTransfer.types).includes('application/x-fileform-page'))event.preventDefault();}}
        onDrop={event=>{const key=event.dataTransfer.getData('application/x-fileform-page');if(key&&editable&&!locked){event.preventDefault();event.stopPropagation();drop(key,entry.key);}}}>
        <div className={'page-visual rotate-'+entry.rotation}>{thumbnail?<img src={thumbnail.dataURL} draggable={false} alt={'Page '+(entry.pageIndex+1)+' of '+source?.name}/>:source?.image?.preview?<ImageThumbnail preview={source.image.preview}/>:<span className="page-placeholder">{preparing?'Preparing…':'Page '+(entry.pageIndex+1)}</span>}</div>
        {editable?<label className="page-label"><input type="checkbox" aria-label={'Select output page '+(position+1)} disabled={locked} checked={selected.has(entry.key)} onChange={event=>setSelected(previous=>{const next=new Set(previous);event.target.checked?next.add(entry.key):next.delete(entry.key);return next;})}/><strong>{position+1}</strong><span>{source?.name} · {entry.pageIndex+1}</span></label>:<div className="page-label"><strong>{position+1}</strong><span>{source?.name}</span></div>}
        {!editable&&position<entries.length-1&&<label className="split-marker"><input type="checkbox" aria-label={'Split after page '+(position+1)} disabled={locked} checked={markers.includes(position+1)} onChange={event=>onMarkersChange?.(event.target.checked?[...markers,position+1].sort((a,b)=>a-b):markers.filter(v=>v!==position+1))}/>Split after this page</label>}
      </article>;
    })}</div>
    <div className="page-pagination"><span>{entries.length?currentBatch*12+1:0}–{Math.min((currentBatch+1)*12,entries.length)} of {entries.length}</span><button disabled={locked||currentBatch===0} onClick={()=>setBatch(currentBatch-1)}>Previous pages</button><button disabled={locked||currentBatch===lastBatch} onClick={()=>setBatch(currentBatch+1)}>Next pages</button></div>
    {preparing&&<div className="preview-status" role="status"><span>Preparing page previews…</span><button onClick={()=>void window.fileform.cancel()}>Cancel</button></div>}
    {error&&<div className="preview-error"><p className="error" role="alert">{error}</p><button disabled={locked} onClick={()=>setRetry(v=>v+1)}>Retry previews</button></div>}
  </section>;
}
