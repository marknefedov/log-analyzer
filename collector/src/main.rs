mod file_watcher;

use crate::file_watcher::FileWatcher;
use futures::SinkExt;
use log_analyzer_transient_types::Document;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::mpsc::unbounded_channel;
use tokio_serde::formats::Cbor;
use tokio_util::codec::{FramedWrite, LengthDelimitedCodec};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let config = std::fs::read_to_string("collector_config.toml").unwrap();
    let config: Config = toml::from_str(&config).unwrap();

    let (tx, mut rx) = unbounded_channel();

    let socket = TcpStream::connect(config.server_address).await.unwrap();
    let length_delimited = FramedWrite::new(socket, LengthDelimitedCodec::new());
    let mut serialized_channel = tokio_serde::SymmetricallyFramed::<FramedWrite<TcpStream, LengthDelimitedCodec>, Document, Cbor<Document, Document>>::new(length_delimited, Cbor::default());

    for file in config.watched_files.iter() {
        let file_watcher = FileWatcher::new(&file.file, tx.clone(), &file.regex);
        file_watcher.start_watch();
    }

    while let Some(doc) = rx.recv().await {
        tracing::debug!("Sending doc: {:?}", doc);
        serialized_channel.send(doc).await.unwrap();
    }
}

#[derive(Serialize, Deserialize)]
struct FileWatcherConfig {
    file: String,
    regex: String,
}

#[derive(Serialize, Deserialize)]
struct Config {
    server_address: String,
    watched_files: Vec<FileWatcherConfig>,
}
