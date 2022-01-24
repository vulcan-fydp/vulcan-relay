# vulcan-relay

## Build from source (Linux only)
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

## Docker
- If the repository is private, authenticate with `docker login ghcr.io -u USERNAME` using a personal access token. The token must have the `read:packages` scope.


### Windows (from repository)
- On Windows, you will only be able to access the relay locally. 

1. Open a command prompt and run `ipconfig`. Use the IPv4 address of the Hyper-V Virtual Ethernet Adapter as `RTC_ANNOUNCE_IP` below.

2. Run the following command:
```
docker run -it --init --rm \
	-p 8443:8443 \
	-p 9443:9443 \
	-p 10000-10010:10000-10010/udp \
	ghcr.io/vulcan-fydp/vulcan-relay:master \
	--rtc-ip 0.0.0.0 \
	--rtc-announce-ip <RTC_ANNOUNCE_IP> \
	--signal-addr 0.0.0.0:8443 \
	--control-addr 0.0.0.0:9443 \
	--rtc-ports-range-min 10000 \
	--rtc-ports-range-max 10010 \
	--no-tls
```

### Linux (from repository)
1. Run the following command:
```
docker run -it --init --rm \
	--network=host \
	ghcr.io/vulcan-fydp/vulcan-relay:master \
	--rtc-ip 0.0.0.0 \
	--rtc-announce-ip <RTC_ANNOUNCE_IP> \
	--signal-addr 0.0.0.0:8443 \
	--control-addr 0.0.0.0:9443 \
	--no-tls
```

If you don't know what to use for `RTC_ANNOUNCE_IP`, see the troubleshooting section.

### From Dockerfile
Build the Dockerfile with:
```
docker build -t vulcan-relay .
```
Substitute the repository image for your local image in the above commands for Windows and Linux.

### Enabling TLS
- To encrypt all endpoints, you must provide a valid TLS certificate. 
- If using TLS, certificates must be mounted somewhere in the container and specified as arguments.
- You must not specify `--no-tls` as an argument when specifying certificates to use.

e.g. Linux:
```bash
# Put your certificates in a folder.
$ ls ~/certs
cert.pem	key.pem
$ docker run -it --init --rm \
	--network=host \
	# Mount your certificates folder in the container under /config.
 	-v ~/certs:/certs
	ghcr.io/vulcan-fydp/vulcan-relay:master \
	# Tell the relay to use these certificates.
	--cert-path /certs/cert.pem \
	--key-path /certs/key.pem \
	--rtc-ip 0.0.0.0 \
	--rtc-announce-ip <ANNOUNCE_IP> \
	--signal-addr 0.0.0.0:8443 \
	--control-addr 0.0.0.0:9443 \
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
4. Register Client
- This will fill the Client Token field
5. Connect Vulcast
6. Connect Client

## Troubleshooting

### certificate parse error
```
thread 'main' panicked at 'error binding to 0.0.0.0:8443: certificate parse error'
```
- Make sure the certificates pointed to by `--cert-path` and `--key-path` exist and are valid
- If you are using the prebuilt Docker container, make sure the certificates are mounted in the container

### I can't connect to the relay
- Try using the sample app to connect, make sure the signal address and 
control address are set to whatever the relay prints when you start it up
- If that doesn't work, then make sure the `--signal-addr`, `--control-addr`, `--rtc-ip` and `--rtc-announce-ip` 
relay parameters are correct (use the situations below to figure out how to assign them)
- If you are in Situation 2 or 3 below, make sure your firewall is not blocking the ports

#### Situation 1: Connecting from local machine
- You can omit all the above arguments and leave them at their defaults

#### Situation 2: Connecting to another machine on LAN (including WSL 2, Hyper-V, VM)
- The relay machine and the machine using the sample app share a common LAN
- Find the IP of the relay machine on the adapter connected to the common LAN
- If you are using Hyper-V, this will be the address on the virtual `Hyper-V Ethernet` adapter
- If you are using a VM, this will be the address on some Host only or NAT virtual Ethernet adapter

- `--signal-addr 0.0.0.0:8443 --control-addr 0.0.0.0:9443 --rtc-ip 0.0.0.0 --rtc-announce-ip <RELAY_LAN_IP>`

#### Situation 3: Connecting across the internet
- Find the public IPv4 address of your relay machine
- Open ports 10000-59999 for both UDP and TCP traffic

- `--signal-addr 0.0.0.0:8443 --control-addr 0.0.0.0:9443 --rtc-ip 0.0.0.0 --rtc-announce-ip <RELAY_PUBLIC_IPV4_IP>`

### Everything is broken
Run the relay with the following arguments to enable full logging then have Gordon look at it
```
RUST_LOG=vulcan_relay=trace,mediasoup=debug RUST_BACKTRACE=1 ./vulcan-relay ... -log-tags info message rtcp rtp rtx sctp srtp
```
