#!/usr/bin/env python3
"""Video operations retain the explicitly selected audio stream."""
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import wave
root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve();suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix);worker=root/'target/release'/('fileform-worker'+suffix)
ffmpeg=pack/'bin'/('ffmpeg'+suffix);ffprobe=pack/'bin'/('ffprobe'+suffix)
def run(*args):return subprocess.run([str(v) for v in args],capture_output=True,timeout=240)
def good(*args):
    result=run(*args);assert result.returncode==0,result.stderr;return result

def execute(request,ok=True):
    result=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,timeout=240)
    reply=json.loads(result.stdout);assert reply['ok']==ok,reply;return reply

def audio(path,index):return good(ffmpeg,'-v','error','-i',path,'-map',f'0:{index}','-c:a','pcm_s16le','-f','s16le','-').stdout

def video(path):return good(ffmpeg,'-v','error','-noautorotate','-i',path,'-map','0:v:0','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-').stdout

def packet_hashes(path,index):return [p['data_hash'] for p in json.loads(good(ffprobe,'-v','error','-select_streams',index,'-show_packets','-show_data_hash','sha256','-show_entries','packet=data_hash','-of','json',path).stdout)['packets']]

def tone(path,index,frequency):
    raw=audio(path,index);values=struct.unpack('<'+'h'*(len(raw)//2),raw)
    # A phase-independent measured spectrum separates the two fixture tracks,
    # including lossy re-encoded trims with different start positions.
    count=min(4410,len(values));values=values[:count]
    energy=sum(v*v for v in values)
    real=sum(v*math.cos(2*math.pi*frequency*i/44100) for i,v in enumerate(values))
    imag=sum(v*math.sin(2*math.pi*frequency*i/44100) for i,v in enumerate(values))
    return (real*real+imag*imag)/(energy*count) if energy else 0
with tempfile.TemporaryDirectory(prefix='fileform-video-track-') as folder:
    base=Path(folder);clip=root/'crates/fileform-engine/tests/fixtures/h264-aac.mp4'
    sources=[]
    for index,frequency in enumerate([440,880]):
        path=base/f'tone-{index}.wav'
        with wave.open(str(path),'wb') as recording:
            recording.setparams((1,2,44100,0,'NONE','not compressed'))
            recording.writeframes(b''.join(struct.pack('<h',int(12000*math.sin(2*math.pi*frequency*i/44100))) for i in range(88200)))
        sources.append(path)
    source=base/'two-track.mp4'
    good(ffmpeg,'-v','error','-i',clip,'-i',sources[0],'-i',sources[1],'-map','0:v:0','-map','1:a:0','-map','2:a:0','-c:v','copy','-c:a','aac',source)
    original=source.read_bytes();decoded_video=video(source)
    interval={'start':{'ticks':35,'timescale':100},'end':{'ticks':121,'timescale':100}}
    for index,frequency in [(1,440),(2,880)]:
        expected_audio=audio(source,index)
        remuxed=base/f'remux-{index}.mov'
        receipt=json.loads(good(cli,'remux-video',source,remuxed,pack,'--audio-stream',index).stdout)
        assert receipt['source_audio_stream_index']==index and receipt['audio_tracks']==1
        assert audio(remuxed,1)==expected_audio and video(remuxed)==decoded_video
        assert packet_hashes(remuxed,1)==packet_hashes(source,index)
        converted=base/f'converted-{index}.mp4'
        receipt=json.loads(good(cli,'convert-video',source,converted,pack,'--audio-stream',index).stdout)
        assert receipt['source_audio_stream_index']==index and audio(converted,1)==expected_audio
        fitted=base/f'fit-{index}.mp4'
        receipt=json.loads(good(cli,'fit-video',source,fitted,pack,100000,'--audio-stream',index).stdout)
        assert receipt['source_audio_stream_index']==index and receipt['bytes']<=100000 and tone(fitted,1,frequency)>.4
        exact=base/f'exact-{index}.mp4'
        receipt=execute({'operation':'trim_video','input':str(source),'output':str(exact),'directory':str(pack),'options':{'interval':interval,'audio_stream':index}})['result']
        assert receipt['source_audio_stream_index']==index and (receipt['start_frame'],receipt['end_frame'])==(4,13)
        assert receipt['audio_samples']=={'start':17640,'end':57330} and tone(exact,1,frequency)>.4
        copied=base/f'copy-{index}.mp4'
        receipt=json.loads(good(cli,'copy-video-trim',source,copied,pack,'1.3','1.7','--audio-stream',index).stdout)
        assert receipt['source_audio_stream_index']==index and receipt['audio_tracks']==1
        original_hashes,copied_hashes=packet_hashes(source,index),packet_hashes(copied,1)
        assert any(original_hashes[i:i+len(copied_hashes)]==copied_hashes for i in range(len(original_hashes)))
    # An unsupported unselected track must not reject an eligible selected one.
    mixed=base/'pcm-and-aac.mov'
    good(ffmpeg,'-v','error','-i',source,'-map','0:v:0','-map','0:a:0','-map','0:a:1','-c:v','copy','-c:a:0','pcm_s16le','-c:a:1','copy',mixed)
    selected=base/'eligible-selected.mov'
    assert json.loads(good(cli,'remux-video',mixed,selected,pack,'--audio-stream',2).stdout)['source_audio_stream_index']==2
    assert audio(selected,1)==audio(mixed,2)
    muted=base/'muted.mp4'
    receipt=json.loads(good(cli,'copy-video-trim',source,muted,pack,'1.3','1.7','--mute-audio').stdout)
    assert receipt['audio_tracks']==0 and receipt['source_audio_stream_index'] is None
    rejected=base/'rejected.mp4'
    for route,args in [('remux-video',[]),('convert-video',[]),('fit-video',[100000]),('trim-video',['.35','1.21']),('copy-video-trim',['1.3','1.7'])]:
        assert run(cli,route,source,rejected,pack,*args).returncode!=0 and not rejected.exists()
        assert run(cli,route,source,rejected,pack,*args,'--audio-stream',0).returncode!=0 and not rejected.exists()
    reply=execute({'operation':'copy_video_trim','input':str(source),'output':str(rejected),'directory':str(pack),'options':{'interval':interval,'audio_stream':2,'mute_audio':True}},ok=False)
    assert reply['error']['code']=='invalid_request' and not rejected.exists()
    assert source.read_bytes()==original
    assert not any(p.is_dir() for p in base.iterdir()),'Staging leaked'
print('Video audio selection: distinct actual tracks in remux/convert/fit/exact trim/keyframe copy, decoded picture/audio and packet preservation, measured tone content, selected AAC with unselected PCM, explicit mute, ambiguous/nonaudio/conflicting rejection and source/cleanup safety passed')
