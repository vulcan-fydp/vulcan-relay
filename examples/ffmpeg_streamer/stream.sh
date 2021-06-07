#!/bin/bash
# ./stream.sh <rtp-ip> <rtp-audio-port> <rtp-video-port>

set -euo pipefail

FILE=$1
IP=$2
AUDIO_PORT=$3
VIDEO_PORT=$4

ffmpeg -re -stream_loop -1 -i "$1" \
    -map 0:v:0 \
    -c:v libx264 -preset ultrafast -maxrate 3000k -bufsize 3000k \
    -pix_fmt yuv420p -g 50 \
    -vf "scale=1280:-1,format=yuv420p" \
    -map 0:a:0 \
    -c:a libopus -ab 128k -ac 2 -ar 48000 \
    -f tee \
    "[select=a:f=rtp:ssrc=11111111:payload_type=101]rtp://$IP:$AUDIO_PORT|[select=v:f=rtp:ssrc=22222222:payload_type=102]rtp://$IP:$VIDEO_PORT"