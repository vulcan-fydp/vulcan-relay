/* eslint-disable no-console */
import { Device } from 'mediasoup-client';
// import { MediaKind, RtpCapabilities, RtpParameters } from 'mediasoup-client/lib/RtpParameters';
import { Transport } from 'mediasoup-client/lib/Transport';
// import { ConsumerOptions } from 'mediasoup-client/lib/Consumer';

import { WebSocketLink } from '@apollo/client/link/ws';
import { ApolloClient, InMemoryCache, gql, FetchResult } from '@apollo/client/core';

// please forgive me, this is the first thing i've ever written in typescript

const sendPreview = document.querySelector('#preview-send') as HTMLVideoElement;
const receivePreview = document.querySelector('#preview-receive') as HTMLVideoElement;

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

async function session(role: Role) {
    const wsLink = new WebSocketLink({
        uri: 'wss://192.168.140.136:8443',
        options: {
            reconnect: true,
            connectionParams: {
                "roomId": "ayush",
                "role": role
            }
        }
    });

    const client = new ApolloClient({
        link: wsLink,
        cache: new InMemoryCache(),
    })

    const device = new Device();

    let init_promise = client.query({
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
            await device.load({ routerRtpCapabilities: jsonClone(initParams.data.init.routerRtpCapabilities) });

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
                let sendTransport = device.createSendTransport(jsonClone(initParams.data.init.sendTransportOptions));
                sendTransport.on('connect', ({ dtlsParameters }, success) => {
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
                return {sendTransport, recvTransport}
        });

    switch (role) {
        case Role.WebClient:
            init_promise.then(async ({sendTransport: _sendTransport, recvTransport}) => {
                client.subscribe({
                    query: gql`
                    subscription {
                        producerAvailable
                    }
                    `
                }).subscribe((result: FetchResult<Record<string, any>>) => {
                    console.log(role, "producer available", result.data)
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
                        const consumer = await (recvTransport as Transport).consume(response.data.consume);
                        console.log(role, "consumer created", consumer);

                        if (receiveMediaStream) {
                            receiveMediaStream.addTrack(consumer.track);
                            receivePreview.srcObject = receiveMediaStream;
                        } else {
                            receiveMediaStream = new MediaStream([consumer.track]);
                            receivePreview.srcObject = receiveMediaStream;
                        }
                        return consumer.id;
                    }).then(consumerId => {
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
            });
            break;
        case Role.Vulcast:
            init_promise.then(async ({sendTransport, recvTransport: _recvTransport}) => {
                sendTransport.on('produce', ({ kind, rtpParameters }, success) => {
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
                        success({ id: response.data.produce });
                    })
                });

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
                sendPreview.srcObject = mediaStream;

                const producers = [];
                for (const track of mediaStream.getTracks()) {
                    const producer = await sendTransport.produce({ track });
                    producers.push(producer);
                    console.log(role, `${track.kind} producer created: `, producer);
                }
            });
            break;
    }
}

async function init() {
    await session(Role.WebClient);
    await session(Role.Vulcast);
}

init()
