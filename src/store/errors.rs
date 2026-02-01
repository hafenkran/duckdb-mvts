use std::fmt;

/// Custom errors for store operations
#[derive(Debug)]
pub enum StoreError {
    /// Table not found in database
    TableNotFound {
        table_name: String,
    },
    /// Table exists but has no geometry column
    NoGeometryColumn {
        table_name: String,
    },
    /// Database query error
    DatabaseError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::TableNotFound { table_name } => {
                write!(f, "Table '{}' not found in database", table_name)
            }
            StoreError::NoGeometryColumn { table_name } => {
                write!(
                    f,
                    "Table '{}' exists but has no geometry column",
                    table_name
                )
            }
            StoreError::DatabaseError { source } => {
                write!(f, "Database error: {}", source)
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::DatabaseError { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}

// Convert DuckDB errors to StoreError
impl From<duckdb::Error> for StoreError {
    fn from(err: duckdb::Error) -> Self {
        StoreError::DatabaseError {
            source: Box::new(err),
        }
    }
}

