mod config;
mod index_interface;
mod web_interface;

use crate::index_interface::IndexInterface;
use anyhow::Result;
use config::Config;
use futures::TryStreamExt;
use log_analyzer_transient_types::{Document, Field};
use rsyslog::parser::msg::LineRaw;
use rsyslog::parser::StructuredData;
use rsyslog::Message;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio_serde::formats::Cbor;
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = std::fs::read_to_string("config.yaml")?;
    let config: Config = serde_yaml::from_str(&config)?;
    let index_interface = IndexInterface::new(&config)?;

    tokio::spawn(start_data_listen(config.port.clone(), index_interface.clone()));
    tokio::spawn(start_syslog_listen_tcp(index_interface.clone()));
    tokio::spawn(start_syslog_listen_udp(index_interface.clone()));

    let routes = web_interface::build_routes(index_interface);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

async fn start_data_listen(port: String, index_interface: IndexInterface) {
    let tcp_listener = match tokio::net::TcpListener::bind("0.0.0.0:".to_string() + &port).await {
        Ok(tcp) => tcp,
        Err(e) => {
            tracing::error!("Failed to start listener: {}", e);
            panic!();
        }
    };
    loop {
        let (socket, _) = match tcp_listener.accept().await {
            Ok(tuple) => tuple,
            Err(e) => {
                tracing::error!("Failed to accept connection, {}", e);
                continue;
            }
        };
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

async fn start_syslog_listen_tcp(index_interface: IndexInterface) {
    let tcp_listener = match tokio::net::TcpListener::bind("0.0.0.0:601").await {
        Ok(tcp) => tcp,
        Err(e) => {
            tracing::error!("Failed to start listener: {}", e);
            panic!();
        }
    };
    tracing::info!("Started tcp syslog message");
    loop {
        let (socket, _) = match tcp_listener.accept().await {
            Ok(tuple) => tuple,
            Err(e) => {
                tracing::error!("Failed to accept connection, {}", e);
                continue;
            }
        };
        tokio::spawn(handle_syslog_connection(socket, index_interface.clone()));
    }
}

async fn handle_syslog_connection(tcp_stream: tokio::net::TcpStream, index_interface: IndexInterface) {
    let mut syslog_msg = String::new();
    let mut buf_reader = BufReader::new(tcp_stream);
    while buf_reader.read_line(&mut syslog_msg).await.unwrap() != 0 {
        process_syslog_message(&syslog_msg, &index_interface);
        syslog_msg.clear();
    }
}

async fn start_syslog_listen_udp(index_interface: IndexInterface) {
    let udp_socket = match tokio::net::UdpSocket::bind("0.0.0.0:514").await {
        Ok(tcp) => tcp,
        Err(e) => {
            tracing::error!("Failed to start listener: {}", e);
            panic!();
        }
    };
    tracing::info!("Started udp syslog message");
    let mut buf = [0u8; 2048];
    loop {
        let res = udp_socket.recv(&mut buf).await;
        match res {
            Ok(bytes) => {
                let msg = String::from_utf8_lossy(&buf[..bytes]);
                process_syslog_message(&msg, &index_interface);
            }
            Err(e) => tracing::error!("udp error: {}", e),
        }
    }
}

fn process_syslog_message(msg: &str, index_interface: &IndexInterface) {
    //type OneLineMessage<'a> = Message<'a, Option<&'a str>, Vec<StructuredData<'a>>, LineRaw<'a>>;
    //let message: OneLineMessage = match rsyslog::Message::parse(msg) {
    //    Ok(msg) => msg,
    //    Err(e) => {
    //        tracing::error!("syslog parse error: {}", e);
    //        return;
    //    }
    //};
    //tracing::warn!("recieved syslog: {}", msg);
    index_interface
        .save_doc(Document {
            fields: vec![Field {
                name: "syslog".into(),
                content: msg.into(),
            }],
        })
        .unwrap();
}
