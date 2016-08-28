var LibraryDataChannelClient = {
    $DataChannelClient: {
        connections: {},
        counter: 1,

        connectToServer: function(server, port) {
            return new Promise(function(resolve, reject) {
                window.RTCPeerConnection = window.RTCPeerConnection ||
                    window.mozRTCPeerConnection || window.webkitRTCPeerConnection;
                window.RTCIceCandidate = window.RTCIceCandidate ||
                    window.mozRTCIceCandidate || window.webkitRTCIceCandidate;
                window.RTCSessionDescription = window.RTCSessionDescription ||
                    window.mozRTCSessionDescription || window.webkitRTCSessionDescription;

                var peerConnectionConfig = {
                    'optional': [{
                        'DtlsSrtpKeyAgreement': true
                    }],
                    'iceServers': [{
                        'url': 'stun:stun.l.google.com:19302'
                    }]
                };

                var peerConnection = new RTCPeerConnection(peerConnectionConfig);
                var socket = new WebSocket('ws://' + server + ':' + port);

                socket.onclose = function(close) {
                    reject('Closed with reason "' + close.reason + '" and code ' + close.code);
                }

                peerConnection.ondatachannel =
                    function(event) {
                        console.log('Got channel ', event);
                        resolve(event.channel);
                    }

                peerConnection.onicecandidate =
                    function(event) {
                        console.log("GOT ICE CANDIDATE", event);
                        if (event.candidate != null) {
                            var response = {
                                type: 'icecandidate',
                                ice: event.candidate.toJSON()
                            };
                            socket.send(JSON.stringify(response));
                        }
                    }

                function processOffer(sdi) {
                    var actual_sdi = new RTCSessionDescription(sdi);
                    peerConnection.setRemoteDescription(actual_sdi)
                        .then(function() {
                            console.log('sdi is now set, creating answer');
                            return peerConnection.createAnswer();
                        })
                        .then(function(sdi) {
                            console.log('have answer, setting');
                            peerConnection.setLocalDescription(sdi).then(function() {
                                console.log('sending answer back');

                                var response = {
                                    type: 'answer',
                                    sdi: sdi.toJSON()
                                };
                                socket.send(JSON.stringify(response));
                            });
                        });
                }

                function processIceCandidate(ice) {
                    var actual_ice = new RTCIceCandidate(ice);
                    peerConnection.addIceCandidate(actual_ice).then(function() {
                        console.log('ice added');
                    });
                }

                socket.onmessage = function(event) {
                    var reader = new FileReader();
                    reader.onload = function() {
                        var text = reader.result;
                        console.log('Got text', text);
                        var message = JSON.parse(text);
                        console.log('GOT', message);
                        if (message.type === 'offer') {
                            processOffer(message.sdi);
                        } else if (message.type === 'icecandidate') {
                            processIceCandidate(message.ice);
                        } else {
                            console.log('INVALID TYPE ', message.type);
                        }
                    }
                    reader.readAsText(event.data);
                }
            });
        }
    },

    CreatePeerConnection: function(server, server_length, port, callback, user_data, error_callback, error_data) {
        server_bytes = Module.HEAPU8.slice(server, server + server_length);
        server = String.fromCharCode.apply(String, server_bytes);
        console.log('lol', server);
        DataChannelClient.connectToServer(server, port).then(function(channel) {
            var id = DataChannelClient.counter++;
            DataChannelClient.connections[id] = channel;
            channel.binaryType = 'arraybuffer';
            var handler = Runtime.dynCall('iii', callback, [id, user_data]);
            channel.native_handler = handler;

            channel.onmessage = function(event) {
                console.log('Got message', event);
                var ptr = Module._malloc(event.data.length);

                writeStringToMemory(event.data, ptr, true);

                Module.ccall('on_message_data_channel_handler', 'void', ['number', 'number', 'number'], [channel.native_handler, ptr, event.data.length]);
                Module._free(ptr);
            };

            channel.onclose = function(event) {
                console.log('Got close', event);
                Module.ccall('on_close_data_channel_handler', 'void', ['number'], [channel.native_handler]);
            }

        }, function(error) {
            console.log('got error', error);

            var ptr = Module._malloc(error.length);
            writeStringToMemory(error, ptr, true);
            Runtime.dynCall('viii', error_callback, [ptr, error.length, error_data]);
            Module._free(ptr);
        });
    },

    DeletePeerConnection: function(peer_handle) {
        var channel = DataChannelClient.connections[peer_handle];
        if (channel.native_handler != null) {
            Module.ccall('delete_data_channel_handler', 'void', ['number'], [channel.native_handler]);
        }
        channel.onmessage = null;
        channel.onclose = null;
        channel.close();
        DataChannelClient.connections[peer_handle] = null;
    },


    SetOnCloseCallback: function(peer_handle, callback, user_data) {
        var channel = DataChannelClient.connections[peer_handle];

    },

    SendPeerConnectionMessage: function(peer_handle, message, message_length) {
        var channel = DataChannelClient.connections[peer_handle];
        channel.send(Module.HEAPU8.slice(message, message + message_length));
    },
};

autoAddDeps(LibraryDataChannelClient, '$DataChannelClient');
mergeInto(LibraryManager.library, LibraryDataChannelClient);