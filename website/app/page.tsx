import {SiteHeader,SiteFooter} from '../components/site-chrome';
const tools=[
  ['Images','Convert, crop, resize and fit a size limit.'],
  ['PDFs','Arrange pages, split, compress and extract content.'],
  ['Audio & video','Convert recordings, extract audio and trim clips.'],
  ['Tables & text','Convert flat tables and recognize text in scans.'],
];
export default function Home(){return <><SiteHeader/><main id="main-content">
  <section className="hero"><h1>Fileform</h1><p className="tagline">Local file tools for your desktop.</p><div className="platforms"><a href="/download#macos">macOS</a><a href="/download#windows">Windows <span>in development</span></a></div><a className="download-action" href="/download">Get Fileform <span aria-hidden="true">↓</span></a><a className="source-link" href="https://github.com/goamaan/fileform">Free and open source</a></section>
  <figure className="hero-preview"><img src="/app-workspace.png" width="1224" height="768" alt="The macOS reference app arranging four pages into two PDFs."/><figcaption>Current macOS app</figcaption></figure>
  <section className="tools-section" aria-label="File tools"><div className="tools-grid">{tools.map(([title,text])=><article key={title}><h2>{title}</h2><p>{text}</p></article>)}</div><p className="support-note">These tools are available in the macOS reference app. The cross-platform preview currently converts tables.</p><a className="text-link" href="/formats">Formats and limits</a></section>
  <section className="community"><h2>Built in the open.</h2><p>Help shape Fileform with code, bug reports and platform testing.</p><div className="links"><a href="https://github.com/goamaan/fileform">GitHub</a><a href="https://github.com/goamaan/fileform/blob/main/CONTRIBUTING.md">Contribute</a><a href="/docs">Documentation</a></div></section>
</main><SiteFooter/></>}
