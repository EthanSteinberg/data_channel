#![feature(link_args)]

pub mod channel;

#[cfg(target_arch = "asmjs")]
pub mod client;

#[cfg(not(target_arch = "asmjs"))]
pub mod server;