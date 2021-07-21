# vulcan-relay

## Build
### Ad-hoc
- Linux only
```
git clone https://github.com/vulcan-fydp/vulcan-relay.git
cd vulcan-relay
cargo run 
```

- List all options by running `cargo run -- --help`.
- The addresses that the signal and control endpoints listen on are controlled with `--signal-addr` and 
`--control-addr` respectively. By default, they **listen on localhost only**.
	- The signal endpoint is a GraphQL endpoint over WebSockets, and the control endpoint is a GraphQL endpoint over HTTP.
	- By default, secure WebSockets and HTTPS are used for both endpoints. Thus, both endpoints require valid certificates. To elide this requirement, use the `--no-tls` flag.
	- You will not be able to connect to the signal endpoint over insecure WebSockets from a web browser.
- To accept RTC connections on an interface other than localhost, use the `--rtc-ip` flag. 
RTC connections include ICE, DTLS, and RTP. 
- If you use `0.0.0.0` or `::` as the RTC IP, specify an RTC Announce IP with the `--rtc-announce-ip` flag. 
In most cases, this will be a public IPv4 address. 
- Ports 10000-59999 (TCP/UDP) must be open for ingress/egress traffic to the interface assigned to the RTC Announce IP. 
- To dump the procedurally generated GraphQL signalling schema, run `cargo run --bin dump_signal_schema`.
- To dump the procedurally generated GraphQL control schema, run `cargo run --bin dump_control_schema`.

### Docker
- If using TLS, certificates must be mounted somewhere in the container and specified as arguments.
- `--network=host` is recommended (do not forward all 50,000 ports with docker proxy).
```
docker pull ghcr.io/vulcan-fydp/vulcan-relay:master
docker run -d vulcan-relay
```
Or, from source:
```
docker build -t vulcan-relay .
docker run -d vulcan-relay
```

Full example for clients connecting within private subnet 192.168.140.0/24:
```
docker run -it --init --rm --name relay --network=host -v /mnt/scratch/vulcan-relay/config:/config vulcan-relay --cert-path /config/cert.pem --key-path /config/key.pem --rtc-ip 0.0.0.0 --rtc-announce-ip 192.168.140.136 --signal-addr 0.0.0.0:8443 --control-addr 0.0.0.0:9443
```

## Sample
- Sample web application using mediasoup-client which emulates the backend and a Vulcast/Web Client from the perspective of the Relay
- You can open multiple tabs of the page to simulate multiple sessions connecting to the same room
- Open the debug console to see detailed logs
```
cd sample
npm start
```

1. Navigate to the signalling endpoint and click through self-signed certificate warnings (default https://localhost:8443)
2. Navigate to https://localhost:3001
3. Set Signal Address and Control Address to the Relay instance
4. Open the debug console
5. Press control buttons to execute actions

All controls will use the parameter fields in the input boxes. Token fields will autofill when registering. 

Example flow:

1. Set the following initial parameters
```
Signal Address: wss://192.168.140.128:8443
Control Address: https://192.168.140.128:9443
Room ID: ayush
Vulcast Session ID: 0
Client Session ID: 1
```

2. Register Vulcast
- This will fill the Vulcast Token field
3. Register Room
- This will fill the Client Token field
4. Register Client
5. Connect Vulcast
6. Connect Client

## Debugging
To dump all relevant logs and backtrace on crashes, set the following envvars:
```
RUST_LOG=vulcan_relay=trace,mediasoup=trace RUST_BACKTRACE=1 ./vulcan-relay ...
```