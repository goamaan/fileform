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
    timeline=json.loads(run(cli,'inspect-audio-timeline',source,pack).stdout)
    assert timeline['decoded_samples']==88200 and timeline['origin_ticks']==0 and timeline['continuous_sample_clock']
    offset=base/'offset.mov'
    run(ffmpeg,'-v','error','-itsoffset','1','-i',source,'-c:a','pcm_s16le',offset)
    offset_timeline=request({'operation':'inspect_audio_timeline','input':str(offset),'directory':str(pack)})['result']
    assert offset_timeline['decoded_samples']==88200 and offset_timeline['origin_ticks']==44100
    gap=base/'gap.m4a'
    run(ffmpeg,'-v','error','-i',source,'-af',r'asetpts=PTS+gte(N\,44100)/TB','-c:a','aac',gap)
    gap_result=request({'operation':'inspect_audio_timeline','input':str(gap),'directory':str(pack)},ok=False)
    assert gap_result['error']['code']=='unsupported' and 'gaps, overlaps' in gap_result['error']['message']
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
    for extension in ['mp3','m4a']:
        output=base/('fitted.'+extension)
        result=json.loads(run(cli,'fit-audio',source,output,pack,20000).stdout)
        assert result['bytes']==output.stat().st_size<=20000
        assert result['attempts']>1 and result['requested_bitrate']>=48000
        assert len(expected_pcm)<=len(pcm(output))<=len(expected_pcm)+4096
        too_high=base/('floor-unmet.'+extension)
        failure=request({'operation':'fit_audio','input':str(source),'output':str(too_high),'directory':str(pack),'max_bytes':20000,'minimum_bitrate':128000},ok=False)
        assert failure['error']['code']=='target_unmet' and not too_high.exists()
    for extension in ['wav','flac']:
        output=base/('fitted-lossless.'+extension)
        value=request({'operation':'fit_audio','input':str(source),'output':str(output),'directory':str(pack),'max_bytes':200000})['result']
        assert value['attempts']==1 and value['requested_bitrate'] is None and pcm(output)==expected_pcm
        rejected=base/('lossless-unmet.'+extension)
        failure=request({'operation':'fit_audio','input':str(source),'output':str(rejected),'directory':str(pack),'max_bytes':1000},ok=False)
        assert failure['error']['code']=='target_unmet' and not rejected.exists()
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
    for name,input_file,extension in [('normal',source,'flac'),('offset',offset,'wav')]:
        output=base/(name+'-timed.'+extension)
        result=json.loads(run(cli,'trim-audio-time',input_file,output,pack,'.25','1.001').stdout)
        assert result['trimmed_samples']=={'start':11025,'end':44145}
        assert result['realized_interval']['end']=={'ticks':44145,'timescale':44100}
        assert result['source_origin_ticks']==(44100 if name=='offset' else 0)
        assert pcm(output)==expected_pcm[11025*2:44145*2]
    interval={'start':{'ticks':250,'timescale':1000},'end':{'ticks':1001,'timescale':1000}}
    for name,input_file,options in [('gap',gap,{}),('stale',source,{'expected_source_sha256':'0'*64}),('past-end',source,{'interval':{'start':{'ticks':0,'timescale':1},'end':{'ticks':3,'timescale':1}}})]:
        output=base/(name+'-timed-rejected.wav')
        failure=request({'operation':'trim_audio_time','input':str(input_file),'output':str(output),'directory':str(pack),'interval':interval,**options},ok=False)
        assert failure['error']['code']=={'gap':'unsupported','stale':'source_changed','past-end':'invalid_request'}[name]
        assert not output.exists()
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
    video_clock=json.loads(run(cli,'inspect-video-timeline',h264,pack).stdout)
    assert video_clock['decoded_frames']==20 and video_clock['frame_ticks']==1024
    assert video_clock['duration_ticks']==20480 and video_clock['keyframe_indices'][0]==0
    assert not video_clock['has_reordered_packets']
    reordered=base/'reordered.mp4'
    run(ffmpeg,'-v','error','-i',h264,'-map','0:v:0','-c:v','mpeg4','-bf','2','-g','10',reordered)
    reordered_clock=request({'operation':'inspect_video_timeline','input':str(reordered),'directory':str(pack)})['result']
    assert reordered_clock['decoded_frames']==20 and reordered_clock['has_reordered_packets']
    variable=base/'variable.mp4'
    run(ffmpeg,'-v','error','-i',h264,'-map','0:v:0','-c','copy','-bsf:v',r'setts=pts=PTS+gte(N\,10)*1024:dts=DTS+gte(N\,10)*1024',variable)
    failure=request({'operation':'inspect_video_timeline','input':str(variable),'directory':str(pack)},ok=False)
    assert failure['error']['code']=='unsupported' and 'constant-rate' in failure['error']['message']

    def packets(path):
        value=json.loads(run(ffprobe,'-v','error','-show_packets','-show_data_hash','sha256','-show_entries','packet=stream_index,pts_time,duration_time,data_hash','-of','json',path).stdout)
        return value['packets']
    mov=base/'copied.mov'
    request({'operation':'remux_video','input':str(h264),'output':str(mov),'directory':str(pack)})
    roundtrip=base/'roundtrip.mp4'
    run(cli,'remux-video',mov,roundtrip,pack)
    assert packets(h264)==packets(mov)==packets(roundtrip)
    for variant,options in [('silent',['-map','0:v:0']),('rotated',['-map','0'])]:
        selected=base/(variant+'.mp4')
        run(ffmpeg,'-v','error',*(['-display_rotation:v:0','90'] if variant=='rotated' else []),'-i',h264,*options,'-c','copy',selected)
        if variant=='rotated':
            info=json.loads(run(ffprobe,'-v','error','-show_streams','-of','json',selected).stdout)
            assert any(d.get('rotation')==90 for d in info['streams'][0].get('side_data_list',[]))
        request({'operation':'remux_video','input':str(selected),'output':str(base/(variant+'.mov')),'directory':str(pack)})
    rejected=base/'unsupported-video.mov'
    request({'operation':'remux_video','input':str(video),'output':str(rejected),'directory':str(pack)},ok=False)
    assert not rejected.exists()
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
    assert not any(p.is_dir() for p in base.iterdir()), 'Staging directory leaked'
    print(json.dumps({'formats':['wav','flac','m4a','mp3'],'lossless_pcm_exact':True,'video_audio_extraction_exact':True,'collisions_preserved':True,'stale_source_rejected':True,'cancellation_clean':True,'high_depth_flac_rejected':True,'source_unchanged':True,'flac_24bit_exact':True,'float_flac_rejected':True,'mp3_resampling_rejected':True,'multiple_tracks_rejected':True,'exact_sample_trims':True,'single_sample_trim':True,'trim_24bit_exact':True,'invalid_ranges_clean':True,'video_packets_and_timing_exact':True,'silent_and_rotated_video':True,'unsupported_video_rejected':True,'audio_byte_limits_verified':True,'minimum_bitrate_enforced':True,'lossless_fit_checked':True,'audio_clock_and_offset_verified':True,'gapped_clock_rejected':True,'source_time_trim_exact':True,'offset_time_trim_exact':True,'invalid_time_trims_clean':True,'decoded_video_clock_verified':True,'reordered_video_identified':True,'variable_timing_rejected':True}))
