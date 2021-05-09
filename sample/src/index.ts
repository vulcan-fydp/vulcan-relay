/* eslint-disable no-console */
import { Device } from 'mediasoup-client';
// import { MediaKind, RtpCapabilities, RtpParameters } from 'mediasoup-client/lib/RtpParameters';
// import { Transport } from 'mediasoup-client/lib/Transport';
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
            // auth token (for now, just a JSON object)
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
                    setInterval(() => {
                        let data = "hello " + Math.floor(1000 * Math.random());
                        console.log(role, "send data", data);
                        dataProducer.send(data);
                    }, 1000);
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
}

async function init() {
    await session(Role.WebClient);
    await session(Role.Vulcast);
}

init()
