#!/usr/bin/env python3
"""Verified full-recording playback exports, separate from source-based edits."""
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import wave
root=Path(__file__).resolve().parents[3];pack=Path(sys.argv[1]).resolve()
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix);worker=root/'target/release'/('fileform-worker'+suffix)
ffmpeg=pack/'bin'/('ffmpeg'+suffix)
def run(*args):return subprocess.run([str(v) for v in args],capture_output=True,timeout=240)
def good(*args):
    value=run(*args);assert value.returncode==0,value.stderr;return value

def execute(request,ok=True):
    value=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,timeout=240)
    reply=json.loads(value.stdout);assert reply['ok']==ok,reply;return reply

def pcm(path,index=0):return good(ffmpeg,'-v','error','-i',path,'-map',f'0:{index}','-c:a','pcm_s16le','-f','s16le','-').stdout
with tempfile.TemporaryDirectory(prefix='fileform-playback-') as folder:
    base=Path(folder);clip=root/'crates/fileform-engine/tests/fixtures/h264-aac.mp4'
    waves=[]
    for index,level in enumerate([8192,-16384]):
        source=base/f'channel-{index}.wav'
        with wave.open(str(source),'wb') as audio:
            audio.setparams((1,2,44100,0,'NONE','not compressed'));audio.writeframes(struct.pack('<h',level)*88200)
        waves.append(source)
    source=base/'selected-audio.mov'
    good(ffmpeg,'-v','error','-itsoffset','1','-i',waves[0],'-itsoffset','1','-i',waves[1],'-map','0:a:0','-map','1:a:0','-c:a','pcm_s16le',source)
    source_bytes=source.read_bytes()
    output=base/'audio-playback.wav'
    receipt=json.loads(good(cli,'playback-preview',source,output,pack,'--audio-stream',1).stdout)
    assert receipt['source_audio_stream_index']==1 and receipt['source_video_stream_index'] is None
    assert receipt['width'] is None and receipt['duration']['ticks']/receipt['duration']['timescale']==2
    assert receipt['source_sha256']==hashlib.sha256(source_bytes).hexdigest()
    assert receipt['sha256']==hashlib.sha256(output.read_bytes()).hexdigest()
    assert pcm(output)==pcm(source,1)
    assert json.loads(good(cli,'inspect-media',output,pack).stdout)['container_start_time'] is None # WAV starts at zero.
    original_output=output.read_bytes()
    assert run(cli,'playback-preview',source,output,pack,'--audio-stream',1).returncode!=0 and output.read_bytes()==original_output
    video=base/'selected-video.mov'
    good(ffmpeg,'-v','error','-itsoffset','1','-i',clip,'-itsoffset','1','-i',waves[0],'-itsoffset','1','-i',waves[1],'-map','0:v:0','-map','1:a:0','-map','2:a:0','-c:v','copy','-c:a','pcm_s16le',video)
    video_bytes=video.read_bytes()
    for index in [1,2]:
        output=base/f'video-playback-{index}.mp4'
        request={'operation':'media_playback_preview','input':str(video),'output':str(output),'directory':str(pack),'audio_stream':index,'max_dimension':64}
        receipt=execute(request)['result']
        assert receipt['source_video_stream_index']==0 and receipt['source_audio_stream_index']==index
        assert max(receipt['width'],receipt['height'])<=64
        info=json.loads(good(cli,'inspect-media',output,pack).stdout)
        assert info['audio_tracks']==info['video_tracks']==1 and abs(float(info['container_start_time']))<=.001
        assert abs(info['duration_seconds']-2)<=.1
        samples=pcm(output,1);values=struct.unpack('<'+'h'*(len(samples)//2),samples)
        average=sum(values[1024:-1024])/len(values[1024:-1024]);assert abs(average-[8192,-16384][index-1])<100
        assert receipt['sha256']==hashlib.sha256(output.read_bytes()).hexdigest() and receipt['source_sha256']==hashlib.sha256(video_bytes).hexdigest()
    selected=base/'video-audio-playback.wav'
    receipt=json.loads(good(cli,'playback-preview',video,selected,pack,'--audio-stream',2,'--audio-only').stdout)
    assert receipt['source_audio_stream_index']==2 and receipt['source_video_stream_index'] is None and receipt['width'] is None
    assert receipt['duration']['ticks']/receipt['duration']['timescale']==2 and pcm(selected)==pcm(video,2)
    assert json.loads(good(cli,'inspect-media',selected,pack).stdout)['video_tracks']==0
    muted=base/'muted.mp4'
    receipt=json.loads(good(cli,'playback-preview',video,muted,pack,'--mute-audio','--max-dimension',64).stdout)
    assert receipt['source_audio_stream_index'] is None and json.loads(good(cli,'inspect-media',muted,pack).stdout)['audio_tracks']==0
    rejected=base/'rejected.mp4'
    request={'operation':'media_playback_preview','input':str(video),'output':str(rejected),'directory':str(pack),'audio_stream':2}
    for change,code in [({'audio_stream':None},'unsupported'),({'audio_stream':0},'invalid_request'),
                        ({'mute_audio':True},'invalid_request'),({'max_dimension':63},'invalid_request'),
                        ({'max_dimension':1921},'invalid_request'),({'expected_source_sha256':'0'*64},'source_changed')]:
        reply=execute({**request,**change},ok=False);assert reply['error']['code']==code and not rejected.exists()
    assert not execute({**request,'input':str(source),'output':str(base/'wrong.wav'),'audio_stream':1,'mute_audio':True},ok=False)['ok']
    assert not execute({**request,'input':str(source),'audio_stream':1},ok=False)['ok']
    # Wait for the operation-owned output staging directory. The original has
    # already been snapshotted; changed original input cannot replace that content.
    for action in ['cancel','change-source']:
        baseline=set(base.iterdir());destination=base/f'{action}.mp4'
        process=subprocess.Popen([str(worker)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            process.stdin.write(json.dumps({'request':{**request,'output':str(destination),'max_dimension':64},'cancel_on_disconnect':True})+'\n');process.stdin.flush()
            deadline=time.monotonic()+30
            while not any(p.is_dir() for p in set(base.iterdir())-baseline):
                assert process.poll() is None and time.monotonic()<deadline
                time.sleep(.005)
            if action=='cancel':process.stdin.write('cancel\n');process.stdin.flush()
            else:video.write_bytes(video_bytes+b'\n%changed source\n')
            process.wait(timeout=180)
            reply=json.loads(process.stdout.read());stderr=process.stderr.read()
            assert not reply['ok'] and reply['error']['code']==('cancelled' if action=='cancel' else 'source_changed'),(reply,stderr)
        finally:
            if process.poll() is None:process.kill();process.wait()
            process.stdin.close();process.stdout.close();process.stderr.close();video.write_bytes(video_bytes)
        assert not destination.exists() and set(base.iterdir())==baseline
    assert source.read_bytes()==source_bytes and video.read_bytes()==video_bytes
    assert not any(p.is_dir() for p in base.iterdir()),'Playback staging leaked'
print('Playback: complete selected-track audio/video exports, offset normalization, bounded pictures, hashes, exact WAV content, measured AAC content/duration, mute, collision/invalid/stale rejection, active cancellation/source-change cleanup and unchanged originals passed')
