/* eslint-disable no-console */
import { Device } from 'mediasoup-client';
// import { MediaKind, RtpCapabilities, RtpParameters } from 'mediasoup-client/lib/RtpParameters';
// import { DtlsParameters, TransportOptions, Transport } from 'mediasoup-client/lib/Transport';
// import { ConsumerOptions } from 'mediasoup-client/lib/Consumer';

import { WebSocketLink } from '@apollo/client/link/ws';
import { ApolloClient, InMemoryCache, gql } from '@apollo/client/core';

// please forgive me, this is the first thing i've ever written in typescript

enum Role {
    WebClient,
    Vulcast
}

async function session(role: Role) {
    const wsLink = new WebSocketLink({
        uri: 'ws://192.168.140.136:8080',
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
                transportOptions {
                    id,
                    dtlsParameters,
                    iceCandidates,
                    iceParameters
                },
                routerRtpCapabilities
            }
        }
        ` }).then(async (initParams) => {
            console.log("received server init", initParams);
            await device.load({ routerRtpCapabilities: initParams.data.init.routerRtpCapabilities });

            return client.mutate({
                mutation: gql`
                mutation($rtpCapabilities: RtpCapabilities!){
                    init(rtpCapabilities: $rtpCapabilities)
                }
                `,
                variables: {
                    rtpCapabilities: device.rtpCapabilities
                }
            })
        });

    switch (role) {
        case Role.WebClient:
            init_promise.then(initResult => {
                console.log("client init finished", initResult);


            });
            init_promise.then(initResult => {
                return client.mutate({
                    mutation: gql`
                    mutation($producerId: ProducerId!){
                        consume(producerId: $producerId) {
                            id,
                            producerId,
                            kind,
                            rtpParameters
                        }
                    }
                    `,
                    variables: {
                        producerId: "9d0c3ca8-0a18-4750-b3f2-362fcb33cdef"
                    }
                })

            });
            break;
        case Role.Vulcast:
            break;
    }

}

async function init() {
    await session(Role.Vulcast);

}

init()
