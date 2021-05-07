mod config;
mod index_interface;
mod web_interface;

use crate::index_interface::IndexInterface;
use anyhow::Result;
use config::Config;
use futures::TryStreamExt;
use log_analyzer_transient_types::Document;
use tokio::net::{TcpListener, TcpStream};
use tokio_serde::formats::Cbor;
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = std::fs::read_to_string("config.yaml")?;
    let config: Config = serde_yaml::from_str(&config)?;
    let index_interface = IndexInterface::new(&config)?;
    let tcp_listener = tokio::net::TcpListener::bind("0.0.0.0:".to_string() + &config.port.to_string()).await?;
    tokio::spawn(start_data_listen(tcp_listener, index_interface.clone()));
    let routes = warp::any().map(|| "Hello, World!");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

async fn start_data_listen(tcp_listener: TcpListener, index_interface: IndexInterface) -> Result<()> {
    loop {
        let (socket, _) = tcp_listener.accept().await?;
        tokio::spawn(handle_connection(socket, index_interface.clone()));
    }
}

async fn handle_connection(tcp_stream: tokio::net::TcpStream, index_interface: IndexInterface) -> Result<()> {
    let length_delimited = tokio_util::codec::FramedRead::new(tcp_stream, tokio_util::codec::LengthDelimitedCodec::new());
    let mut deserialized = tokio_serde::SymmetricallyFramed::<FramedRead<TcpStream, LengthDelimitedCodec>, Document, Cbor<Document, Document>>::new(length_delimited, Cbor::default());
    while let Some(msg) = deserialized.try_next().await? {
        tracing::trace!("GOT: {:?}", msg);
        if let Err(e) = index_interface.save_doc(msg) {
            tracing::warn!("Failed to save doc! error: {}", e);
        }
    }
    Ok(())
}
