'use client';

import {useState, useRef} from 'react';
import {Files, Crop, AudioLines} from 'lucide-react';

const previews = [
  {label:'Arrange PDFs', icon:Files, image:'/app-workspace.png', alt:'The macOS app arranging four pages into two PDFs.', caption:'Reorder pages. Split a document. See what you’re saving.'},
  {label:'Crop images', icon:Crop, image:'/crop-workspace.png', alt:'An original image with a square crop beside the saved square JPEG result.', caption:'Frame your image with crop handles and aspect presets.'},
  {label:'Trim recordings', icon:AudioLines, image:'/trim-workspace.png', alt:'The macOS audio trimmer showing a waveform, selection and playback controls.', caption:'Choose a range and listen before you export.'},
];

export function WorkspacePreview() {
  const [selected, setSelected] = useState(0);
  const buttons = useRef<(HTMLButtonElement|null)[]>([]);
  return <section className="workspace-showcase" aria-label="Explore the macOS app">
    <div className="preview-tabs" role="tablist" aria-label="Editor previews">
      {previews.map(({label,icon:Icon},i)=><button key={label} ref={el=>{buttons.current[i]=el;}} id={`preview-tab-${i}`} type="button" role="tab" aria-selected={selected===i} aria-controls={`preview-panel-${i}`} tabIndex={selected===i?0:-1} onClick={()=>setSelected(i)} onKeyDown={e=>{
        const next=e.key==='ArrowRight'?(i+1)%previews.length:e.key==='ArrowLeft'?(i+previews.length-1)%previews.length:e.key==='Home'?0:e.key==='End'?previews.length-1:null;
        if(next!==null){e.preventDefault();setSelected(next);buttons.current[next]?.focus();}
      }}><Icon size={16} aria-hidden="true"/>{label}</button>)}
    </div>
    {previews.map((preview,i)=><figure key={preview.label} id={`preview-panel-${i}`} role="tabpanel" aria-labelledby={`preview-tab-${i}`} hidden={selected!==i} tabIndex={0}>
      <div className="preview-frame"><img src={preview.image} width="1224" height="768" loading={i===0?'eager':'lazy'} alt={preview.alt}/></div>
      <figcaption>{preview.caption}<span>Current macOS app</span></figcaption>
    </figure>)}
  </section>;
}
