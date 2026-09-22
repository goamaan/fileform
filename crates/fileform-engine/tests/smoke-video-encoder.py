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
