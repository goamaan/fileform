import {useState,useEffect,useRef} from 'react';
import type {ImageSource,ImageSavedFile,ImagePreview,PixelCrop} from './contracts';

import {outputDimensions,parseByteLimit,type ByteUnit} from './image-export';
import {resizeCrop,type Corner} from './crop-geometry';
function Preview({preview,name,background,width,height,crop,disabled,onDraft,onCommit,onCancel}:{preview:ImagePreview;name:string;background?:'white'|'black';width:number;height:number;crop:PixelCrop|null;disabled:boolean;onDraft:(value:PixelCrop|null)=>void;onCommit:(value:PixelCrop|null,previous?:PixelCrop|null)=>void;onCancel:(value:PixelCrop,committed:boolean)=>void}) {
  const canvas=useRef<HTMLCanvasElement>(null);
  const stage=useRef<HTMLDivElement>(null);
  const drag=useRef<{start:PixelCrop;draft:PixelCrop;corner:Corner;x:number;y:number;committed:boolean}|null>(null);
  useEffect(()=>{canvas.current?.getContext('2d')?.putImageData(new ImageData(new Uint8ClampedArray(preview.rgba),preview.width,preview.height),0,0);},[preview]);
  return <figure className="image-preview"><div ref={stage} className="image-stage" style={{width:Math.min(320,320*width/height),aspectRatio:`${width}/${height}`}}><canvas ref={canvas} width={preview.width} height={preview.height} style={background?{backgroundColor:background,backgroundImage:'none'}:undefined} role="img" aria-label={`Normalized preview of ${name}`}/>{crop&&<div className="crop-selection" style={{left:`${crop.x/width*100}%`,top:`${crop.y/height*100}%`,width:`${crop.width/width*100}%`,height:`${crop.height/height*100}%`}}>{(['nw','ne','sw','se'] as Corner[]).map(corner=><button key={corner} type="button" disabled={disabled} className={`crop-handle ${corner}`} aria-label={`Crop ${corner.startsWith('n')?'top':'bottom'} ${corner.endsWith('w')?'left':'right'} corner`} onPointerDown={event=>{event.preventDefault();event.currentTarget.focus();event.currentTarget.setPointerCapture(event.pointerId);drag.current={start:crop,draft:crop,corner,x:event.clientX,y:event.clientY,committed:false};}} onPointerMove={event=>{const current=drag.current;const box=stage.current?.getBoundingClientRect();if(disabled||!current||!box||box.width<=0||box.height<=0)return;const next=resizeCrop(current.start,current.corner,(event.clientX-current.x)/box.width*width,(event.clientY-current.y)/box.height*height,width,height);current.draft=next;if(!current.committed&&JSON.stringify(next)!==JSON.stringify(current.start)){current.committed=true;onCommit(next,current.start);}else{onDraft(next);}}} onPointerUp={event=>{const current=drag.current;if(!current)return;drag.current=null;if(event.currentTarget.hasPointerCapture(event.pointerId))event.currentTarget.releasePointerCapture(event.pointerId);}} onLostPointerCapture={()=>{drag.current=null;}} onPointerCancel={()=>{if(drag.current)onCancel(drag.current.start,drag.current.committed);drag.current=null;}} onKeyDown={event=>{const amount=event.shiftKey?10:1;const delta=event.key==='ArrowLeft'?[-amount,0]:event.key==='ArrowRight'?[amount,0]:event.key==='ArrowUp'?[0,-amount]:event.key==='ArrowDown'?[0,amount]:null;if(delta){event.preventDefault();onCommit(resizeCrop(crop,corner,delta[0],delta[1],width,height));}}}/>)}</div>}</div></figure>;
}

