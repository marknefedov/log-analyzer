use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum FieldType {
    Text,
    Date,
    Integer64,
    Unsigned64,
    Float64,
}

#[derive(Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
}
#[derive(Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub schema: Schema,
    pub storage_path: String,
}
