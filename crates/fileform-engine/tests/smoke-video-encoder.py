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
