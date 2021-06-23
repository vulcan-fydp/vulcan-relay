# ffmpeg_streamer
Tool for streaming directly to a Relay from FFmpeg.

## Usage
```
cargo run --example ffmpeg_streamer --signal-addr <addr> --token <token>
```
- `--signal-addr` is the address of the Relay signalling endpoint (e.g. `wss://localhost:8443`)
  - **DO NOT** use an IP address, you must use a hostname (or you will get TLS errors)
- `--token` is an access token issued by the Relay after registering a session

Leave the program open and record the tuples for audio/video plain transport options. Example:
```
[2021-06-07T22:24:38Z DEBUG ffmpeg_streamer] audio plain transport options: PlainTransportOptions { id: TransportId(66a6ee3a-f543-4ba9-b456-4f23264cbe3f), tuple: LocalOnly { local_ip: 192.168.140.128, local_port: 34637, protocol: Udp } }
[2021-06-07T22:24:38Z DEBUG ffmpeg_streamer] video plain transport options: PlainTransportOptions { id: TransportId(813055ea-344d-4106-86a3-0a96d8c6cb8f), tuple: LocalOnly { local_ip: 192.168.140.128, local_port: 37400, protocol: Udp } }
```
- `192.168.140.128` is the RTP IP
- `34637` is the RTP audio port
- `37400` is the RTP video port

Then, run
```
./stream.sh <file> <rtp-ip> <rtp-audio-port> <rtp-video-port>
```

For example,
```
./stream.sh The.Emoji.Movie.YIFY.BDRip.1080p.x264.mkv 192.168.140.128 34637 37400
```

Then, the audio/video will be streaming. You can use the sample to both get the
Vulcast token and also watch the resulting stream.
