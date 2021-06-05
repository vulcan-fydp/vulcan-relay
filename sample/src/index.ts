/* eslint-disable no-console */

import { Device } from 'mediasoup-client';
// import { MediaKind, RtpCapabilities, RtpParameters } from 'mediasoup-client/lib/RtpParameters';
// import { Transport } from 'mediasoup-client/lib/Transport';
// import { ConsumerOptions } from 'mediasoup-client/lib/Consumer';

import { WebSocketLink } from '@apollo/client/link/ws';
import { ApolloClient, NormalizedCacheObject, InMemoryCache, gql, FetchResult, HttpLink } from '@apollo/client/core';
import { SubscriptionClient } from "subscriptions-transport-ws";

declare global {
    interface HTMLCanvasElement {
        captureStream(frameRate?: number): MediaStream;
    }
    class CanvasCaptureMediaStreamTrack extends MediaStreamTrack {
        requestFrame(): void;
    }
}

// please forgive me, this is the first thing i've ever written in typescript

const sendPreview = document.querySelector('#preview-send') as HTMLVideoElement;
const receivePreview = document.querySelector('#preview-receive') as HTMLVideoElement;

function param(param: string) {
    return function (value?: string) {
        const input = document.getElementById(param) as HTMLInputElement;
        if (value) { input.value = value; }
        return input.value;
    }
}
const signalAddr = param("signalAddr");
const controlAddr = param("controlAddr");
const roomId = param("roomId");
const vulcastId = param("vulcastId");
const clientId = param("clientId");
const vulcastToken = param("vulcastToken");
const clientToken = param("clientToken");

let vulcastSub: SubscriptionClient | null = null;
let clientSub: SubscriptionClient | null = null;

(document.getElementById("registerVulcast") as HTMLButtonElement).addEventListener("click", async function () {
    let client = getControlConnection();
    let result = await client.mutate({
        mutation: gql`
                mutation($sessionId: ID!){
                    registerVulcastSession(sessionId: $sessionId) {
                        ... on SessionWithToken {
                            id,
                            accessToken
                        }
                    }
                }
                `,
        variables: {
            sessionId: vulcastId()
        }
    }).then(response => {
        let data = response.data.registerVulcastSession;
        console.log('registerVulcastSession', data.__typename, data);
        return data?.accessToken;
    });
    vulcastToken(result);
}, false);

(document.getElementById("unregisterVulcast") as HTMLButtonElement).addEventListener("click", async function () {
    let client = getControlConnection();
    await client.mutate({
        mutation: gql`
                mutation($sessionId: ID!){
                    unregisterSession(sessionId: $sessionId) {
                        ... on Session {
                            id
                        }
                    }
                }
                `,
        variables: {
            sessionId: vulcastId()
        }
    }).then(response => {
        let data = response.data.unregisterSession;
        console.log('unregisterSession', data.__typename, data);
        return data.id;
    });
}, false);
(document.getElementById("connectVulcast") as HTMLButtonElement).addEventListener("click", async function () {
    vulcastSub?.close();
    vulcastSub = await session(Role.Vulcast, vulcastToken());
}, false);
(document.getElementById("disconnectVulcast") as HTMLButtonElement).addEventListener("click", async function () {
    console.log("disconnectVulcast", vulcastSub);
    vulcastSub?.close();
}, false);

