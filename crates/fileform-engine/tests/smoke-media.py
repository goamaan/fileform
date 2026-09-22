#!/usr/bin/env python3
"""Real-tool audio parity checks. Run from any cwd with a verified media pack path."""
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import wave

root = Path(__file__).resolve().parents[3]
pack = Path(sys.argv[1]).resolve()
suffix = '.exe' if sys.platform == 'win32' else ''
cli = root / 'target/release' / ('fileform-native' + suffix)
worker = root / 'target/release' / ('fileform-worker' + suffix)
ffmpeg = pack / 'bin' / ('ffmpeg' + suffix)

def run(*args):
    return subprocess.run([str(x) for x in args], capture_output=True, check=True)

def request(data, ok=True, cancel=False):
    response = subprocess.run([str(worker)], input=json.dumps(data)+'\n'+('cancel\n' if cancel else ''), text=True, capture_output=True)
    value = json.loads(response.stdout)
    assert value['ok'] == ok and (response.returncode == 0) == ok, value
    return value

run(cli, 'verify-media-pack', pack)
with tempfile.TemporaryDirectory(prefix='fileform-audio-smoke-') as folder:
    base = Path(folder)
    source = base/'tone.wav'
    with wave.open(str(source), 'wb') as audio:
        audio.setparams((1, 2, 44100, 0, 'NONE', 'not compressed'))
        audio.writeframes(b''.join(struct.pack('<h', int(12000*math.sin(2*math.pi*440*i/44100))) for i in range(88200)))
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    def pcm(path):
        return run(ffmpeg, '-v', 'error', '-i', path, '-map', '0:a:0', '-f', 's16le', '-').stdout
    expected_pcm = pcm(source)
    receipts = []
    for extension in ['wav', 'flac', 'm4a', 'mp3']:
        output = base/('converted.'+extension)
        result = json.loads(run(cli, 'convert-audio', source, output, pack).stdout)
        assert result['channels'] == 1 and result['sample_rate'] == '44100'
        assert abs(result['duration_seconds']-2) <= .25
        decoded = pcm(output)
        assert len(decoded) >= len(expected_pcm)-4096
        if extension in ['wav','flac']:
            assert decoded == expected_pcm
        before = output.read_bytes()
        collision = subprocess.run([str(cli), 'convert-audio', str(source), str(output), str(pack)], capture_output=True)
        assert collision.returncode != 0 and output.read_bytes() == before
        receipts.append(result)
    frames = base/'frames.rgb'
    frames.write_bytes(b''.join(bytes((x*4%256,y*5%256,i*10%256)) for i in range(20) for y in range(48) for x in range(64)))
    video = base/'clip.mp4'
    run(ffmpeg, '-v','error','-f','rawvideo','-pixel_format','rgb24','-video_size','64x48','-framerate','10','-i',frames,'-i',source,'-c:v','mpeg4','-pix_fmt','yuv420p','-c:a','aac','-shortest',video)
    extracted = base/'extracted.wav'
    request({'operation':'convert_audio','input':str(video),'output':str(extracted),'directory':str(pack)})
    assert pcm(extracted) == pcm(video)
    for name, extras, cancel in [('changed',{'expected_source_sha256':'0'*64},False),('cancelled',{},True)]:
        output = base/(name+'.wav')
        failure = request({'operation':'convert_audio','input':str(source),'output':str(output),'directory':str(pack),**extras},ok=False,cancel=cancel)
        assert failure['error']['code'] == ('cancelled' if cancel else 'source_changed')
        assert not output.exists()
    high = base/'high.wav'
    with wave.open(str(high),'wb') as audio:
        audio.setparams((1,4,44100,0,'NONE','not compressed'));audio.writeframes(b'\0'*44100*4)
    unsupported = base/'high.flac'
    request({'operation':'convert_audio','input':str(high),'output':str(unsupported),'directory':str(pack)},ok=False)
    assert not unsupported.exists()
    precision = base/'precision.wav'
    with wave.open(str(precision),'wb') as audio:
        audio.setparams((1,3,44100,0,'NONE','not compressed'))
        audio.writeframes(b''.join(int(4000000*math.sin(2*math.pi*440*i/44100)).to_bytes(3,'little',signed=True) for i in range(44100)))
    flac24 = base/'precision.flac'
    request({'operation':'convert_audio','input':str(precision),'output':str(flac24),'directory':str(pack)})
    def pcm32(path):
        return run(ffmpeg,'-v','error','-i',path,'-map','0:a:0','-f','s32le','-').stdout
    assert pcm32(precision) == pcm32(flac24)
    floating = base/'floating.wav'
    run(ffmpeg,'-v','error','-i',source,'-c:a','pcm_f32le',floating)
    lowrate = base/'lowrate.wav'
    with wave.open(str(lowrate),'wb') as audio:
        audio.setparams((1,2,8000,0,'NONE','not compressed'));audio.writeframes(b'\0'*16000)
    multi = base/'multi.mka'
    run(ffmpeg,'-v','error','-i',source,'-map','0:a:0','-map','0:a:0','-c:a','flac',multi)
    for name, input_file, extension in [('float',floating,'flac'),('lowrate',lowrate,'mp3'),('multiple',multi,'wav')]:
        output = base/(name+'-rejected.'+extension)
        failure = request({'operation':'convert_audio','input':str(input_file),'output':str(output),'directory':str(pack)},ok=False)
        assert failure['error']['code'] == 'unsupported' and not output.exists()
    for extension, start, end in [('wav',12345,54321),('flac',12345,54321),('wav',0,1)]:
        output = base/f'trim-{start}-{end}.{extension}'
        result = json.loads(run(cli,'trim-audio',source,output,pack,start,end).stdout)
        assert result['trimmed_samples'] == {'start':start,'end':end}
        assert pcm(output) == expected_pcm[start*2:end*2]
    trimmed24=base/'trim24.flac'
    request({'operation':'trim_audio','input':str(precision),'output':str(trimmed24),'directory':str(pack),'samples':{'start':100,'end':12345}})
    assert pcm32(trimmed24) == pcm32(precision)[100*4:12345*4]
    for name,start,end in [('empty',1,1),('reversed',2,1),('past-end',88000,89000)]:
        output=base/(name+'-trim.wav')
        request({'operation':'trim_audio','input':str(source),'output':str(output),'directory':str(pack),'samples':{'start':start,'end':end}},ok=False)
        assert not output.exists()
    h264=root/'crates/fileform-engine/tests/fixtures/h264-aac.mp4'
    ffprobe=pack/'bin'/('ffprobe'+suffix)
    def packets(path):
        value=json.loads(run(ffprobe,'-v','error','-show_packets','-show_data_hash','sha256','-show_entries','packet=stream_index,pts_time,duration_time,data_hash','-of','json',path).stdout)
        return value['packets']
    mov=base/'copied.mov'
    request({'operation':'remux_video','input':str(h264),'output':str(mov),'directory':str(pack)})
    roundtrip=base/'roundtrip.mp4'
    run(cli,'remux-video',mov,roundtrip,pack)
    assert packets(h264)==packets(mov)==packets(roundtrip)
    for variant,options in [('silent',['-map','0:v:0']),('rotated',['-map','0','-metadata:s:v:0','rotate=90'])]:
        selected=base/(variant+'.mp4')
        run(ffmpeg,'-v','error','-i',h264,*options,'-c','copy',selected)
        request({'operation':'remux_video','input':str(selected),'output':str(base/(variant+'.mov')),'directory':str(pack)})
    rejected=base/'unsupported-video.mov'
    request({'operation':'remux_video','input':str(video),'output':str(rejected),'directory':str(pack)},ok=False)
    assert not rejected.exists()
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
    assert not any(p.is_dir() for p in base.iterdir()), 'Staging directory leaked'
    print(json.dumps({'formats':['wav','flac','m4a','mp3'],'lossless_pcm_exact':True,'video_audio_extraction_exact':True,'collisions_preserved':True,'stale_source_rejected':True,'cancellation_clean':True,'high_depth_flac_rejected':True,'source_unchanged':True,'flac_24bit_exact':True,'float_flac_rejected':True,'mp3_resampling_rejected':True,'multiple_tracks_rejected':True,'exact_sample_trims':True,'single_sample_trim':True,'trim_24bit_exact':True,'invalid_ranges_clean':True,'video_packets_and_timing_exact':True,'silent_and_rotated_video':True,'unsupported_video_rejected':True}))
