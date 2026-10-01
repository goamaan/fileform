#!/usr/bin/env python3
"""Distinct real audio tracks prove selection across conversion and editing."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import wave
root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve()
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
ffmpeg=pack/'bin'/('ffmpeg'+suffix);ffprobe=pack/'bin'/('ffprobe'+suffix)
def run(*args):return subprocess.run([str(v) for v in args],capture_output=True,timeout=180)
def good(*args):
    value=run(*args);assert value.returncode==0,value.stderr;return value

def execute(request,ok=True,cancel=False):
    value=subprocess.run([str(worker)],input=json.dumps(request)+'\n'+('cancel\n' if cancel else ''),text=True,capture_output=True,timeout=180)
    reply=json.loads(value.stdout);assert reply['ok']==ok,reply;return reply

def pcm(path,index=0):return good(ffmpeg,'-v','error','-i',path,'-map',f'0:{index}','-c:a','pcm_s16le','-f','s16le','-').stdout

def packets(path,index):
    return json.loads(good(ffprobe,'-v','error','-select_streams',index,'-show_packets','-show_data_hash','sha256','-show_entries','packet=pts,dts,duration,data_hash','-of','json',path).stdout)['packets']
with tempfile.TemporaryDirectory(prefix='fileform-selected-audio-') as folder:
    base=Path(folder)
    inputs=[];values=[8192,-16384]
    for index,value in enumerate(values):
        source=base/f'track-{index}.wav'
        with wave.open(str(source),'wb') as audio:
            audio.setparams((1,2,44100,0,'NONE','not compressed'));audio.writeframes(struct.pack('<h',value)*88200)
        inputs.append(source)
    source=base/'two-tracks.mov'
    good(ffmpeg,'-v','error','-i',inputs[0],'-i',inputs[1],'-map','0:a:0','-map','1:a:0','-c:a','pcm_s16le','-metadata:s:a:0','language=eng','-metadata:s:a:1','language=fra',source)
    original=source.read_bytes();source_hash=hashlib.sha256(original).hexdigest()
    inspection=json.loads(good(cli,'inspect-media',source,pack).stdout)
    assert inspection['audio_tracks']==2 and [s['tags']['language'] for s in inspection['streams']]==['eng','fra']
    for index,value in enumerate(values):
        expected=struct.pack('<h',value)*88200
        for extension in ['wav','flac']:
            output=base/f'selected-{index}.{extension}'
            receipt=json.loads(good(cli,'convert-audio',source,output,pack,'--audio-stream',index).stdout)
            assert receipt['source_stream_index']==index and pcm(output)==expected
        timeline=json.loads(good(cli,'inspect-audio-timeline',source,pack,'--audio-stream',index).stdout)
        assert timeline['stream_index']==index and timeline['decoded_samples']==88200
        waveform=json.loads(good(cli,'waveform',source,pack,'--audio-stream',index).stdout)
        assert waveform['stream_index']==index and waveform['sample_count']==88200
        assert waveform['channels'][0]['minimum']==waveform['channels'][0]['maximum']==[value/32768]*len(waveform['channels'][0]['minimum'])
        sample_output=base/f'samples-{index}.flac'
        sample=json.loads(good(cli,'trim-audio',source,sample_output,pack,123,9876,'--audio-stream',index).stdout)
        assert sample['source_stream_index']==index and pcm(sample_output)==expected[123*2:9876*2]
        timed_output=base/f'time-{index}.wav'
        timed=json.loads(good(cli,'trim-audio-time',source,timed_output,pack,'.25','1.001','--audio-stream',index).stdout)
        assert timed['source_stream_index']==index and pcm(timed_output)==expected[11025*2:44145*2]
        lossy=base/f'time-{index}.m4a'
        receipt=json.loads(good(cli,'trim-audio-time',source,lossy,pack,'.25','1.001','--audio-stream',index).stdout)
        assert receipt['source_stream_index']==index and receipt['lossy_codec'] and abs(receipt['duration_seconds']-.751)<1025/44100
        decoded=pcm(lossy)
        assert 33120*2<=len(decoded)<=(33120+1024)*2
        samples=struct.unpack('<'+'h'*(len(decoded)//2),decoded)
        middle=samples[1024:-1024]
        assert abs(sum(middle)/len(middle)-value)<100
        fit_output=base/f'fit-{index}.flac'
        fitted=json.loads(good(cli,'fit-audio',source,fit_output,pack,200000,'--audio-stream',index).stdout)
        assert fitted['source_stream_index']==index and pcm(fit_output)==expected
    # MP3's discrete bitrate choices must stay above a nonstandard floor.
    floored=base/'nonstandard-floor.mp3'
    result=execute({'operation':'fit_audio','input':str(source),'output':str(floored),'directory':str(pack),'audio_stream':1,'max_bytes':100000,'minimum_bitrate':56001})['result']
    assert result['requested_bitrate']>=56001 and result['source_stream_index']==1
    # Source indices are actual ffprobe stream indices, not ordinal audio positions.
    clip=root/'crates/fileform-engine/tests/fixtures/h264-aac.mp4'
    mixed=base/'picture-and-two-tracks.mov'
    good(ffmpeg,'-v','error','-i',clip,'-i',inputs[0],'-i',inputs[1],'-map','0:v:0','-map','1:a:0','-map','2:a:0','-c:v','copy','-c:a','aac',mixed)
    selected=base/'mixed-selected.wav'
    result=execute({'operation':'convert_audio','input':str(mixed),'output':str(selected),'directory':str(pack),'audio_stream':2})['result']
    assert result['source_stream_index']==2 and pcm(selected)==pcm(mixed,2) and pcm(selected)!=pcm(mixed,1)
    copied=base/'selected-copy.m4a'
    receipt=json.loads(good(cli,'copy-audio-trim',mixed,copied,pack,'.35','1.21','--audio-stream',2).stdout)
    assert receipt['source_stream_index']==2
    before,after=packets(mixed,2),packets(copied,0)
    hashes=[p['data_hash'] for p in before];out_hashes=[p['data_hash'] for p in after]
    assert any(hashes[i:i+len(out_hashes)]==out_hashes for i in range(len(hashes)))
    # Missing, nonaudio, unknown, repeated and stale selections cannot publish.
    output=base/'rejected.wav'
    for selection in [None,999]:
        options={} if selection is None else {'audio_stream':selection}
        reply=execute({'operation':'convert_audio','input':str(source),'output':str(output),'directory':str(pack),**options},ok=False)
        assert reply['error']['code']==('unsupported' if selection is None else 'invalid_request') and not output.exists()
    reply=execute({'operation':'convert_audio','input':str(mixed),'output':str(output),'directory':str(pack),'audio_stream':0},ok=False)
    assert reply['error']['code']=='invalid_request' and not output.exists()
    assert run(cli,'convert-audio',source,output,pack,'--audio-stream',1,'--audio-stream',0).returncode!=0 and not output.exists()
    for options,cancel in [({'expected_source_sha256':'0'*64},False),({},True)]:
        reply=execute({'operation':'convert_audio','input':str(source),'output':str(output),'directory':str(pack),'audio_stream':1,**options},ok=False,cancel=cancel)
        assert reply['error']['code']==('cancelled' if cancel else 'source_changed') and not output.exists()
    assert source.read_bytes()==original and hashlib.sha256(source.read_bytes()).hexdigest()==source_hash
    assert not any(p.is_dir() for p in base.iterdir()),'Staging leaked'
print('Audio selection: distinct real tracks, actual stream indices, language labels, lossless conversion/extraction/fit, exact sample/time trims, measured timelines/waveforms, AAC packet copy, invalid/missing/nonaudio/stale selection, cancellation and cleanup passed')
