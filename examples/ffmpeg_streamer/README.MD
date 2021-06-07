
```
ffmpeg -re -f lavfi -i aevalsrc="sin(400*2*PI*t)" \
    -acodec libopus -ar 48000 -ac 2 -ab 128k \
    -ssrc 11111111 -payload_type 101 \
    -f rtp rtp://192.168.140.128:50180
```

```
ffmpeg -re -stream_loop -1 -i /mnt/vmshared/tX0ddiqtUM.mp4 -an \
    -c:v libx264 -preset ultrafast -maxrate 3000k -bufsize 3000k \
    -pix_fmt yuv420p -g 50 \
    -vf "scale=1280:-1,format=yuv420p" \
    -ssrc 22222222 -payload_type 102 \
    -f rtp rtp://192.168.140.128:50180
```