(document.getElementById("registerClient") as HTMLButtonElement).addEventListener("click", async function () {
    let client = getControlConnection();
    let result = await client.mutate({
        mutation: gql`
                mutation($sessionId: ID!, $roomId: ID!){
                    registerClientSession(sessionId: $sessionId, roomId: $roomId) {
                        ... on SessionWithToken {
                            id,
                            accessToken
                        }
                    }
                }
                `,
        variables: {
            sessionId: clientId(),
            roomId: roomId()
        }
    }).then(response => {
        let data = response.data.registerClientSession;
        console.log('registerClientSession', data.__typename, data);
        return data?.accessToken;
    });
    clientToken(result);
}, false);
(document.getElementById("unregisterClient") as HTMLButtonElement).addEventListener("click", async function () {
    let client = getControlConnection();
    await client.mutate({
        mutation: gql`
                mutation($sessionId: ID!){
                    unregisterSession(sessionId: $sessionId) {
                        ... on Session {
                            id
                        }
                    }
                }
                `,
        variables: {
            sessionId: clientId()
        }
    }).then(response => {
        let data = response.data.unregisterSession;
        console.log('unregisterSession', data.__typename, data);
        return data?.id;
    });
}, false);
(document.getElementById("connectClient") as HTMLButtonElement).addEventListener("click", async function () {
    clientSub?.close();
    clientSub = await session(Role.WebClient, clientToken());
}, false);
(document.getElementById("disconnectClient") as HTMLButtonElement).addEventListener("click", function () {
    console.log("disconnectClient", clientSub);
    clientSub?.close();
}, false);

(document.getElementById("registerRoom") as HTMLButtonElement).addEventListener("click", async function () {
    let client = getControlConnection();
    let result = await client.mutate({
        mutation: gql`
                mutation($vulcastSessionId: ID!, $roomId: ID!){
                    registerRoom(vulcastSessionId: $vulcastSessionId, roomId: $roomId) {
                        ... on Room {
                            id,
                        }
                    }
                }
                `,
        variables: {
            vulcastSessionId: vulcastId(),
            roomId: roomId()
        }
    }).then(response => {
        let data = response.data.registerRoom;
        console.log('registerRoom', data.__typename, data);
        return data?.id;
    });
}, false);
(document.getElementById("unregisterRoom") as HTMLButtonElement).addEventListener("click", async function () {
    let client = getControlConnection();
    let result = await client.mutate({
        mutation: gql`
                mutation($roomId: ID!){
                    unregisterRoom(roomId: $roomId) {
                        ... on Room {
                            id,
                        }
                    }
                }
                `,
        variables: {
            roomId: roomId()
        }
    }).then(response => {
        let data = response.data.unregisterRoom;
        console.log('unregisterRoom', data.__typename, data);
        return data?.id;
    });
    clientToken(result);
}, false);

// const canvas = document.querySelector('canvas') as HTMLCanvasElement;

sendPreview.onloadedmetadata = () => {
    sendPreview.play();
};
receivePreview.onloadedmetadata = () => {
    receivePreview.play();
};

let receiveMediaStream: MediaStream | undefined;

enum Role {
    WebClient = "WebClient",
    Vulcast = "Vulcast"
}

function jsonClone(x: Object) {
    return JSON.parse(JSON.stringify(x))
}

function getControlConnection() {
    const httpLink = new HttpLink({
        uri: controlAddr(),
    });
    return new ApolloClient({
        link: httpLink,
        cache: new InMemoryCache(),
    })
}

function getSignalConnection(token: string) {
    let sub = new SubscriptionClient(signalAddr(), {
        connectionParams: {
            token
        }
    });
    const wsLink = new WebSocketLink(sub);
    let client = new ApolloClient({
        link: wsLink,
        cache: new InMemoryCache(),
    })
    return { client, sub };
}

