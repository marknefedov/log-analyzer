use crate::config::{Config, FieldType};
use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tantivy::schema::{Field, Schema, INDEXED, STORED, TEXT};
use tantivy::{Document, Index, IndexReader, IndexWriter, ReloadPolicy};

pub struct IndexInterface {
    index_writer: Arc<RwLock<IndexWriter>>,
    index_reader: Arc<RwLock<IndexReader>>,
    field_lookup: Arc<RwLock<HashMap<String, Field>>>,
}

impl IndexInterface {
    pub fn new(config: &Config) -> Result<Self> {
        let mut field_lookup = HashMap::new();
        let mut schema_builder = Schema::builder();
        for field in config.schema.fields.iter() {
            match field.field_type {
                FieldType::Text => {
                    field_lookup.insert(field.name.to_owned(), schema_builder.add_text_field(&field.name, STORED | TEXT));
                }
                FieldType::Date => {
                    field_lookup.insert(field.name.to_owned(), schema_builder.add_date_field(&field.name, STORED | INDEXED));
                }
                FieldType::Integer64 => {
                    field_lookup.insert(field.name.to_owned(), schema_builder.add_i64_field(&field.name, STORED | INDEXED));
                }
                FieldType::Unsigned64 => {
                    field_lookup.insert(field.name.to_owned(), schema_builder.add_u64_field(&field.name, STORED | INDEXED));
                }
                FieldType::Float64 => {
                    field_lookup.insert(field.name.to_owned(), schema_builder.add_f64_field(&field.name, STORED | INDEXED));
                }
            }
        }
        let schema = schema_builder.build();
        let index = Index::create_in_dir(&config.storage_path, schema.clone())?;
        let index_writer = Arc::new(RwLock::new(index.writer(50_000_000)?));
        let index_writer2 = index_writer.clone();
        thread::spawn(move || {
            index_writer2.write().commit();
            thread::sleep(Duration::from_millis(500));
        });

        let index_reader = index.reader_builder().reload_policy(ReloadPolicy::OnCommit).try_into()?;
        let index_reader = Arc::new(RwLock::new(index_reader));

        let field_lookup = Arc::new(RwLock::new(field_lookup));

        Ok(Self {
            index_writer,
            index_reader,
            field_lookup,
        })
    }

    pub fn save_doc(&self) {
        let document = Document::new();

        self.index_writer.read().add_document(document);
    }

    pub fn search_everythig(&self) {
        let searcher = self.index_reader.read().searcher();
    }
}
