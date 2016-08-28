pub trait DataChannelHandler {
    fn on_message(&mut self, message: &[u8]);
    fn on_close(&mut self);
}

pub trait DataChannel {
    fn send(&mut self, message: &[u8]);
}