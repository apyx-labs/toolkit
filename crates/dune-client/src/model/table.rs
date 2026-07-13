use std::fmt;

use serde::{Deserialize, Serialize};

/// A reference to a Dune-managed table (`dune.{namespace}.{table_name}`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TableRef {
    pub namespace: String,
    pub table_name: String,
}

impl TableRef {
    pub fn new(namespace: impl Into<String>, table_name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            table_name: table_name.into(),
        }
    }
}

impl fmt::Display for TableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dune.{}.{}", self.namespace, self.table_name)
    }
}

/// Column types supported by Dune table schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    Varchar,
    Integer,
    Bigint,
    Double,
    Boolean,
    Timestamp,
    Uint256,
    Int256,
    Varbinary,
}

/// A column in a Dune table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: ColumnType,
    pub nullable: bool,
}

impl Column {
    pub fn new(name: impl Into<String>, r#type: ColumnType, nullable: bool) -> Self {
        Self {
            name: name.into(),
            r#type,
            nullable,
        }
    }
}

/// Request body for `POST /uploads` (create table).
#[derive(Debug, Clone, Serialize)]
pub struct CreateTableRequest {
    pub namespace: String,
    pub table_name: String,
    pub schema: Vec<Column>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub is_private: bool,
}

/// Response of `POST /uploads`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreateTableResponse {
    pub namespace: String,
    pub table_name: String,
    pub full_name: String,
    pub example_query: String,
    pub already_existed: bool,
    pub message: String,
}

/// Response of `POST /uploads/{namespace}/{table_name}/insert`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct InsertRowsResponse {
    pub rows_written: u64,
    pub bytes_written: u64,
}
