import React,{useEffect,useState} from 'react';
import {createRoot} from 'react-dom/client';
import type {Appearance,SourceFile,SavedFile,TableOutput} from './contracts';
import './style.css';
function App(){
  const [source,setSource]=useState<SourceFile|null>(null);
  const [saved,setSaved]=useState<SavedFile|null>(null);
  const [format,setFormat]=useState<TableOutput>('json');
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [appearance,setAppearance]=useState<Appearance>('system');
  const applyTheme=async(mode?:Appearance)=>{const state=await window.fileform.appearance(mode);setAppearance(state.mode);document.documentElement.dataset.theme=state.dark?'dark':'light';};
  useEffect(()=>{void applyTheme().catch(()=>setError('Appearance could not be loaded.'));const query=matchMedia('(prefers-color-scheme: dark)');const changed=()=>void applyTheme().catch(()=>{});query.addEventListener('change',changed);return()=>query.removeEventListener('change',changed);},[]);
  async function run(action:()=>Promise<void>){setBusy(true);setError('');try{await action();}catch(error){setError(error instanceof Error?error.message.replace(/^Error invoking remote method '[^']+': Error: /,''):'The file could not be processed.');}finally{setBusy(false);}}
  const choose=()=>run(async()=>{const file=await window.fileform.chooseTable();if(file){setSource(file);setSaved(null);}});
  return <main>
    <header><div className="brand"><span aria-hidden="true">▤</span>Fileform</div><label>Appearance<select aria-label="Appearance" value={appearance} onChange={e=>void applyTheme(e.target.value as Appearance).catch(()=>setError('Appearance could not be saved.'))}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label></header>
    <section className="workspace">
      <div className="heading"><h1>Convert a table</h1><span className="format">CSV / TSV → JSON, CSV, TSV</span></div>
      {!source?<button className="dropzone" disabled={busy} onClick={choose}><span className="file-icon" aria-hidden="true">▤</span><strong>{busy?'Reading table…':'Choose a table'}</strong><span>Up to 8 MiB</span></button>:<article className="file"><span className="file-icon" aria-hidden="true">▤</span><div><h2>{source.name}</h2><p>{source.rows.toLocaleString()} rows · {source.columns} columns · {new Intl.NumberFormat(undefined,{style:'unit',unit:'kilobyte',maximumFractionDigits:1}).format(source.bytes/1000)}</p></div><button disabled={busy} onClick={choose}>Change</button></article>}
      {error&&<p className="error" role="alert">{error}</p>}
      {source&&<div className="actions"><label>Output format<select aria-label="Output format" disabled={busy} value={format} onChange={e=>setFormat(e.target.value as TableOutput)}><option value="json">JSON</option><option value="csv">CSV</option><option value="tsv">TSV</option></select></label><button className="primary" disabled={busy} onClick={()=>run(async()=>{const result=await window.fileform.saveTable(source.id,format);if(result)setSaved(result);})}>{busy?'Processing…':'Save '+format.toUpperCase()+'…'}</button></div>}
      {saved&&<article className="result" role="status"><span aria-hidden="true">✓</span><div><h2>{saved.name}</h2><p>{saved.rows.toLocaleString()} rows saved</p></div><button onClick={()=>run(()=>window.fileform.reveal(saved.id))}>Show in folder</button></article>}
    </section>
    <footer><span>Free and open source</span><span>Desktop migration preview</span></footer>
  </main>;
}
createRoot(document.getElementById('root')!).render(<App/>);
