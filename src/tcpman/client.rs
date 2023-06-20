use anyhow::Context;
use clap::builder::Str;
use tokio::io::{AsyncBufRead, AsyncWrite, BufReader};
use tokio::net::{TcpStream, ToSocketAddrs};

use super::{BlankConnectionMessage, Request};

pub struct BlankConnection<S> {
    stream: S,
    message_buf: Vec<u8>,
};

impl BlankConnection<BufReader<TcpStream>> {
    pub async fn connect(addr: impl ToSocketAddrs) -> anyhow::Result<Self> {
        todo!()
    }
}

impl<S> BlankConnection<S> {
    pub async fn request<'a>(mut self, req: Request<'a>) -> anyhow::Result<EstablishedConnection<S>>
        where S: AsyncBufRead + AsyncWrite + Unpin {
        self.message_buf.clear();
        serde_json::to_writer(&mut self.message_buf, &BlankConnectionMessage::Connect(req)).context("writing json")?;
        todo!()
    }

    pub async fn ping(&mut self) -> anyhow::Result<()> {
        todo!()
    }
}

pub struct EstablishedConnection<S>(S);

impl EstablishedConnection<BufReader<TcpStream>> {
    pub async fn connect(addr: impl ToSocketAddrs, req: &Request<'_>) -> anyhow::Result<Self> {
        todo!()
    }
}

impl<S> EstablishedConnection<S> {
    pub fn inner(self) -> S {
        self.0
    }
}

