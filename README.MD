# vulcan-relay

## Build
- Linux only
- You may need to change WebRTC listen/announce endpoints in `src/session.rs` 
```
git clone https://github.com/vulcan-fydp/vulcan-relay.git
cd vulcan-relay
cargo run
```

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

1. Navigate to wss://localhost:8443 and click through self-signed certificate warnings
2. Navigate to https://localhost:3001
