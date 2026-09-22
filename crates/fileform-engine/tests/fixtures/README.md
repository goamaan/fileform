# Generated media fixture

`h264-aac.mp4` is original synthetic test content under the repository's Apache-2.0
license. It contains no user recordings: 20 generated 64×48 RGB frames at 10 fps
and a generated 2-second 440 Hz sine wave at 44.1 kHz. Frame pixel `(x,y)` at frame
`i` is `(x*4 % 256, y*5 % 256, i*10 % 256)`; mono PCM16 samples are
`int(12000*sin(2*pi*440*i/44100))`.

Generated with the verified Fileform FFmpeg 9.0.1 pack: raw RGB + WAV were encoded
to MPEG-4/AAC, then video transcoded with `h264_videotoolbox -allow_sw 1 -pix_fmt
yuv420p` while copying AAC. The intermediate recipe is exercised by smoke-media.py.
This checked-in H.264 fixture allows Windows remux/decoding tests without assuming
hardware encoder availability. Encoder byte-for-byte reproducibility is not claimed.

SHA-256: `6c347933b4681bf3c95ab62ebb240fe5eddd8827c53103decada29f16cd9772d`.
