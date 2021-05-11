# vulcan-relay

## Build
- Linux only
```
git clone https://github.com/vulcan-fydp/vulcan-relay.git
cd vulcan-relay
cargo run -- run --cert-path config/cert.pem --key-path config/key.pem
```

To accept RTC connections on an interface other than localhost, use the `--rtc-ip` flag. 
RTC connections include ICE, DTLS, and RTP. 
If you use `0.0.0.0` or `::` as the RTC IP, specify an RTC Announce IP with the `--rtc-announce-ip` flag. 
In most cases, this will be a public IPv4 address. 
Ports 10000-59999 (TCP/UDP) must be open for ingress/egress traffic to the interface assigned to the RTC Announce IP. 

To dump the procedurally generated GraphQL signalling schema, run `cargo run -- schema`.

## Sample
- Sample web application using mediasoup-client which emulates a Vulcast/Web Client from the perspective of the Relay
- Each instance of the page you open will connect one Vulcast session and one Web Client session to a room called `ayush`
- You can open multiple tabs of the page to simulate multiple sessions connecting to the same room
    - Any Vulcast sessions after the first one will be denied since there is a limit of 1 per room
- You may need to change WebSocket endpoint address in `sample/src/index.ts`
```
cd sample
npm start
```

1. Navigate to https://localhost:8443 and click through self-signed certificate warnings
2. Navigate to https://localhost:3001
