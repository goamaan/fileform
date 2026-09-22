#!/usr/bin/env python3
"""Diagnostic comparison using generated content, not an acceptance test."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
assert sys.platform=='win32'
root=Path(__file__).resolve().parents[3]
pack=Path(sys.argv[1]).resolve()
ffmpeg=pack/'bin/ffmpeg.exe'
ffprobe=pack/'bin/ffprobe.exe'
source=root/'crates/fileform-engine/tests/fixtures/h264-aac.mp4'
with tempfile.TemporaryDirectory(prefix='fileform-mf-probe-') as folder:
    for width,height in [(32,24),(64,48),(128,96)]:
        for timing,flags in [('default',[]),('passthrough',['-fps_mode','passthrough']),('passthrough-rate',['-fps_mode','passthrough','-r','10'])]:
            for minimal in [False,True]:
                output=Path(folder)/f'{width}-{height}-{timing}-{minimal}.mp4'
                args=[str(ffmpeg),'-hide_banner','-nostdin','-v','error','-nostats','-xerror','-max_alloc','268435456','-protocol_whitelist','file,pipe','-format_whitelist','mov,matroska,webm,avi','-threads','2','-i',str(source),'-map','0:v:0','-map','0:a:0?','-map_metadata','-1','-map_chapters','-1','-vf',f'scale={width}:{height},setsar=1','-c:v','h264_mf','-hw_encoding','0','-b:v','2000000','-pix_fmt','yuv420p',*flags,'-filter_threads','2','-threads','2','-c:a','copy','-movflags','+faststart','-f','mp4',str(output)]
                env={'SystemRoot':os.environ['SystemRoot']} if minimal else None
                result=subprocess.run(args,stdin=subprocess.DEVNULL,capture_output=True,env=env,creationflags=subprocess.CREATE_NO_WINDOW,timeout=60)
                row={'size':[width,height],'timing':timing,'minimalEnvironment':minimal,'status':result.returncode,'error':result.stderr.decode(errors='replace')[:1200]}
                if result.returncode==0:
                    probe=subprocess.run([str(ffprobe),'-v','error','-select_streams','v:0','-show_entries','stream=width,height,nb_frames,avg_frame_rate','-of','json',str(output)],capture_output=True,check=True,timeout=30)
                    row['output']=json.loads(probe.stdout)['streams'][0]
                print(json.dumps(row),flush=True)
