use std::borrow::Cow;
use serde::{Serialize, Deserialize};

pub mod client;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request<'a> {
    Tcp {
        addr: Cow<'a, str>,
        port: u16,
        initial_data: Option<Cow<'a, [u8]>>,
    },

    Udp {
        addr: Cow<'a, str>,
        port: u16,
        initial_data: Cow<'a, [u8]>,
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum BlankConnectionMessage<'a> {
    Ping,
    Connect(Request<'a>),
}