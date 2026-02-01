use crate::store::errors::StoreError;
use duckdb::Connection;
use std::sync::{Arc, Mutex};

/// Table-related database queries

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRef {
    pub schema_name: String,
    pub table_name: String,
}

impl TableRef {
    pub fn new(schema_name: String, table_name: String) -> Self {
        Self {
            schema_name,
            table_name,
        }
    }

    /// Parse table reference from string
    /// Requires "schema.table" format
    pub fn from_str(table_ref: &str) -> Result<Self, String> {
        if let Some((schema_name, table_name)) = super::parse_table_ref(table_ref.to_string()) {
            if schema_name.is_empty() {
                return Err(format!(
                    "Schema is required. Expected format: 'schema.table', got: '{}'",
                    table_ref
                ));
            }
            Ok(Self::new(schema_name, table_name))
        } else {
            Err(format!(
                "Invalid table reference format. Expected 'schema.table', got: '{}'",
                table_ref
            ))
        }
    }

    pub fn to_string(&self) -> String {
        if self.schema_name.is_empty() {
            self.table_name.clone()
        } else {
            format!("{}.{}", self.schema_name, self.table_name)
        }
    }

    pub fn to_quoted_string(&self) -> String {
        let quoted_table = super::quote_identifier(&self.table_name);
        if self.schema_name.is_empty() {
            quoted_table
        } else {
            let quoted_schema = super::quote_identifier(&self.schema_name);
            format!("{}.{}", quoted_schema, quoted_table)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub database_name: String,
    pub schema_name: String,
    pub table_name: String,
    pub bounds: Option<[f64; 4]>, // [xmin, ymin, xmax, ymax]
    pub properties: Vec<String>,
    pub geometry_columns: Vec<String>,
}

impl TableInfo {
    pub fn table_ref(&self) -> TableRef {
        TableRef::new(self.schema_name.clone(), self.table_name.clone())
    }
}

/// Query all tables from DuckDB that have geometry columns
/// Returns a list of tables with their database, schema, and table names
pub fn query_geometry_tables(
    connection: &Arc<Mutex<Connection>>,
) -> Result<Vec<TableInfo>, StoreError> {
    let conn = connection.lock().unwrap();

    // Query all tables with geometry columns
    let mut tables = query_geometry_table_rows(&conn, None, None).map_err(StoreError::from)?;

    // Calculate bounds for each table
    for table_info in &mut tables {
        let table_ref = table_info.table_ref();
        let bounds = calculate_bounds(&conn, &table_ref).map_err(StoreError::from)?;
        if bounds.is_some() {
            table_info.bounds = bounds;
        } else {
            table_info.bounds = None;
        }
    }

    Ok(tables)
}

/// Query a specific table by name
/// Returns the table info if it exists and has a geometry column
///
/// # Errors
/// - `StoreError::TableNotFound` if the table doesn't exist
/// - `StoreError::NoGeometryColumn` if the table exists but has no geometry column
pub fn query_geometry_table(
    connection: &Arc<Mutex<Connection>>,
    table_ref: &TableRef,
) -> Result<TableInfo, StoreError> {
    let conn = connection.lock().unwrap();

    let mut tables = query_geometry_table_rows(
        &conn,
        Some(&table_ref.schema_name),
        Some(&table_ref.table_name),
    )
    .map_err(StoreError::from)?;

    let mut table_info = tables.pop().ok_or_else(|| StoreError::TableNotFound {
        table_name: table_ref.table_name.clone(),
    })?;

    // Calculate bounds for the table
    let table_ref = table_info.table_ref();
    let bounds = calculate_bounds(&conn, &table_ref).map_err(StoreError::from)?;
    if bounds.is_some() {
        table_info.bounds = bounds;
    } else {
        table_info.bounds = None;
    }

    Ok(table_info)
}

/// Calculate bounds for a table by finding the extent of all geometry columns
/// Returns the union of all bounds from all geometry columns
fn calculate_bounds(
    conn: &Connection,
    table_ref: &TableRef,
) -> Result<Option<[f64; 4]>, duckdb::Error> {
    // Fetch the first geometry column
    let geometry_column = match fetch_first_geometry_column(conn, table_ref) {
        Ok(col) => col,
        Err(_) => return Ok(None), // No geometry column found
    };

    // Calculate bounds using the new fetch method
    let bounds_result = fetch_extent_for_geometry_column(conn, table_ref, &geometry_column);

    match bounds_result {
        Ok(bounds) => {
            let [xmin, ymin, xmax, ymax] = bounds;

            // Validate bounds: ensure min < max and values are reasonable
            if xmin >= xmax || ymin >= ymax {
                return Ok(None);
            }

            Ok(Some(bounds))
        }
        Err(duckdb::Error::QueryReturnedNoRows) => {
            Ok(None) // No rows in table or all geometries are NULL/empty
        }
        Err(e) => {
            Err(e)
        }
    }
}

/// Helper function to query geometry tables with optional WHERE clause
/// Returns a list of TableInfo objects without bounds (bounds must be calculated separately)
fn query_geometry_table_rows(
    conn: &Connection,
    schema_name: Option<&str>,
    table_name: Option<&str>,
) -> Result<Vec<TableInfo>, duckdb::Error> {
    let (sql, _has_params) = build_query_geometry_tables_sql(schema_name, table_name);
    let mut stmt = conn.prepare(&sql)?;

    // Helper closure to process a row into TableInfo
    let process_row = |row: &duckdb::Row| -> Result<TableInfo, duckdb::Error> {
        let properties_str: Option<String> = row.get(3)?;
        let geometry_columns_str: Option<String> = row.get(4)?;

        let properties = properties_str
            .map(|s| super::parse_list_string(&s))
            .unwrap_or_default();

        let geometry_columns = geometry_columns_str
            .map(|s| super::parse_list_string(&s))
            .unwrap_or_default();

        Ok(TableInfo {
            database_name: row.get(0)?,
            schema_name: row.get(1)?,
            table_name: row.get(2)?,
            bounds: None,
            properties,
            geometry_columns,
        })
    };

    // Execute query with appropriate parameters
    let tables: Vec<TableInfo> = match (schema_name, table_name) {
        (Some(schema), Some(table)) => stmt
            .query_map([schema, table], process_row)?
            .collect::<Result<Vec<_>, _>>()?,
        _ => stmt
            .query_map([], process_row)?
            .collect::<Result<Vec<_>, _>>()?,
    };

    Ok(tables)
}

/// Build the SQL query for querying geometry tables.
fn build_query_geometry_tables_sql(
    schema_name: Option<&str>,
    table_name: Option<&str>,
) -> (String, bool) {
    const BASE: &str = r#"
        SELECT
            database_name,
            schema_name,
            table_name,
            STRING_AGG(DISTINCT column_name, ',')
                FILTER (WHERE upper(data_type) NOT LIKE '%GEOMETRY%') AS properties,
            STRING_AGG(DISTINCT column_name, ',')
                FILTER (WHERE upper(data_type) LIKE '%GEOMETRY%') AS geometry_columns
        FROM duckdb_columns()"#;

    const GROUP_ORDER: &str = r#"
        GROUP BY database_name, schema_name, table_name
        HAVING COUNT(*) FILTER (WHERE upper(data_type) LIKE '%GEOMETRY%') > 0
        ORDER BY database_name, schema_name, table_name"#;

    let mut sql = String::from(BASE);
    let mut has_params = false;

    if schema_name.is_some() && table_name.is_some() {
        sql.push_str(" WHERE schema_name = ? AND table_name = ?");
        has_params = true;
    }

    sql.push_str(GROUP_ORDER);
    (sql, has_params)
}

fn fetch_first_geometry_column(
    conn: &Connection,
    table_ref: &TableRef,
) -> Result<String, duckdb::Error> {
    const SQL: &str = r#"
        SELECT column_name
        FROM duckdb_columns()
        WHERE schema_name = ?
           AND table_name = ?
           AND upper(data_type) LIKE '%GEOMETRY%'
        ORDER BY column_index
        LIMIT 1"#;

    conn.prepare(SQL)?
        .query_row([&table_ref.schema_name, &table_ref.table_name], |row| {
            row.get::<_, String>(0)
        })
}

fn fetch_extent_for_geometry_column(
    conn: &Connection,
    table_ref: &TableRef,
    geometry_column: &str,
) -> Result<[f64; 4], duckdb::Error> {
    let quoted_table = table_ref.to_quoted_string();
    let quoted_geom = super::quote_identifier(geometry_column);
    let sql = format!(
        r#"
        SELECT 
            MIN(ST_XMin({})) AS xmin, 
            MIN(ST_YMin({})) AS ymin, 
            MAX(ST_XMax({})) AS xmax, 
            MAX(ST_YMax({})) AS ymax
        FROM {}
        WHERE {} IS NOT NULL
        AND ST_IsEmpty({}) = FALSE
        AND ST_XMin({}) != 0
        AND ST_YMin({}) != 0
        AND ST_XMax({}) != 0
        AND ST_YMax({}) != 0"#,
        quoted_geom, quoted_geom, quoted_geom, quoted_geom, 
        quoted_table, quoted_geom, quoted_geom, 
        quoted_geom, quoted_geom, quoted_geom, quoted_geom,
    );

    conn.prepare(&sql)?.query_row([], |row| {
        let xmin: f64 = row.get(0)?;
        let ymin: f64 = row.get(1)?;
        let xmax: f64 = row.get(2)?;
        let ymax: f64 = row.get(3)?;
        Ok([xmin, ymin, xmax, ymax])
    })
}
