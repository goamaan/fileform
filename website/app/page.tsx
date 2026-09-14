import {SiteHeader,SiteFooter} from '../components/site-chrome';
import {WorkspacePreview} from '../components/workspace-preview';
import {Image,Files,AudioLines,Table2,ArrowUpRight,ArrowRight,Check,FolderOpen,Terminal,ShieldCheck,Monitor} from 'lucide-react';

const tools=[
  {title:'Images',text:'Convert, crop, resize and fit a size limit.',icon:Image,from:'PNG',to:'JPEG',detail:'Choose the format. Keep the detail.'},
  {title:'PDFs',text:'Arrange pages, split, compress and extract content.',icon:Files,from:'01 02 03 04',to:'01 02',detail:'Your pages, in the right order.'},
  {title:'Audio & video',text:'Convert recordings, extract audio and trim clips.',icon:AudioLines,from:'00:12',to:'00:48',detail:'Keep the part that matters.'},
  {title:'Tables & text',text:'Convert flat tables and recognize text in scans.',icon:Table2,from:'CSV',to:'JSON',detail:'Make your data easier to use.'},
];

export default function Home(){return <><SiteHeader/><main id="main-content">
  <section className="hero"><h1>Fileform</h1><p className="tagline">The file you have. The file you need.</p><p className="hero-description">Convert, crop, combine and trim. All on your desktop.</p><div className="platforms"><a href="/download#macos">macOS</a><a href="/download#windows">Windows <span>in development</span></a></div><a className="download-action" href="/download">Get Fileform <span aria-hidden="true">↓</span></a><a className="source-link" href="https://github.com/goamaan/fileform">Free and open source</a></section>
  <WorkspacePreview/>
  <section className="capabilities" aria-labelledby="tools-heading"><div className="section-top"><h2 id="tools-heading">A little less file juggling.</h2><a className="text-link" href="/formats">Formats and limits <ArrowUpRight size={15} aria-hidden="true"/></a></div>
    <div className="capability-grid">{tools.map(({title,text,icon:Icon,from,to},i)=><article className={`capability-card capability-${i}`} key={title}>
      <div className="capability-heading"><Icon size={21} aria-hidden="true"/><h3>{title}</h3></div><p>{text}</p>
      <div className="format-example" aria-label={`Example: ${from} to ${to}`}><span>{from}</span>{i===2?<div className="waveform" aria-hidden="true">{[20,36,56,32,65,44,24,52,72,42,28,58,38,20,48,30].map((height,n)=><i key={n} style={{height:`${height}%`}}/>)}</div>:<ArrowRight size={18} aria-hidden="true"/>}<span>{to}</span></div>
    </article>)}</div>
    <p className="support-note">Shown above: the macOS app. The Electron preview for macOS and Windows currently converts CSV and TSV to JSON. <a href="/releases">Follow the migration ↗</a></p>
  </section>
  <section className="workflow-section" aria-labelledby="workflow-heading"><h2 id="workflow-heading">Drop. Choose. Save.</h2><ol className="workflow-grid">
    <li><div className="workflow-widget file-widget"><FolderOpen size={24} aria-hidden="true"/><span>cover.png<small>Original file</small></span></div><h3><span>01</span> Add your files</h3></li>
    <li><div className="workflow-widget options-widget"><span>Format <strong>JPEG</strong></span><span>Size limit <strong>10 MB</strong></span></div><h3><span>02</span> Set the result</h3></li>
    <li><div className="workflow-widget saved-widget"><Check size={21} aria-hidden="true"/><span>cover-converted.jpg<small>Example output</small></span></div><h3><span>03</span> Save a new file</h3></li>
  </ol><div className="file-principles"><span><ShieldCheck size={17} aria-hidden="true"/> Originals stay untouched</span><span><Monitor size={17} aria-hidden="true"/> Local file processing</span></div></section>
  <section className="terminal-section" aria-labelledby="terminal-heading"><div><Terminal size={23} aria-hidden="true"/><h2 id="terminal-heading">At home in your terminal.</h2><p>Inspect files and run conversions from a script with the open-source CLI.</p><a className="text-link" href="/docs">Read the CLI guide <ArrowRight size={15} aria-hidden="true"/></a></div><div className="terminal-example"><div className="terminal-title"><span>fileform-native</span><span>macOS / Windows preview</span></div><pre><code><span className="terminal-comment"># Convert a local table</span>{'\n'}<span className="terminal-prompt">$ </span>fileform-native convert-table data.csv data.json</code></pre></div></section>
  <section className="community"><h2>Yours to use. Yours to improve.</h2><p>One free project for the app, engine and CLI.</p><div className="links"><a href="https://github.com/goamaan/fileform">View source ↗</a><a href="https://github.com/goamaan/fileform/blob/main/CONTRIBUTING.md">Contribute</a><a href="/download">Get Fileform</a></div></section>
</main><SiteFooter/></>}
