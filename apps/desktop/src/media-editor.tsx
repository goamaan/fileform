// SPDX-License-Identifier: Apache-2.0
import {memo,useEffect,useRef,useState} from 'react';
import type {AssetSource,MediaPreview,TaskOptions} from './contracts';

const seconds=(value:number)=>String(Math.floor(Math.max(0,value)*1e9)/1e9);
const clock=(value:number)=>{
 value=Number.isFinite(value)?Math.max(0,value):0;
 const minutes=Math.floor(value/60);return minutes+':'+(value-minutes*60).toFixed(2).padStart(5,'0');
};
const Waveform=memo(function Waveform({waveform,duration}:{waveform:NonNullable<MediaPreview['waveform']>;duration:number}){
 return <g>{waveform.channels.map((channel,row)=><g key={row}>{channel.minimum.map((low,index)=>{
  const x=index*waveform.samplesPerBucket/waveform.sampleRate/duration*1000;if(x>=1000)return null;
  const width=Math.max(.5,waveform.samplesPerBucket/waveform.sampleRate/duration*1000);
  const high=Math.min(1,Math.max(-1,channel.maximum[index]));low=Math.min(1,Math.max(-1,low));
  return <rect className="waveform-bar" key={index} x={x} y={row*40+20-high*18} width={width} height={Math.max(.5,(high-low)*18)}/>;
 })}</g>)}</g>;
});
export function MediaEditor({source,audioOnly,trim,options,disabled,onChange,onBusyChange}:{
 source:AssetSource;audioOnly:boolean;trim:boolean;options:TaskOptions;disabled:boolean;
 onChange:(value:Partial<TaskOptions>)=>void;onBusyChange:(value:boolean)=>void;
}){
 const [preview,setPreview]=useState<MediaPreview|null>(null);const [preparing,setPreparing]=useState(false);
 const [error,setError]=useState('');const [playing,setPlaying]=useState(false);const [time,setTime]=useState(0);const [volume,setVolume]=useState(.5);
 const media=useRef<HTMLMediaElement|null>(null);const lease=useRef<string|null>(null);const mounted=useRef(true);
 const duration=preview?preview.duration.ticks/preview.duration.timescale:source.duration??0;
 const limit=(value:string|undefined,fallback:number)=>{const number=Number(value);return value!==undefined&&Number.isFinite(number)?Math.max(0,Math.min(duration,number)):fallback;};
 const start=limit(options.start,0);const end=limit(options.end,duration);const step=Math.min(.001,duration/1000);
 const hasChoice=(source.audioTracks?.length??0)>1&&!options.muteAudio&&options.audioStream===undefined;
 useEffect(()=>()=>{mounted.current=false;media.current?.pause();if(lease.current)void window.fileform.releaseMediaPreview(lease.current);},[]);
 useEffect(()=>{
  media.current?.pause();setPlaying(false);setTime(0);setPreview(null);setError('');
  if(lease.current){void window.fileform.releaseMediaPreview(lease.current);lease.current=null;}
 },[source.id,audioOnly,options.audioStream,options.muteAudio]);
 useEffect(()=>{if(media.current)media.current.volume=volume;},[volume,preview]);
 useEffect(()=>{
  if(!playing||!trim)return;let frame=0;
  const tick=()=>{const player=media.current;if(!player)return;setTime(Math.min(player.currentTime,duration));if(player.currentTime>=end)player.pause();else frame=requestAnimationFrame(tick);};
  frame=requestAnimationFrame(tick);return()=>cancelAnimationFrame(frame);
 },[playing,trim,end,duration]);
 useEffect(()=>{
  if(preview&&trim&&options.end===String(source.duration))onChange({end:seconds(duration)});
 },[preview,trim]);
 async function prepare(){
  setPreparing(true);onBusyChange(true);setError('');
  try {
   const result=await window.fileform.previewMedia(source.id,{audioOnly,audioStream:options.audioStream,muteAudio:options.muteAudio});
   if(!mounted.current){await window.fileform.releaseMediaPreview(result.id);return;}
   lease.current=result.id;setPreview(result);setTime(0);
  }catch(e){if(mounted.current)setError(e instanceof Error?e.message.replace(/^Error invoking remote method '[^']+': Error: /,''):'Playback could not be prepared.');}
  finally {if(mounted.current)setPreparing(false);onBusyChange(false);}
 }
 async function play(){
  const player=media.current;if(!player)return;
  if(!player.paused){player.pause();return;}
  if(trim&&(player.currentTime<start||player.currentTime>=end)||player.currentTime>=duration)player.currentTime=trim?start:0;
  try{await player.play();}catch{setError('Playback could not start.');}
 }
 const waveform=preview?.waveform;const height=(waveform?.channels.length??1)*40;
 const mediaEvents={onPlay:()=>setPlaying(true),onPause:()=>setPlaying(false),onEnded:()=>setPlaying(false),onError:()=>setError('Playback could not be loaded.'),onTimeUpdate:()=>{const player=media.current;if(player){setTime(Math.min(player.currentTime,duration));if(trim&&player.currentTime>=end)player.pause();}}};
 return <section className="media-editor" aria-label={audioOnly?'Audio editor':'Video editor'}>
  <div className={'media-stage '+(audioOnly?'audio-stage':'')}>{preview?
   preview.kind==='video'?<video ref={element=>{media.current=element;}} src={preview.url} poster={preview.poster} preload="metadata" playsInline aria-hidden="true" {...mediaEvents}/>:<><audio ref={element=>{media.current=element;}} src={preview.url} preload="metadata" aria-hidden="true" {...mediaEvents}/><div className="audio-identity"><span aria-hidden="true">♫</span><strong>{source.name}</strong><span>{clock(duration)}</span></div></>:
   <div className="media-placeholder"><span className="media-symbol" aria-hidden="true">{audioOnly?'♫':'▷'}</span><h2>{audioOnly?'Audio playback':'Video playback'}</h2><button className="primary" disabled={disabled||preparing||hasChoice} onClick={prepare}>{preparing?'Preparing playback…':'Load playback'}</button>{hasChoice&&<p>Choose an audio track below.</p>}{preparing&&<button onClick={()=>void window.fileform.cancel()}>Cancel</button>}</div>}
  </div>
  {preview&&<>
   <div className="playback-controls"><button onClick={play} aria-label={playing?'Pause preview':'Play preview'}>{playing?'Pause':'Play'}</button><input aria-label="Playback position" type="range" min="0" max={duration} step={step} value={time} onChange={event=>{const value=Number(event.target.value);if(media.current)media.current.currentTime=value;setTime(value);}}/><output>{clock(time)} / {clock(duration)}</output><label className="volume-control">Volume<input aria-label="Playback volume" type="range" min="0" max="1" step=".01" value={volume} onChange={event=>setVolume(Number(event.target.value))}/></label></div>
   <div className="media-timeline"><svg viewBox={'0 0 1000 '+height} role="img" aria-label={waveform?'Waveform of '+waveform.channels.length+' channels':'Recording timeline'} preserveAspectRatio="none">
    {waveform&&<Waveform waveform={waveform} duration={duration}/>}
    {trim&&<><rect className="trim-shade" x="0" y="0" width={start/duration*1000} height={height}/><rect className="trim-shade" x={end/duration*1000} y="0" width={(duration-end)/duration*1000} height={height}/><line className="trim-line" x1={start/duration*1000} x2={start/duration*1000} y1="0" y2={height}/><line className="trim-line" x1={end/duration*1000} x2={end/duration*1000} y1="0" y2={height}/></>}
    <line className="playhead-line" x1={time/duration*1000} x2={time/duration*1000} y1="0" y2={height}/>
   </svg>{trim&&<div className="trim-handles"><input aria-label="Visual trim start" type="range" min="0" max={duration} step={step} value={start} disabled={disabled} onChange={event=>onChange({start:seconds(Math.min(Number(event.target.value),Math.max(0,end-step)))})}/><input aria-label="Visual trim end" type="range" min="0" max={duration} step={step} value={end} disabled={disabled} onChange={event=>onChange({end:seconds(Math.max(Number(event.target.value),Math.min(duration,start+step)))})}/></div>}</div>
   <div className="timeline-labels"><span>{trim?clock(start):'0:00.00'}</span><span>{trim?clock(end-start)+' selected':'Full recording'}</span><span>{clock(end)}</span></div>
   {trim&&<div className="trim-actions"><button disabled={disabled} onClick={()=>onChange({start:seconds(Math.min(time,Math.max(0,end-step)))})}>Set start here</button><button disabled={disabled} onClick={()=>onChange({end:seconds(Math.max(time,Math.min(duration,start+step)))})}>Set end here</button><button disabled={disabled} onClick={()=>onChange({start:'0',end:seconds(duration)})}>Reset range</button></div>}
   <p className="preview-note">Preview only. Exports use the original file.</p>
  </>}
  {error&&<div className="preview-error"><p className="error" role="alert">{error}</p>{preview&&<button disabled={disabled} onClick={()=>{media.current?.pause();if(lease.current)void window.fileform.releaseMediaPreview(lease.current);lease.current=null;setPreview(null);setError('');}}>Reload playback</button>}</div>}
 </section>;
}
