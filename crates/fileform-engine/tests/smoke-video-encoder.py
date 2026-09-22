#!/usr/bin/env python3
"""Prove an OS video encoder can encode, resize and fully decode a generated clip."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve()
encoder=sys.argv[2]
assert encoder in ['h264_mf','h264_videotoolbox']
suffix='.exe' if sys.platform=='win32' else ''
ffmpeg=pack/'bin'/('ffmpeg'+suffix)
ffprobe=pack/'bin'/('ffprobe'+suffix)
source=root/'crates/fileform-engine/tests/fixtures/h264-aac.mp4'

def run(*args):
    result=subprocess.run([str(x) for x in args],capture_output=True,timeout=120)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace'))
    return result.stdout

with tempfile.TemporaryDirectory(prefix='fileform-video-encoder-') as folder:
    output=Path(folder)/'encoded.mp4'
    options=['-hw_encoding','0'] if encoder=='h264_mf' else ['-allow_sw','1']
    run(ffmpeg,'-v','error','-nostdin','-i',source,'-map','0:v:0','-map','0:a:0','-vf','scale=128:96,setsar=1','-c:v',encoder,*options,'-b:v','2000000','-pix_fmt','yuv420p','-c:a','copy',output)
    info=json.loads(run(ffprobe,'-v','error','-show_streams','-show_format','-of','json',output))
    video=[s for s in info['streams'] if s['codec_type']=='video']
    audio=[s for s in info['streams'] if s['codec_type']=='audio']
    assert len(video)==len(audio)==1 and len(info['streams'])==2
    assert video[0]['codec_name']=='h264' and (video[0]['width'],video[0]['height'])==(128,96)
    assert video[0]['pix_fmt']=='yuv420p' and audio[0]['codec_name']=='aac'
    assert abs(float(info['format']['duration'])-2)<.1
    pixels=run(ffmpeg,'-v','error','-xerror','-err_detect','explode','-i',output,'-map','0:v:0','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-')
    assert len(pixels)==20*128*96*3//2
    audio_bytes=run(ffmpeg,'-v','error','-xerror','-i',output,'-map','0:a:0','-f','s16le','-')
    original_audio=run(ffmpeg,'-v','error','-xerror','-i',source,'-map','0:a:0','-f','s16le','-')
    assert audio_bytes==original_audio
    print(json.dumps({'encoder':encoder,'size':[128,96],'decodedFrames':20,'audioUnchanged':True,'bytes':output.stat().st_size}))

# Exercise the actual Rust route, not only the codec binary.
cli=root/'target/release'/('fileform-native'+suffix)
worker=root/'target/release'/('fileform-worker'+suffix)
with tempfile.TemporaryDirectory(prefix='fileform-video-native-') as folder:
    base=Path(folder)
    resized=base/'resized.mp4'
    receipt=json.loads(run(cli,'convert-video',source,resized,pack,'--max-dimension','32'))
    assert receipt['stream_copy'] is False and (receipt['width'],receipt['height'])==(32,24)
    pixels=run(ffmpeg,'-v','error','-i',resized,'-map','0:v:0','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-')
    reference=run(ffmpeg,'-v','error','-i',source,'-map','0:v:0','-vf','scale=32:24,setsar=1','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-')
    assert len(pixels)==len(reference)==20*32*24*3//2
    assert sum(abs(a-b) for a,b in zip(pixels,reference))/len(pixels)<12
    rotated=base/'rotated.mp4'
    run(ffmpeg,'-v','error','-display_rotation:v:0','90','-i',source,'-map','0','-c','copy',rotated)
    rotated_info=json.loads(run(ffprobe,'-v','error','-show_streams','-of','json',rotated))
    assert any(d.get('rotation')==90 for d in rotated_info['streams'][0].get('side_data_list',[]))
    output=base/'normalized.mov'
    response=subprocess.run([str(worker)],input=json.dumps({'operation':'convert_video','input':str(rotated),'output':str(output),'directory':str(pack),'options':{}})+'\n',text=True,capture_output=True,timeout=120)
    result=json.loads(response.stdout)
    assert response.returncode==0 and result['ok'],result
    assert (result['result']['width'],result['result']['height'])==(48,64)
    def picture(path):
        return run(ffmpeg,'-v','error','-i',path,'-map','0:v:0','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-')
    actual,reference=picture(output),picture(rotated)
    assert len(actual)==len(reference)==20*48*64*3//2
    assert sum(abs(a-b) for a,b in zip(actual,reference))/len(actual)<12
    pcm=base/'pcm.mov'
    run(ffmpeg,'-v','error','-i',source,'-c:v','copy','-c:a','pcm_s16le',pcm)
    run(cli,'convert-video',pcm,base/'aac.mp4',pack)
    invalid=base/'invalid.mp4'
    result=subprocess.run([str(worker)],input=json.dumps({'operation':'convert_video','input':str(source),'output':str(invalid),'directory':str(pack),'options':{'bitrate':1}})+'\n',text=True,capture_output=True,timeout=120)
    assert result.returncode!=0 and not invalid.exists()
    assert not any(p.is_dir() for p in base.iterdir())
    print(json.dumps({'nativeResize':[32,24],'decodedFrames':20,'rotationNormalized':[48,64],'pcmToAac':True,'invalidBitrateRejected':True}))

# A detailed synthetic source requires a genuine bitrate reduction to fit.
import random
with tempfile.TemporaryDirectory(prefix='fileform-video-fit-') as folder:
    base=Path(folder)
    raw=base/'noise.rgb'
    raw.write_bytes(random.Random(7).randbytes(128*96*3*60))
    dense=base/'dense.mp4'
    run(ffmpeg,'-v','error','-f','rawvideo','-pixel_format','rgb24','-video_size','128x96','-framerate','30','-i',raw,'-c:v','mpeg4','-q:v','2','-pix_fmt','yuv420p',dense)
    fitted=base/'fitted.mp4'
    receipt=json.loads(run(cli,'fit-video',dense,fitted,pack,200000))
    assert receipt['bytes']==fitted.stat().st_size<=200000
    assert receipt['attempts']>1 and receipt['requested_bitrate']>=150000
    assert (receipt['width'],receipt['height'])==(128,96)
    decoded=run(ffmpeg,'-v','error','-xerror','-i',fitted,'-map','0:v:0','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-')
    assert len(decoded)==60*128*96*3//2
    with_audio=base/'with-audio.mp4'
    audio_receipt=json.loads(run(cli,'fit-video',source,with_audio,pack,1000000))
    assert audio_receipt['audio_tracks']==1 and abs(audio_receipt['duration_seconds']-2)<.1
    missing=base/'unmet.mov'
    result=subprocess.run([str(worker)],input=json.dumps({'operation':'fit_video','input':str(dense),'output':str(missing),'directory':str(pack),'options':{'max_bytes':1,'minimum_bitrate':2000000}})+'\n',text=True,capture_output=True,timeout=120)
    failure=json.loads(result.stdout)
    assert result.returncode!=0 and failure['error']['code']=='target_unmet' and not missing.exists()
    assert not any(p.is_dir() for p in base.iterdir())
    print(json.dumps({'videoFitBytes':receipt['bytes'],'attempts':receipt['attempts'],'requestedBitrate':receipt['requested_bitrate'],'decodedFrames':60,'unmetTargetClean':True}))

# Exact video trims are checked against independently selected decoded content.
import math
import struct
import wave
with tempfile.TemporaryDirectory(prefix='fileform-video-trim-') as folder:
    base=Path(folder)
    tone=base/'distinct.wav'
    with wave.open(str(tone),'wb') as audio:
        audio.setparams((1,2,44100,0,'NONE','not compressed'))
        audio.writeframes(b''.join(struct.pack('<h',int(12000*math.sin(2*math.pi*437*i/44100))) for i in range(88200)))
    recording=base/'recording.mp4'
    run(ffmpeg,'-v','error','-i',source,'-i',tone,'-map','0:v:0','-map','1:a:0','-c:v','copy','-c:a','aac',recording)
    output=base/'trimmed.mp4'
    receipt=json.loads(run(cli,'trim-video',recording,output,pack,'.35','1.21'))
    assert (receipt['start_frame'],receipt['end_frame'])==(4,13)
    assert receipt['audio_samples']=={'start':17640,'end':57330}
    actual=run(ffmpeg,'-v','error','-i',output,'-map','0:v:0','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-')
    reference=run(ffmpeg,'-v','error','-i',recording,'-map','0:v:0','-vf','trim=start_frame=4:end_frame=13,setpts=PTS-STARTPTS','-pix_fmt','yuv420p','-fps_mode','passthrough','-f','rawvideo','-')
    assert len(actual)==len(reference)==9*64*48*3//2
    assert sum(abs(a-b) for a,b in zip(actual,reference))/len(actual)<12
    actual_audio=run(ffmpeg,'-v','error','-i',output,'-map','0:a:0','-f','s16le','-')
    expected_audio=run(ffmpeg,'-v','error','-i',recording,'-map','0:a:0','-af','atrim=start_sample=17640:end_sample=57330,asetpts=PTS-STARTPTS','-f','s16le','-')
    assert len(actual_audio)>=len(expected_audio)
    a=struct.unpack('<'+'h'*(len(expected_audio)//2),actual_audio[:len(expected_audio)])
    b=struct.unpack('<'+'h'*(len(expected_audio)//2),expected_audio)
    assert sum(abs(x-y) for x,y in zip(a,b))/len(a)/32768<.05
    muted=base/'muted.mov'
    muted_receipt=json.loads(run(cli,'trim-video',recording,muted,pack,'.35','1.21','--mute-audio'))
    assert muted_receipt['muted_audio'] and muted_receipt['audio_tracks']==0
    invalid=base/'past-end.mp4'
    request={'operation':'trim_video','input':str(recording),'output':str(invalid),'directory':str(pack),'options':{'interval':{'start':{'ticks':0,'timescale':1},'end':{'ticks':3,'timescale':1}}}}
    failure=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,timeout=120)
    assert failure.returncode!=0 and not invalid.exists()
    assert not any(p.is_dir() for p in base.iterdir())
    print(json.dumps({'exactVideoTrimFrames':9,'selectedFrames':[4,13],'audioSelectionVerified':True,'mutingVerified':True,'invalidRangeClean':True}))
