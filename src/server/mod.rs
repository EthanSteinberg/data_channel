extern crate ws;
extern crate std;

use channel::DataChannelHandler;
use channel::DataChannel;

mod ffi;

struct Observer {
    sender: std::sync::mpsc::Sender<Message>,

    id: u64
}

impl ffi::PeerConnectionObserver for Observer {
    fn on_open(&mut self) {
        self.sender.send(Message::OnDataChannelOpen(self.id)).unwrap();
    }
    fn on_close(&mut self) {
        self.sender.send(Message::OnDataChannelClose(self.id)).unwrap();
    }

    fn process_websocket_message(&mut self, message: &[u8]) {
        println!("Trying to send {}", std::str::from_utf8(message).unwrap());
        self.sender.send(Message::SendWebsocketMessage(self.id, Vec::from(message))).unwrap();
    }
    fn process_data_channel_message(&mut self, message: &[u8]) {
        println!("Forwarding data channel message");
        self.sender.send(Message::GotDataChannelMessage(self.id, Vec::from(message))).unwrap();
    }
}

enum Message {
    OnWebsocketOpen(u64, ws::Sender),
    OnDataChannelOpen(u64),

    OnWebsocketClose(u64),
    OnDataChannelClose(u64),

    SendWebsocketMessage(u64, Vec<u8>),
    SendDataChannelMessage(u64, Vec<u8>),

    GotWebsocketMessage(u64, Vec<u8>),
    GotDataChannelMessage(u64, Vec<u8>),

    Close(u64),

}

struct Item<'a, H: DataChannelHandler> {
    sender: ws::Sender,
    peer: ffi::PeerConnection<'a>,
    handler: Option<H>,
    channel: Option<ServerDataChannel>,
}

struct ServerDataChannel {
    id: u64,
    sender: std::sync::mpsc::Sender<Message>,
}

impl DataChannel for ServerDataChannel {
    fn send(&mut self, message: &[u8]) {
        self.sender.send(Message::SendDataChannelMessage(self.id, Vec::from(message))).unwrap();
    }
}

impl Drop for ServerDataChannel {
    fn drop(&mut self) {
        self.sender.send(Message::Close(self.id)).unwrap();
    }
}

struct WsHandler {
    sender: std::sync::mpsc::Sender<Message>,

    id: u64
}

impl ws::Handler for WsHandler {
    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        self.sender.send(Message::GotWebsocketMessage(self.id, msg.into_data())).unwrap();
        Ok(())
    }

    fn on_close(&mut self, _: ws::CloseCode, _: &str) {
        self.sender.send(Message::OnWebsocketClose(self.id)).unwrap();
    }
}

pub fn listen<H, B: DataChannelHandler>(host: &str, port: u16, mut connection_handler: H)
    where H: FnMut(Box<DataChannel>) -> B {

    let darn = ffi::DataChannelOptions {
        ordered: true,
        max_retransmit_time: -1,
        max_retransmits: -1
    };

    let (tx, rx) = std::sync::mpsc::channel();

    let ws_tx = tx.clone();
    let temp_host = host.to_owned();
    std::thread::spawn(move || {
        let mut counter = 0;
        ws::listen((temp_host.as_str(), port), |out| {
            let id = counter;
            counter += 1;

            ws_tx.send(Message::OnWebsocketOpen(id.clone(), out)).unwrap();
            let ws_tx = ws_tx.clone();
            WsHandler {sender: ws_tx.clone(), id: id.clone() }
        }).unwrap();
    });

    let thread = ffi::ProcessingThread::new();

    let mut connections: std::collections::HashMap<u64, Item<B>> = std::collections::HashMap::new();
    loop {
        let item = rx.recv().unwrap();

        match item {
            Message::OnWebsocketOpen(id, sender) => {

                let channel = ServerDataChannel {
                    sender: tx.clone(),
                    id: id
                };

                let observer = Observer {
                    sender: tx.clone(),
                    id: id
                };

                let peer = ffi::PeerConnection::new(&thread, observer, darn);
                let item = Item {
                    sender: sender,
                    peer: peer,
                    handler: None,
                    channel: Some(channel)
                };

                connections.insert(id, item);
            }
            Message::OnDataChannelOpen(id) => {
                let item = connections.get_mut(&id).unwrap();

                let boxed_channel = Box::new(item.channel.take().unwrap());

                let handler = connection_handler(boxed_channel);
                item.handler = Some(handler);
            }
            Message::OnWebsocketClose(id) => {
                match connections.get_mut(&id) {
                    Some(item) => {
                        match item.handler.as_mut() {
                            Some(handler) => {
                                handler.on_close()
                            }

                            None => {}
                        }
                    }

                    None => {}
                }
            }
            Message::OnDataChannelClose(id) => {
                println!("Closing");

                match connections.get_mut(&id) {
                    Some(item) => {
                        item.handler.as_mut().unwrap().on_close();
                        item.sender.close(ws::CloseCode::Normal).unwrap();
                    }

                    None => {}
                }
                println!("Done closing");
            }
            Message::GotWebsocketMessage(id, message) => {
                let item = connections.get_mut(&id).unwrap();
                item.peer.send_websocket_message(message.as_slice());
            }
            Message::GotDataChannelMessage(id, message) => {
                let item = connections.get_mut(&id).unwrap();
                item.handler.as_mut().unwrap().on_message(message.as_slice());
            }
            Message::SendWebsocketMessage(id, message) => {
                let item = connections.get_mut(&id).unwrap();
                item.sender.send(message.as_slice()).unwrap();
            }
            Message::SendDataChannelMessage(id, message) => {
                println!("Trying to forward messag");
                match connections.get_mut(&id) {
                    None => {}
                    Some(item) => {
                        item.peer.send_data_channel_message(message.as_slice());
                    }
                }

                println!("Done forwarding");
            }

            Message::Close(id) => {

                {
                    let item = connections.get_mut(&id).unwrap();
                    item.sender.close(ws::CloseCode::Normal).unwrap();
                }

                connections.remove(&id);
            }
        }
    }
}