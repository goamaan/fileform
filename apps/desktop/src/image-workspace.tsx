import {useState,useEffect,useRef} from 'react';
import type {ImageSource,ImageSavedFile,ImagePreview} from './contracts';

function Preview({preview,name}:{preview:ImagePreview;name:string}) {
  const canvas=useRef<HTMLCanvasElement>(null);
  useEffect(()=>{canvas.current?.getContext('2d')?.putImageData(new ImageData(new Uint8ClampedArray(preview.rgba),preview.width,preview.height),0,0);},[preview]);
  return <figure className="image-preview"><canvas ref={canvas} width={preview.width} height={preview.height} role="img" aria-label={`Normalized preview of ${name}`}/></figure>;
}

export function ImageWorkspace({onBusyChange,hidden}:{onBusyChange:(busy:boolean)=>void;hidden:boolean}){
  const [source,setSource]=useState<ImageSource|null>(null);
  const [saved,setSaved]=useState<ImageSavedFile|null>(null);
  const [busy,setBusy]=useState(false);
  const [cancelling,setCancelling]=useState(false);
  const [error,setError]=useState('');
  async function run(action:()=>Promise<void>){
    setBusy(true);onBusyChange(true);setError('');
    try{await action();}catch(error){setError(error instanceof Error?error.message.replace(/^Error invoking remote method '[^']+': Error: /,''):'The image could not be processed.');}
    finally{setBusy(false);setCancelling(false);onBusyChange(false);}
  }
  const choose=()=>run(async()=>{const file=await window.fileform.chooseImage();if(file){setSource(file);setSaved(null);}});
  return <section className="workspace" hidden={hidden} aria-label="PNG image workspace">
    <div className="heading"><h1>Prepare a PNG</h1><span className="format">Orientation · sRGB · Transparency</span></div>
    {!source?<button className="dropzone" disabled={busy} onClick={choose}><span className="file-icon" aria-hidden="true">▧</span><strong>{busy?'Reading image…':'Choose a PNG'}</strong><span>Still images · up to 80 megapixels</span></button>:<article className="file"><span className="file-icon" aria-hidden="true">▧</span><div><h2>{source.name}</h2><p>{source.width.toLocaleString()} × {source.height.toLocaleString()} pixels · {source.hasAlpha?'With transparency':'Opaque'}</p></div><button disabled={busy} onClick={choose}>Change</button></article>}
    {source?.preview&&<Preview preview={source.preview} name={source.name}/>}
    {source&&<p className="conversion-note">{source.canConvert?'Save a new sRGB PNG with the correct orientation. Transparency is retained.':'This image needs extended-color support that is still being implemented.'}</p>}
    {busy&&<button disabled={cancelling} onClick={()=>{setCancelling(true);void window.fileform.cancel().catch(()=>{setCancelling(false);setError('Cancellation could not be requested.');});}}>{cancelling?'Cancelling…':'Cancel'}</button>}
    {error&&<p className="error" role="alert">{error}</p>}
    {source&&<div className="actions"><p>Originals stay untouched.</p><button className="primary" disabled={busy||!source.canConvert} onClick={()=>run(async()=>{const result=await window.fileform.saveImage(source.id);if(result)setSaved(result);})}>{busy?'Processing…':'Save PNG…'}</button></div>}
    {saved&&<article className="result" role="status"><span aria-hidden="true">✓</span><div><h2>{saved.name}</h2><p>{saved.width.toLocaleString()} × {saved.height.toLocaleString()} pixels saved</p></div><button disabled={busy} onClick={()=>run(()=>window.fileform.reveal(saved.id))}>Show in folder</button></article>}
  </section>;
}
