/// Data store layer - handles all DuckDB queries and data access
/// This module separates database access logic from REST API handlers
///
/// Structure:
/// - tables.rs: Table-related queries
/// - tiles.rs: Tile-related queries
/// - errors.rs: Custom error types
/// - mod.rs: Common types and utilities
pub mod errors;
pub mod tables;
pub mod tiles;

/// Parse a table identifier into schema and table names
pub(crate) fn parse_table_ref(table_string: String) -> Option<(String, String)> {
    let parts: Vec<&str> = table_string.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parts[0].to_string(), parts[1].to_string()))
}

/// Quote an identifier for SQL
pub(crate) fn quote_identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Format a list of columns as a SELECT clause
/// 
/// # Arguments
/// * `columns` - List of column names
/// * `table_alias` - Optional table alias (e.g., "t")
/// * `separator` - Separator between columns (default: ",\n")
/// 
/// # Examples
/// ```
/// let cols = vec!["id".to_string(), "name".to_string()];
/// let result = format_select_columns(&cols, Some("t"), ",\n");
/// // Result: "t.id AS id,\nt.name AS name"
/// ```
pub(crate) fn format_select_columns(
    columns: &[String],
    table_alias: Option<&str>,
    separator: &str,
) -> String {
    if columns.is_empty() {
        return String::new();
    }

    columns
        .iter()
        .map(|col| {
            let quoted_col = quote_identifier(col);
            if let Some(alias) = table_alias {
                format!("{}.{} AS {}", alias, quoted_col, quoted_col)
            } else {
                quoted_col
            }
        })
        .collect::<Vec<_>>()
        .join(separator)
}

/// Format struct fields for ST_AsMVT with geometry and optional property columns.
/// Converts unsupported types (DATE, TIMESTAMP, etc.) to VARCHAR.
///
/// # Arguments
/// * `property_columns` - List of (column_name, data_type) tuples
/// * `geometry_expr` - SQL expression for the geometry field
///
/// # Returns
/// A string like "geom:=geom_expr, prop1:=CAST(prop1 AS VARCHAR), prop2:=prop2"
/// or just "geom:=geom_expr" if no properties.
///
/// Supported types: VARCHAR, FLOAT, DOUBLE, INTEGER, BIGINT, BOOLEAN
/// Unsupported types (converted to VARCHAR): DATE, TIMESTAMP, TIME, and others
pub(crate) fn format_struct_fields(
    property_columns: &[(String, String)],
    geometry_expr: &str,
) -> String {
    let geom_field = format!("geom:={}", geometry_expr);
    
    if property_columns.is_empty() {
        return geom_field;
    }
    
    let props: Vec<String> = property_columns
        .iter()
        .map(|(col_name, data_type)| {
            let quoted_col = quote_identifier(col_name);
            let data_type_upper = data_type.to_uppercase();
            
            // Check if type needs conversion to VARCHAR
            // Supported types: VARCHAR, FLOAT, DOUBLE, INTEGER, BIGINT, BOOLEAN
            let needs_conversion = !matches!(
                data_type_upper.as_str(),
                "VARCHAR" | "TEXT" | "STRING" | "FLOAT" | "DOUBLE" | "REAL" | 
                "INTEGER" | "INT" | "BIGINT" | "BOOLEAN" | "BOOL"
            );
            
            if needs_conversion {
                // Convert unsupported types (DATE, TIMESTAMP, etc.) to VARCHAR
                format!("{}:=CAST({} AS VARCHAR)", col_name, quoted_col)
            } else {
                format!("{}:={}", col_name, quoted_col)
            }
        })
        .collect();
    
    format!("{}, {}", geom_field, props.join(", "))
}

/// Parse DuckDB LIST string representation into Vec<String>
/// DuckDB LIST format: ['value1', 'value2', 'value3'] or just comma-separated
pub(crate) fn parse_list_string(s: &str) -> Vec<String> {
    // Remove brackets if present
    let cleaned = s.trim_start_matches('[').trim_end_matches(']');
    // Split by comma and clean up quotes
    cleaned
        .split(',')
        .map(|item| item.trim().trim_matches('\'').trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}
