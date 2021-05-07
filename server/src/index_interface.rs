use crate::config::{Config, FieldType};
use anyhow::Result;
use chrono::DateTime;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tantivy::collector::{Count, TopDocs};
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, INDEXED, STORED, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};

#[derive(Clone)]
pub struct IndexInterface {
    index: Arc<RwLock<Index>>,
    schema: Arc<RwLock<Schema>>,
    index_writer: Arc<RwLock<IndexWriter>>,
    index_reader: Arc<RwLock<IndexReader>>,
    field_lookup: Arc<RwLock<HashMap<String, (Field, FieldType)>>>,
}

impl IndexInterface {
    pub fn new(config: &Config) -> Result<Self> {
        let mut field_lookup = HashMap::new();
        let mut schema_builder = Schema::builder();
        for field in config.schema.fields.iter() {
            match field.field_type {
                FieldType::Text => {
                    field_lookup.insert(field.name.to_owned(), (schema_builder.add_text_field(&field.name, STORED | TEXT), FieldType::Text));
                }
                FieldType::Date => {
                    field_lookup.insert(field.name.to_owned(), (schema_builder.add_date_field(&field.name, STORED | INDEXED), FieldType::Date));
                }
                FieldType::Integer64 => {
                    field_lookup.insert(field.name.to_owned(), (schema_builder.add_i64_field(&field.name, STORED | INDEXED), FieldType::Integer64));
                }
                FieldType::Unsigned64 => {
                    field_lookup.insert(field.name.to_owned(), (schema_builder.add_u64_field(&field.name, STORED | INDEXED), FieldType::Unsigned64));
                }
                FieldType::Float64 => {
                    field_lookup.insert(field.name.to_owned(), (schema_builder.add_f64_field(&field.name, STORED | INDEXED), FieldType::Float64));
                }
            }
        }
        let schema = schema_builder.build();
        let index = Index::open_or_create(MmapDirectory::open(&config.storage_path)?, schema.clone())?;

        let index_writer = Arc::new(RwLock::new(index.writer(50_000_000)?));
        let index_writer2 = index_writer.clone();
        thread::spawn(move || {
            if let Err(e) = index_writer2.write().commit() {
                tracing::error!("Error committing changes to disk: {}", e);
            }
            thread::sleep(Duration::from_millis(500));
        });

        let index_reader = index.reader_builder().reload_policy(ReloadPolicy::OnCommit).try_into()?;
        let index_reader = Arc::new(RwLock::new(index_reader));

        let field_lookup = Arc::new(RwLock::new(field_lookup));

        Ok(Self {
            index: Arc::new(RwLock::new(index)),
            schema: Arc::new(RwLock::new(schema)),
            index_writer,
            index_reader,
            field_lookup,
        })
    }

    pub fn save_doc(&self, inbound_doc: log_analyzer_transient_types::Document) -> Result<()> {
        let mut parsed_doc = tantivy::Document::new();
        let lookup_table = self.field_lookup.read();
        for field in inbound_doc.fields.iter() {
            let (index_field, field_type) = &lookup_table[&field.name];
            match field_type {
                FieldType::Text => parsed_doc.add_text(*index_field, &field.content),
                FieldType::Date => parsed_doc.add_date(*index_field, &tantivy::DateTime::from(DateTime::parse_from_rfc3339(&field.content)?)),
                FieldType::Integer64 => parsed_doc.add_i64(*index_field, i64::from_str(&field.content)?),
                FieldType::Unsigned64 => parsed_doc.add_u64(*index_field, u64::from_str(&field.content)?),
                FieldType::Float64 => parsed_doc.add_f64(*index_field, f64::from_str(&field.content)?),
            }
        }
        self.index_writer.read().add_document(parsed_doc);
        Ok(())
    }

    pub fn search_everything(&self, query: &str, offset: usize) -> Result<(usize, Vec<String>)> {
        let fields: Vec<Field> = self.field_lookup.read().iter().map(|kv_pair| kv_pair.1).map(|tuple| tuple.0).collect();
        let index = self.index.read();
        let query_parser = QueryParser::for_index(&index, fields);
        let query = query_parser.parse_query(query)?;
        let searcher = self.index_reader.read().searcher();
        let (doc_count, top_docs) = searcher.search(&query, &(Count, TopDocs::with_limit(20).and_offset(offset)))?;
        let schema = self.schema.read();
        let mut result = vec![];
        for (_score, doc_address) in top_docs.iter() {
            let doc = searcher.doc(*doc_address)?;
            let formatted_doc = schema.to_json(&doc);
            result.push(formatted_doc);
        }
        Ok((doc_count, result))
    }
}
