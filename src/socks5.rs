use std::borrow::Cow;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use anyhow::{bail, Context};
use bytes::BufMut;
use smallvec::{smallvec, SmallVec};
use tokio::io::{AsyncWrite, AsyncWriteExt, AsyncBufRead, AsyncReadExt};
use num_enum::IntoPrimitive;

pub struct Acceptor<S> {
    stream: S,
    is_v6: bool,
}

impl<S> Acceptor<S> {
    pub async fn accept(mut stream: S) -> anyhow::Result<(Request<'static>, Self)>
        where
            S: AsyncBufRead + AsyncWrite + Unpin,
    {
        if stream.read_u8().await.context("Reading SOCKS version")? != 0x05 {
            bail!("invalid socks version");
        }

        let n_auth = stream.read_u8().await.context("Reading auth len")? as usize;

        let mut auth_methods: SmallVec<[u8; 1]> = smallvec![0u8; n_auth];
        stream.read_exact(&mut auth_methods).await.context("Reading auth methods")?;

        // Make sure the no auth is in the list
        if !auth_methods.contains(&0x00) {
            bail!("only no auth is supported");
        }

        // Respond OK
        stream.write_all(&[0x5, 0x0]).await.context("Writing auth response")?;

        // Read the request
        if stream.read_u8().await.context("Reading request SOCKS version")? != 0x05 {
            bail!("invalid socks version");
        }

        let cmd = stream.read_u8().await.context("Reading command")?;
        let _reserved = stream.read_u8().await.context("Reading reserved")?;
        let address = Address::parse(&mut stream).await.context("Parsing address")?;
        let port = stream.read_u16().await.context("Reading port")?;
        let is_v6 = match &address {
            Address::IP(IpAddr::V6(_)) => true,
            _ => false,
        };

        Ok((
            match cmd {
                0x01 => Request::Connect(address, port),
                0x02 => Request::Bind(address, port),
                0x03 => Request::UdpAssociate(address, port),
                _ => bail!("invalid command: {cmd}"),
            },
            Acceptor { stream, is_v6 },
        ))
    }

    pub async fn reply_success(mut self, bound: &Address<'_>, port: u16) -> anyhow::Result<S> where S: AsyncWrite + Unpin {
        self.reply(None, Some(bound), Some(port)).await?;
        Ok(self.stream)
    }

    pub async fn reply_failure(mut self, status: FailStatus) where S: AsyncWrite + Unpin {
        let _ = self.reply(Some(status), None, None).await;
    }

    async fn reply(&mut self, status: Option<FailStatus>, bound: Option<&Address<'_>>, port: Option<u16>) -> anyhow::Result<()> where S: AsyncWrite + Unpin {
        let mut buf = Vec::with_capacity(20);

        buf.extend_from_slice(&[0x5, status.map(|s| s.into()).unwrap_or(0x0), 0x0]);

        if let Some(addr) = bound {
            addr.write(&mut buf);
        } else {
            Address::default(self.is_v6).write(&mut buf);
        }

        self.stream.write_u16(port.unwrap_or(0)).await.context("Writing port")?;
        Ok(())
    }
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Address<'a> {
    IP(IpAddr),
    Domain(Cow<'a, str>),
}

impl<'a> Address<'a> {

    fn write(&self, w: &mut impl BufMut) {
        match self {
            Address::IP(IpAddr::V4(addr)) => {
                w.put_u8(0x1);
                w.put_slice(&addr.octets());
            }

            Address::IP(IpAddr::V6(addr)) => {
                w.put_u8(0x4);
                w.put_slice(&addr.octets());
            }

            Address::Domain(addr) => {
                w.put_u8(0x3);
                w.put_u8(addr.len().try_into().unwrap());
                w.put_slice(addr.as_bytes());
            }
        }
    }
}

impl Address<'static> {
    fn default(v6: bool) -> Self {
        if v6 {
            Address::IP(Ipv6Addr::UNSPECIFIED.into())
        } else {
            Address::IP(Ipv4Addr::UNSPECIFIED.into())
        }
    }

    async fn parse(s: &mut (impl AsyncBufRead + Unpin)) -> anyhow::Result<Self> {
        match s.read_u8().await.context("Reading address type")? {
            0x1 => {
                let mut buf = [0u8; 4];
                s.read_exact(&mut buf).await.context("Reading IPv4 address")?;
                Ok(Address::IP(buf.into()))
            }

            0x3 => {
                let len = s.read_u8().await.context("Reading domain length")? as usize;
                let mut buf = vec![0u8; len];
                s.read_exact(&mut buf).await.context("Reading domain")?;
                Ok(Address::Domain(String::from_utf8(buf)?.into()))
            }

            0x4 => {
                let mut buf = [0u8; 16];
                s.read_exact(&mut buf).await.context("Reading IPv6 address")?;
                Ok(Address::IP(buf.into()))
            }

            _ => bail!("invalid address type"),
        }
    }

}

pub enum Request<'a> {
    Connect(Address<'a>, u16),
    Bind(Address<'a>, u16),
    UdpAssociate(Address<'a>, u16),
}

#[derive(IntoPrimitive, Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailStatus {
    GeneralFailure = 1,
    NotAllowed = 2,
    NetworkUnreachable = 3,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    TtlExpired = 6,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
}