export function ImageWorkspace({onBusyChange,hidden}:{onBusyChange:(busy:boolean)=>void;hidden:boolean}){
  const [source,setSource]=useState<ImageSource|null>(null);
  const [crop,setCrop]=useState<PixelCrop|null>(null);
  const [past,setPast]=useState<(PixelCrop|null)[]>([]);
  const [future,setFuture]=useState<(PixelCrop|null)[]>([]);
  const dragFuture=useRef<(PixelCrop|null)[]>([]);
  const commitCrop=(value:PixelCrop|null,previous:PixelCrop|null=crop)=>{if(JSON.stringify(value)===JSON.stringify(previous))return;setPast(items=>[...items.slice(-49),previous]);setFuture([]);setCrop(value);};
  const [saved,setSaved]=useState<ImageSavedFile|null>(null);
  const [format,setFormat]=useState<'png'|'jpeg'|'tiff'>('png');
  const [resizeEnabled,setResizeEnabled]=useState(false);
  const [maximum,setMaximum]=useState('1280');
  const invalidMaximum=resizeEnabled&&(!Number.isSafeInteger(Number(maximum))||Number(maximum)<1||Number(maximum)>4294967295);
  const size=source?outputDimensions(crop?.width??source.width,crop?.height??source.height,resizeEnabled&&!invalidMaximum?Number(maximum):undefined):null;
  const [quality,setQuality]=useState(85);
  const [fitEnabled,setFitEnabled]=useState(false);
  const [limitText,setLimitText]=useState('1');
  const [limitUnit,setLimitUnit]=useState<ByteUnit>('MB');
  const [minimumQuality,setMinimumQuality]=useState(35);
  const byteLimit=fitEnabled?parseByteLimit(limitText,limitUnit):undefined;
  const invalidFit=fitEnabled&&(byteLimit===undefined||(format==='jpeg'&&minimumQuality>quality));
  const [background,setBackground]=useState<''|'white'|'black'>('');
  const [busy,setBusy]=useState(false);
  const [cancelling,setCancelling]=useState(false);
  const [error,setError]=useState('');
  async function run(action:()=>Promise<void>){
    setBusy(true);onBusyChange(true);setError('');
    try{await action();}catch(error){setError(error instanceof Error?error.message.replace(/^Error invoking remote method '[^']+': Error: /,''):'The image could not be processed.');}
    finally{setBusy(false);setCancelling(false);onBusyChange(false);}
  }
  const choose=()=>run(async()=>{const file=await window.fileform.chooseImage();if(file){setSource(file);setSaved(null);setCrop(null);setPast([]);setFuture([]);}});
  return <section className={`workspace image-workspace ${source?'has-source':''}`} hidden={hidden} aria-label="Image workspace">
    <div className="heading"><h1>Images</h1><span className="format">PNG / JPEG / TIFF</span></div>
    {!source?<button className="dropzone" disabled={busy} onClick={choose}><span className="file-icon" aria-hidden="true">▧</span><strong>{busy?'Reading image…':'Choose an image'}</strong><span>PNG / JPEG / TIFF · up to 80 megapixels</span></button>:<article className="file"><span className="file-icon" aria-hidden="true">▧</span><div><h2>{source.name}</h2><p>{source.width.toLocaleString()} × {source.height.toLocaleString()} pixels · {source.hasAlpha?'With transparency':'Opaque'}</p></div><button disabled={busy} onClick={choose}>Change</button></article>}
    <div className="preview-panel" hidden={!source}>
    {source?.preview&&<Preview preview={source.preview} name={source.name} width={source.width} height={source.height} crop={crop} disabled={busy} onDraft={setCrop} onCommit={(value,previous)=>{if(previous!==undefined)dragFuture.current=future;commitCrop(value,previous);}} onCancel={(value,committed)=>{setCrop(value);if(committed){setPast(items=>items.slice(0,-1));setFuture(dragFuture.current);}}} background={format==='jpeg'&&background?background:undefined}/>}
    {source?.canConvert&&<div className="crop-tools"><button disabled={busy} onClick={()=>commitCrop(crop?null:{x:0,y:0,width:source.width,height:source.height})}>{crop?'Remove crop':'Crop'}</button><button disabled={busy} onClick={()=>{const size=Math.min(source.width,source.height);commitCrop({x:Math.floor((source.width-size)/2),y:Math.floor((source.height-size)/2),width:size,height:size});}}>1:1</button><button disabled={busy||past.length===0} onClick={()=>{setFuture(values=>[crop,...values]);setCrop(past[past.length-1]);setPast(values=>values.slice(0,-1));}}>Undo crop</button><button disabled={busy||future.length===0} onClick={()=>{setPast(values=>[...values,crop]);setCrop(future[0]);setFuture(values=>values.slice(1));}}>Redo crop</button></div>}
    {source&&crop&&<div className="crop-fields">{(['x','y','width','height'] as const).map(field=><label key={field}>{field==='x'?'X':field==='y'?'Y':field==='width'?'Width':'Height'}<input aria-label={`Crop ${field}`} type="number" min={field==='x'||field==='y'?0:1} step="1" disabled={busy} value={crop[field]} onChange={event=>{const number=Number(event.target.value);if(!Number.isFinite(number))return;const maximum=field==='x'?source.width-crop.width:field==='y'?source.height-crop.height:field==='width'?source.width-crop.x:source.height-crop.y;commitCrop({...crop,[field]:Math.max(field==='x'||field==='y'?0:1,Math.min(maximum,Math.round(number)))});}}/></label>)}</div>}
    </div><div className="export-panel" hidden={!source}><h2 className="panel-title">Export settings</h2>
    {source&&<p className="conversion-note">{source.canConvert?(format==='jpeg'?'JPEG is lossy and does not support transparency.':format==='tiff'?'Save a lossless sRGB TIFF.':'Save a new sRGB PNG. Transparency is retained.'):'This image needs preservation support that is still being implemented.'}</p>}
    {source&&<div className="image-options"><label>Format<select disabled={busy} aria-label="Image output format" value={format} onChange={e=>setFormat(e.target.value as 'png'|'jpeg'|'tiff')}><option value="png">PNG</option><option value="jpeg">JPEG</option><option value="tiff">TIFF</option></select></label>{format==='jpeg'&&<label>{fitEnabled?'Maximum quality':'Quality'}<input aria-label="JPEG quality" type="range" min="1" max="100" step="1" disabled={busy} value={quality} onChange={e=>setQuality(Number(e.target.value))}/><output>{quality}</output></label>}{format==='jpeg'&&source.hasAlpha&&<label>Background<select disabled={busy} aria-label="JPEG background" value={background} onChange={e=>setBackground(e.target.value as ''|'white'|'black')}><option value="">Choose…</option><option value="white">White</option><option value="black">Black</option></select></label>}</div>}
    {source&&<div className="resize-options"><label><input type="checkbox" aria-label="Resize image" disabled={busy} checked={resizeEnabled} onChange={e=>setResizeEnabled(e.target.checked)}/>Resize</label>{resizeEnabled&&<label>Longest edge<input type="number" aria-label="Maximum image dimension" min="1" step="1" disabled={busy} value={maximum} onChange={e=>setMaximum(e.target.value)}/>px</label>}<span>{size?.width.toLocaleString()} × {size?.height.toLocaleString()} output</span></div>}
    {source&&<div className="fit-options"><label><input type="checkbox" aria-label="Fit file size" disabled={busy} checked={fitEnabled} onChange={e=>setFitEnabled(e.target.checked)}/>Fit file size</label>{fitEnabled&&<><label>Limit<input aria-label="File-size limit" inputMode="decimal" maxLength={32} disabled={busy} value={limitText} onChange={e=>setLimitText(e.target.value)}/></label><select aria-label="File-size unit" disabled={busy} value={limitUnit} onChange={e=>setLimitUnit(e.target.value as ByteUnit)}><option value="B">B</option><option value="KB">KB</option><option value="MB">MB</option></select>{format==='jpeg'&&<label>Minimum quality<input aria-label="Minimum JPEG quality" type="range" min="1" max="100" disabled={busy} value={minimumQuality} onChange={e=>setMinimumQuality(Number(e.target.value))}/><output>{minimumQuality}</output></label>}</>}</div>}
    {invalidFit&&<p className="error" role="alert">{byteLimit===undefined?'Enter a limit from 1 byte to 512 MiB.':'Minimum quality must not exceed maximum quality.'}</p>}
    {invalidMaximum&&<p className="error" role="alert">Enter a positive whole number of pixels.</p>}
    {busy&&<button disabled={cancelling} onClick={()=>{setCancelling(true);void window.fileform.cancel().catch(()=>{setCancelling(false);setError('Cancellation could not be requested.');});}}>{cancelling?'Cancelling…':'Cancel'}</button>}
    {error&&<p className="error" role="alert">{error}</p>}
    {source&&<div className="actions"><p>Originals stay untouched.</p><button className="primary" disabled={busy||invalidMaximum||invalidFit||!source.canConvert||(format==='jpeg'&&source.hasAlpha&&!background)} onClick={()=>run(async()=>{const result=await window.fileform.saveImage(source.id,{format,background:format==='jpeg'&&background?background:undefined,quality:format==='jpeg'?quality:undefined,crop:crop??undefined,maxDimension:resizeEnabled?Number(maximum):undefined,maxBytes:fitEnabled?byteLimit:undefined,minimumQuality:fitEnabled&&format==='jpeg'?minimumQuality:undefined});if(result)setSaved(result);})}>{busy?'Processing…':'Save '+format.toUpperCase()+'…'}</button></div>}
    </div>
    {saved&&<article className="result" role="status"><span aria-hidden="true">✓</span><div><h2>{saved.name}</h2><p>{saved.width.toLocaleString()} × {saved.height.toLocaleString()} pixels · {saved.bytes.toLocaleString()} bytes{saved.quality!==null?` · Quality ${saved.quality}`:''}</p></div><button disabled={busy} onClick={()=>run(()=>window.fileform.reveal(saved.id))}>Show in folder</button></article>}
  </section>;
}
