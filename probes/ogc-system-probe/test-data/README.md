# MPEG decoder probe inputs

These short fixtures contain synthetic test audio. The Layer II and Layer III
files come from FFmpeg lavfi sine sources. The Layer I file is a deterministic
mono stream with one allocated subband and twelve valid MPEG-1 Layer I frames.
The Layer III file is not included in the repository. Generate it outside the
repository and set `LIBOGC_MAD_PROBE_MP3` to its absolute path before building
the probe. The decoder probe appends the eight zero guard bytes required by
libmad at run time.

The probe decodes all three files through `mad-sys`, checks the MPEG layer,
sample rate, and channel count, then checks that each stream produces enough
PCM frames and nonzero samples.
It also sends the MP3 fixture through `mp3player-sys` and checks that the
decoder filter runs for the full stream and playback completes.

To recreate the Layer II and Layer III files with FFmpeg, run these commands
in a directory outside the repository:

```sh
ffmpeg -f lavfi -i 'aevalsrc=0.20*sin(2*PI*440*t)|0.20*sin(2*PI*660*t):s=44100:d=0.50' \
  -map_metadata -1 -c:a mp2 -b:a 128k -f mp2 layer2-stereo.mp2
ffmpeg -f lavfi -i 'aevalsrc=0.20*sin(2*PI*440*t)|0.20*sin(2*PI*660*t):s=44100:d=0.50' \
  -map_metadata -1 -c:a libmp3lame -b:a 128k -write_xing 0 -id3v2_version 0 \
  -f mp3 layer3-stereo.mp3
```
