use anyhow::{bail, Context};
use async_shutdown::Shutdown;
use tokio::io::{BufReader, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::ctrl_c;
use tokio::spawn;

mod socks5;
mod tcpman;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    env_logger::init();

    let listener = TcpListener::bind("[::0]:6000").await.context("To bind")?;
    log::info!("Listening on {}", listener.local_addr()?);

    let shutdown = Shutdown::new();

    spawn(shutdown.wrap_cancel(serve_socks5(shutdown.clone(), listener)));

    ctrl_c().await.context("Waiting for Ctrl-C")?;

    log::info!("Shutting down...");
    shutdown.shutdown();
    shutdown.wait_shutdown_complete().await;
    log::info!("Shutdown complete");

    Ok(())
}

async fn serve_socks5(shutdown: Shutdown, listener: TcpListener) -> anyhow::Result<()> {
    while let Some(v) = shutdown.wrap_cancel(listener.accept()).await {
        let (stream, addr) = v.context("Accepting connection")?;
        log::debug!("Accepted connection from {addr}");

        let shutdown = shutdown.clone();
        spawn(async move {
            if let Some(Err(e)) = shutdown.wrap_cancel(handle_socks5_client(stream)).await {
                log::error!("Error handling connection from {addr}: {e:?}");
            }
            log::debug!("Disconnected: {addr}");
        });
    }

    Ok(())
}

async fn handle_socks5_client(stream: TcpStream) -> anyhow::Result<()> {
    use socks5::*;

    let (req, acceptor) = Acceptor::accept(BufReader::new(stream)).await.context("Accepting socks5 connection")?;
    log::info!("Proxying {req:?}");

    let upstream = match &req {
        Request::Connect(Address::Domain(addr), port) => TcpStream::connect((addr.as_ref(), *port)).await,
        Request::Connect(Address::IP(addr), port) => TcpStream::connect((*addr, *port)).await,
        _ => bail!("Invalid request"),
    };

    let (mut stream, mut upstream) = match upstream {
        Ok(upstream) => {
            let bound = upstream.local_addr().unwrap();
            log::info!("Connected to {req:?}");
            (acceptor.reply_success(&Address::IP(bound.ip()), bound.port()).await.context("Replying to socks5 conn")?, upstream)
        }

        Err(e) => {
            acceptor.reply_failure((&e).into()).await;
            return Err(e).with_context(|| format!("Connecting to {req:?}"));
        }
    };

    let (upload, download) = copy_bidirectional(&mut stream, &mut upstream).await.context("Copying data")?;
    log::debug!("Disconnecting from {req:?}, uploaded {upload} bytes, downloaded {download} bytes");
    Ok(())
}
