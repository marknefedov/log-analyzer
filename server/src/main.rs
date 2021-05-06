mod config;
mod index_interface;
mod web_interface;

use crate::config::FieldType;
use crate::index_interface::IndexInterface;
use anyhow::Result;
use config::Config;
use std::collections::HashMap;
use tantivy::schema::{Schema, INDEXED, STORED, TEXT};
use tantivy::Index;

#[tokio::main]
async fn main() -> Result<()> {
    let config = std::fs::File::open("config.yaml")?;
    let config: Config = serde_yaml::from_reader(config)?;
    let index_interface = IndexInterface::new(&config)?;
    Ok(())
}
