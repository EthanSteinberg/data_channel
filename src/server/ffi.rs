extern crate libc;
extern crate std;

enum RawProcessingThread {}
enum RawPeerConnection {}

#[repr(C)]
struct RawPeerConnectionObserver {
    deleter :extern fn(*mut libc::c_void),

    on_open :extern fn(*mut libc::c_void),
    on_close :extern fn(*mut libc::c_void),

    process_websocket_message: extern fn (*mut libc::c_void, *const libc::c_char, libc::c_int),
    process_data_channel_message: extern fn (*mut libc::c_void, *const libc::c_char, libc::c_int),

    data: *mut libc::c_void
}

#[repr(C)]
struct RawDataChannelOptions {
    ordered: bool,
    max_retransmit_time: libc::c_int,
    max_retransmits: libc::c_int
}


#[link(name = "DataChannelServer")]
extern {
    fn CreateProcessingThread() -> *mut RawProcessingThread;
    fn DeleteProcessingThread(thread: *mut RawProcessingThread);

    fn CreatePeerConnection(thread: *mut RawProcessingThread, observer: RawPeerConnectionObserver, options: RawDataChannelOptions) -> *mut RawPeerConnection;
    fn DeletePeerConnection(thread: *mut RawProcessingThread, peer: *mut RawPeerConnection);


    fn SendWebsocketMessage(thread: *mut RawProcessingThread, peer: *mut RawPeerConnection, message: *const libc::c_char, message_length: libc::c_int);
    fn SendDataChannelMessage(thread: *mut RawProcessingThread, peer: *mut RawPeerConnection, message: *const libc::c_char, message_length: libc::c_int);
}

pub struct ProcessingThread {
    data: *mut RawProcessingThread
}

impl ProcessingThread {
    pub fn new() -> ProcessingThread {
        let data = unsafe { CreateProcessingThread() };
        ProcessingThread {
            data: data
        }
    }
}

impl Drop for ProcessingThread {
    fn drop(&mut self) {
        unsafe {
           DeleteProcessingThread(self.data)
       }
    }
}


pub trait PeerConnectionObserver {
    fn on_open(&mut self);
    fn on_close(&mut self);

    fn process_websocket_message(&mut self, message: &[u8]);
    fn process_data_channel_message(&mut self, data: &[u8]);
}

fn wrap_peer_connection_observer<T: PeerConnectionObserver>(observer: T) -> RawPeerConnectionObserver {
    let boxed_observer = Box::new(observer);
    let data = Box::into_raw(boxed_observer);

    extern fn deleter<T>(data: *mut libc::c_void) {
        let actual_data = data as *mut T;
        unsafe {
            drop(Box::from_raw(actual_data));
        }
    }

    extern fn on_open<T: PeerConnectionObserver>(data: *mut libc::c_void) {
        let actual_data = data as *mut T;
         unsafe {
            (*actual_data).on_open();
        }
    }

    extern fn on_close<T: PeerConnectionObserver>(data: *mut libc::c_void) {
        let actual_data = data as *mut T;
        unsafe {
            (*actual_data).on_close();
        }
    }

    extern fn process_websocket_message<T: PeerConnectionObserver>(data: *mut libc::c_void, message: *const libc::c_char, message_length: libc::c_int) {
        let actual_data = data as *mut T;
        let slice = unsafe { std::slice::from_raw_parts(message as *const u8, message_length as usize) };
        unsafe {
            (*actual_data).process_websocket_message(slice);
        }
    }

    extern fn process_data_channel_message<T: PeerConnectionObserver>(data: *mut libc::c_void, message: *const libc::c_char, message_length: libc::c_int) {
        let actual_data = data as *mut T;
        let slice = unsafe { std::slice::from_raw_parts(message as *const u8, message_length as usize) };
        unsafe {
            (*actual_data).process_data_channel_message(slice);
        }
    }

    RawPeerConnectionObserver {
        data: data as *mut libc::c_void,

        deleter: deleter::<T>,

        on_open: on_open::<T>,
        on_close: on_close::<T>,

        process_websocket_message: process_websocket_message::<T>,
        process_data_channel_message: process_data_channel_message::<T>,
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DataChannelOptions {
    pub ordered: bool,
    pub max_retransmit_time: i32,
    pub max_retransmits: i32
}

pub struct PeerConnection<'a> {
    thread: &'a ProcessingThread,
    data: *mut RawPeerConnection
}

impl<'a> PeerConnection<'a> {
    pub fn new<T: PeerConnectionObserver>(thread: &ProcessingThread, observer: T, options: DataChannelOptions) -> PeerConnection {
        let raw_options = RawDataChannelOptions {
            ordered: options.ordered,
            max_retransmit_time: options.max_retransmit_time,
            max_retransmits: options.max_retransmits
        };

        let data = unsafe { CreatePeerConnection(thread.data, wrap_peer_connection_observer(observer), raw_options) };
        PeerConnection {
            data: data,
            thread: thread
        }
    }

    pub fn send_websocket_message(&mut self, message: &[u8]) {
        unsafe {
            SendWebsocketMessage(self.thread.data, self.data, message.as_ptr() as *const libc::c_char, message.len() as libc::c_int);
        }
    }

    pub fn send_data_channel_message(&mut self, message: &[u8]) {
        unsafe {
            SendDataChannelMessage(self.thread.data, self.data, message.as_ptr() as *const libc::c_char, message.len() as libc::c_int);
        }
    }
}

impl<'a> Drop for PeerConnection<'a> {
    fn drop(&mut self) {
        unsafe {
           DeletePeerConnection(self.thread.data, self.data)
       }
    }
}