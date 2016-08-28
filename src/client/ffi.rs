extern crate libc;
extern crate std;

use channel::DataChannelHandler;
use channel::DataChannel;

enum RawPeerConnection {}

#[no_mangle]
pub extern fn delete_data_channel_handler(handler: *mut Box<DataChannelHandler>) {
    unsafe {
        drop(Box::from_raw(handler));
    }
}

#[no_mangle]
pub extern fn on_close_data_channel_handler(handler: *mut Box<DataChannelHandler>) {
    unsafe {
        (*handler).on_close();
    }
}

#[no_mangle]
pub extern fn on_message_data_channel_handler(handler: *mut Box<DataChannelHandler>, message: *const libc::c_char, message_length: libc::c_int) {
    let slice = unsafe { std::slice::from_raw_parts(message as *const u8, message_length as usize) };
    unsafe {
        (*handler).on_message(slice);
    }
}

extern {
    fn CreatePeerConnection(
        server: *const libc::c_char,
        server_length: libc::c_int,
        port: libc::c_int,
        callback: extern fn(*mut RawPeerConnection, *mut libc::c_void) -> *mut Box<DataChannelHandler>,
        user_data: *mut libc::c_void,
        error_callback: extern fn(*const libc::c_char, libc::c_int, *mut libc::c_void),
        error_data: *mut libc::c_void);

    fn DeletePeerConnection(peer: *mut RawPeerConnection);
    fn SendPeerConnectionMessage(peer: *mut RawPeerConnection, message: *const libc::c_char, message_length: libc::c_int);
}

extern {
    fn emscripten_exit_with_live_runtime();
}

struct PeerConnection {
    data: *mut RawPeerConnection
}

impl PeerConnection {
    fn send(&mut self, message: &[u8]) {
        unsafe {
            SendPeerConnectionMessage(self.data, message.as_ptr() as *const libc::c_char, message.len() as libc::c_int);
        }
    }
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        unsafe {
            DeletePeerConnection(self.data)
        }
    }
}

struct ClientDataChannel {
    peer: std::rc::Rc<std::cell::RefCell<PeerConnection>>
}


impl DataChannel for ClientDataChannel {
    fn send(&mut self, message: &[u8]) {
        self.peer.borrow_mut().send(message);
    }
}

pub fn listen<C, E, H: DataChannelHandler + 'static>(server: &str, port: u16, callback: C, error_callback: E)
    where C: Fn(Box<DataChannel>) -> H, E: Fn(&str) {

    let boxed_callback = Box::new(callback);
    let callback_data = Box::into_raw(boxed_callback);

    extern fn callback_handler<C, H: DataChannelHandler +'static>(raw_peer: *mut RawPeerConnection, data: *mut libc::c_void) -> *mut Box<DataChannelHandler>
        where C: Fn(Box<DataChannel>) -> H, H: DataChannelHandler {
        let actual_data = data as *mut C;
        let peer = std::rc::Rc::new(std::cell::RefCell::new(PeerConnection {data: raw_peer}));
        let result = Box::new(ClientDataChannel { peer: peer.clone() });
        unsafe {
            let handler: H= (*actual_data)(result);
            drop(Box::from_raw(actual_data));

            let double_boxed_handler: Box<Box<DataChannelHandler>> = Box::new(Box::new(handler));

            Box::into_raw(double_boxed_handler)
        }
    }

    let boxed_error_callback = Box::new(error_callback);
    let error_callback_data = Box::into_raw(boxed_error_callback);


    extern fn error_callback_handler<E>(message: *const libc::c_char, message_length: libc::c_int, data: *mut libc::c_void)
        where E: Fn(&str) {
        let actual_data = data as *mut E;
        let slice = unsafe { std::slice::from_raw_parts(message as *const u8, message_length as usize) };
        let string = std::str::from_utf8(slice).unwrap();
        unsafe {
            (*actual_data)(string);
            drop(Box::from_raw(actual_data));
        }
    }

    unsafe {
        CreatePeerConnection(server.as_ptr() as *const libc::c_char, server.len() as libc::c_int, port as libc::c_int, callback_handler::<C, H>, callback_data as *mut libc::c_void, error_callback_handler::<E>, error_callback_data as *mut libc::c_void);
        emscripten_exit_with_live_runtime();
    }
}