#!/bin/bash
# ./stream.sh <rtp-ip> <rtp-audio-port> <rtp-video-port>

set -euo pipefail

FILE=$1

ffmpeg -re -stream_loop -1 -i "$1" \
    -map 0:v:0 \
    -c:v libx264 -profile:v high -level:v 4.0 \
    -pix_fmt yuv420p -g 99999 \
    -map 0:a:0 \
    -c:a libopus -ab 128k -ac 2 -ar 48000 \
    -f tee \
    out.mp4
