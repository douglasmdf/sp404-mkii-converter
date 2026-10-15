#!/bin/sh
# Regenerates the compressed test fixtures. Needs ffmpeg (dev only, never shipped).
set -eu
cd "$(dirname "$0")"
gen() {
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "sine=frequency=1000:sample_rate=44100:duration=0.5" -ac 2 "$@"
}
gen -c:a flac sine.flac
gen -c:a libmp3lame -b:a 192k sine.mp3
gen -c:a aac -b:a 192k sine.m4a
if ffmpeg -hide_banner -encoders 2>/dev/null | grep -q libvorbis; then
  gen -c:a libvorbis -q:a 5 sine.ogg
else
  # Builds without libvorbis: ffmpeg's native Vorbis encoder (stereo only) is fine for a test file.
  gen -c:a vorbis -strict -2 sine.ogg
fi
gen -c:a pcm_s16be sine.aiff
gen -c:a pcm_s16be -f caf sine.caf