async function session(role: Role, token: string) {
    let { client, sub } = getSignalConnection(token);

    const device = new Device();

    // common setup for both Vulcast and WebClient paths
    let init_promise = client.query({ // query relay for init parameters
        query: gql`
        query {
            init {
                sendTransportOptions,
                recvTransportOptions, 
                routerRtpCapabilities
            }
        }
        ` }).then(async (initParams) => {
            console.log(role, "received server init", initParams.data);
            // load init params into device
            await device.load({ routerRtpCapabilities: jsonClone(initParams.data.init.routerRtpCapabilities) });

            // send init params back to relay
            return client.mutate({
                mutation: gql`
                mutation($rtpCapabilities: RtpCapabilities!){
                    init(rtpCapabilities: $rtpCapabilities)
                }
                `,
                variables: {
                    rtpCapabilities: device.rtpCapabilities
                }
            }).then(() => {
                return initParams;
            })
        }).then(async initParams => {
            // create bidirectional transport
            let sendTransport = device.createSendTransport(jsonClone(initParams.data.init.sendTransportOptions));
            sendTransport.on('connect', ({ dtlsParameters }, success) => {
                // this callback is called on first consume/produce to link transport to relay
                client.mutate({
                    mutation: gql`
                        mutation($dtlsParameters: DtlsParameters!){
                            connectSendTransport(dtlsParameters: $dtlsParameters) 
                        }
                        `,
                    variables: {
                        dtlsParameters: dtlsParameters
                    }
                }).then(response => {
                    console.log(role, "connected send transport", response.data);
                    success();
                })
            });
            let recvTransport = device.createRecvTransport(jsonClone(initParams.data.init.recvTransportOptions));
            recvTransport.on('connect', ({ dtlsParameters }, success) => {
                // this callback is called on first consume/produce to link transport to relay
                client.mutate({
                    mutation: gql`
                        mutation($dtlsParameters: DtlsParameters!){
                            connectRecvTransport(dtlsParameters: $dtlsParameters) 
                        }
                        `,
                    variables: {
                        dtlsParameters: dtlsParameters
                    }
                }).then(response => {
                    console.log(role, "connected recv transport", response.data);
                    success();
                })
            });
            return { sendTransport, recvTransport }
        });

    switch (role) {
        case Role.WebClient:
            init_promise.then(async ({ sendTransport, recvTransport }) => {
                sendTransport.on('producedata', ({ sctpStreamParameters }, success) => {
                    // this callback is called on produceData to request connection from relay
                    client.mutate({
                        mutation: gql`
                        mutation($sctpStreamParameters: SctpStreamParameters!){
                            produceData(sctpStreamParameters: $sctpStreamParameters) 
                        }
                        `,
                        variables: {
                            sctpStreamParameters
                        }
                    }).then(response => {
                        console.log(role, "produced data", response.data);
                        // the mutation returns a producerId, which we need to yield 
                        success({ id: response.data.produce_data });
                    })
                });

                receivePreview.srcObject = null;
                receiveMediaStream = undefined;

                // listen for when new media producers are available
                client.subscribe({
                    query: gql`
                    subscription {
                        producerAvailable
                    }
                    `
                }).subscribe((result: FetchResult<Record<string, any>>) => {
                    // callback is called when new producer is available
                    console.log(role, "producer available", result.data)

                    // request consumerOptions for new producer from relay
                    client.mutate({
                        mutation: gql`
                        mutation($producerId: ProducerId!){
                            consume(producerId: $producerId) 
                        }
                        `,
                        variables: {
                            producerId: result.data?.producerAvailable
                        }
                    }).then(async response => {
                        console.log(role, "consumed", response.data);
                        // use consumerOptions to connect to producer from relay
                        const consumer = await recvTransport.consume(response.data.consume);
                        console.log(role, "consumer created", consumer);

                        // display media streams
                        if (receiveMediaStream) {
                            if (consumer.track.kind == "video") {
                                receiveMediaStream.getVideoTracks().forEach(track => receiveMediaStream?.removeTrack(track));
                            } else if (consumer.track.kind == "audio") {
                                receiveMediaStream.getAudioTracks().forEach(track => receiveMediaStream?.removeTrack(track));
                            }
                            receiveMediaStream.addTrack(consumer.track);
                            receivePreview.srcObject = receiveMediaStream;
                        } else {
                            receiveMediaStream = new MediaStream([consumer.track]);
                            receivePreview.srcObject = receiveMediaStream;
                        }
                        return consumer.id;
                    }).then(consumerId => {
                        // the stream begins paused for technical reasons, request stream to resume
                        return client.mutate({
                            mutation: gql`
                            mutation($consumerId: ConsumerId!){
                                consumerResume(consumerId: $consumerId) 
                            }
                            `,
                            variables: {
                                consumerId: consumerId
                            }
                        })
                    }).then(response => {
                        console.log(role, "consumer resume", response.data);
                    });
                });

                // start producing data (this would be controller inputs in binary format)
                let dataProducer = await sendTransport.produceData({ ordered: false });
                dataProducer.on('open', () => {
                    let handle = setInterval(() => {
                        let data = "hello " + Math.floor(1000 * Math.random());
                        console.log(role, "send data", data);
                        if (dataProducer.closed) {
                            clearInterval(handle);
                            return;
                        }
                        dataProducer.send(data);
                    }, 10000);
                });
            });
            break;
        case Role.Vulcast:
            init_promise.then(async ({ sendTransport, recvTransport }) => {
                sendTransport.on('produce', ({ kind, rtpParameters }, success) => {
                    // this callback is called when produce is called to request connection from relay
                    client.mutate({
                        mutation: gql`
                        mutation($kind: MediaKind!, $rtpParameters: RtpParameters!){
                            produce(kind: $kind, rtpParameters: $rtpParameters) 
                        }
                        `,
                        variables: {
                            kind: kind,
                            rtpParameters: rtpParameters
                        }
                    }).then(response => {
                        console.log(role, "produced", response.data);
                        // mutation returns producerId which we must yield
                        success({ id: response.data.produce });
                    })
                });

                // create a webcam/mic combo for testing (would be cap card output from Vulcast)
                const mediaStream = await navigator.mediaDevices.getUserMedia({
                    audio: true,
                    video: {
                        width: {
                            ideal: 1270
                        },
                        height: {
                            ideal: 720
                        },
                        frameRate: {
                            ideal: 60
                        }
                    }
                });
                // const mediaStream = canvas.captureStream(0);
                // var videoTrack = mediaStream.getVideoTracks()[0] as CanvasCaptureMediaStreamTrack;
                // var ctx = canvas.getContext("2d") as CanvasRenderingContext2D;
                // function draw() {
                //     ctx.fillStyle = "#000000";
                //     ctx.fillRect(0, 0, 640, 480);
                //     ctx.font = "30px Arial";
                //     ctx.fillStyle = "#" + Math.floor(Math.random() * 16777215).toString(16);
                //     ctx.fillText(new Date().getTime() + "", 10, 30);
                //     videoTrack.requestFrame();
                //     window.requestAnimationFrame(draw);
                // }
                // window.requestAnimationFrame(draw);
                sendPreview.srcObject = mediaStream;

                // create producers for each media track
                const producers = [];
                for (const track of mediaStream.getTracks()) {
                    const producer = await sendTransport.produce({ track });
                    producers.push(producer);
                    console.log(role, `${track.kind} producer created: `, producer);
                }

                // listen for new data producers (web client controllers)
                client.subscribe({
                    query: gql`
                    subscription {
                        dataProducerAvailable
                    }
                    `
                }).subscribe((result: FetchResult<Record<string, any>>) => {
                    // callback is called when new data producer is available
                    console.log(role, "data producer available", result.data)

                    // request dataConsumerOptions from relay
                    client.mutate({
                        mutation: gql`
                        mutation($dataProducerId: DataProducerId!){
                            consumeData(dataProducerId: $dataProducerId) 
                        }
                        `,
                        variables: {
                            dataProducerId: result.data?.dataProducerAvailable
                        }
                    }).then(async response => {
                        console.log(role, "data consumed", response.data);
                        // use dataConsumerOptions to connect to dataProducer from relay
                        const dataConsumer = await recvTransport.consumeData(response.data.consumeData);
                        console.log(role, "data consumer created", dataConsumer);

                        // print every message we get from this dataProducer
                        dataConsumer.on('message', (message, ppid) => {
                            console.log(role, "recv data", ppid, message);
                        });
                    });
                });
            });
            break;
    }
    return sub;
}
