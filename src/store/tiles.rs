use crate::store::errors::StoreError;
use std::sync::{Arc, Mutex};

use duckdb::Connection;

use super::tables::TableRef;

/// Tile coordinates (z, x, y) for MVT tiles
#[derive(Debug, Clone, Copy)]
pub struct TileXYZ {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl TileXYZ {
    /// Create a new TileXYZ
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }
}

pub(crate) fn query_tile(
    connection: &Arc<Mutex<Connection>>,
    table_ref: &TableRef,
    xyz: TileXYZ,
) -> Result<Option<Vec<u8>>, StoreError> {
    let conn = connection.lock().unwrap();

    // Fetch geometry column and property columns
    let (geom_col, geom_crs, property_cols_with_types) = {
        let geom_col = fetch_geometry_column(&conn, table_ref)?;
        let geom_crs = fetch_geometry_column_crs(&conn, table_ref, &geom_col)?;
        let property_cols = fetch_property_columns(&conn, table_ref)?;
        (geom_col, geom_crs, property_cols)
    };

    // Extract column names for SELECT clause
    // Property columns only (geometry will be computed separately)
    let property_col_names: Vec<String> = property_cols_with_types
        .iter()
        .map(|(name, _)| name.clone())
        .collect();

    let quoted_geom_col = crate::store::quote_identifier(&geom_col);
    let source_geom_expr = format!("t.{}", quoted_geom_col);
    let webmercator_geom_expr =
        crate::store::webmercator_geometry_expr(&source_geom_expr, geom_crs.as_deref());
    let select_properties =
        crate::store::format_select_columns(&property_col_names, Some("t"), ",\n");
    let clip_geometry = crate::utils::env::get_clip_geometries();
    let geom_expr = format!(
        "ST_AsMVTGeom(ST_MakeValid({}), bounds_box, 4096, 256, {})",
        webmercator_geom_expr, clip_geometry
    );
    let struct_fields = crate::store::format_struct_fields(&property_cols_with_types, "geom");

    let qualified_table = table_ref.to_quoted_string();

    // Get maximum features per tile from environment variable
    let max_features = crate::utils::env::get_max_features_per_tile();

    // Build SELECT clause - properties (if any) followed by repaired geometry directly in SQL
    let select_prefix = if property_col_names.is_empty() {
        String::new()
    } else {
        format!("{},\n", select_properties)
    };

    let sql = format!(
        "
        WITH params AS (
            SELECT ?::INTEGER AS z, ?::INTEGER AS x, ?::INTEGER AS y
        ),
        tile_raw AS (
            SELECT 
                ST_TileEnvelope(z, x, y) AS bounds,
                ST_Extent(ST_TileEnvelope(z, x, y)) AS bounds_box
            FROM params
        ),
        tile AS (
            SELECT bounds, bounds_box
            FROM tile_raw
            WHERE bounds_box IS NOT NULL
              AND ST_XMax(bounds_box) > ST_XMin(bounds_box)
              AND ST_YMax(bounds_box) > ST_YMin(bounds_box)
        ),
        geom_rows AS (
            SELECT {}{} AS geom,
                tile.bounds_box AS bounds_box
            FROM {} AS t, tile
            WHERE ST_Intersects({}, tile.bounds)
            LIMIT {}
        ),
        mvt AS (
            SELECT ST_AsMVT(
                STRUCT_PACK({}),
                'default',
                4096,
                'geom'
            ) AS tile
            FROM geom_rows
            WHERE geom IS NOT NULL
              AND NOT ST_IsEmpty(geom)
              AND (ST_Dimension(geom) <> 1 OR ST_NPoints(geom) >= 2)
        )
        SELECT tile FROM mvt LIMIT 1
        ",
        select_prefix,
        geom_expr,
        qualified_table,
        webmercator_geom_expr,
        max_features,
        struct_fields
    );

    // Lock connection only for the actual query execution
    // Minimize lock duration by preparing and executing in one go
    let tile_data = {
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([xyz.z, xyz.x, xyz.y])?;

        match rows.next()? {
            Some(row) => {
                let data: Vec<u8> = row.get(0)?;
                Some(data)
            }
            None => None,
        }
    };

    Ok(tile_data)
}

fn fetch_geometry_column_crs(
    conn: &Connection,
    table_ref: &TableRef,
    geometry_column: &str,
) -> Result<Option<String>, StoreError> {
    let quoted_table = table_ref.to_quoted_string();
    let quoted_geom = crate::store::quote_identifier(geometry_column);
    let sql = format!(
        "
        SELECT ST_CRS({0})
        FROM {1}
        WHERE {0} IS NOT NULL
          AND ST_IsEmpty({0}) = FALSE
        LIMIT 1
        ",
        quoted_geom, quoted_table
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;

    match rows.next()? {
        Some(row) => {
            let crs: Option<String> = row.get(0)?;
            Ok(crs)
        }
        None => Ok(None),
    }
}

fn fetch_property_columns(
    conn: &Connection,
    table_ref: &TableRef,
) -> Result<Vec<(String, String)>, StoreError> {
    // Get all columns that are NOT geometry columns (property columns) with their data types
    // Returns (column_name, data_type) tuples
    let sql = "
        SELECT column_name, data_type
        FROM duckdb_columns()
        WHERE schema_name = ?
           AND table_name = ?
           AND upper(data_type) NOT LIKE '%GEOMETRY%'
        ORDER BY column_index";

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([&table_ref.schema_name, &table_ref.table_name])?;
    let mut cols = Vec::new();

    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        let data_type: String = row.get(1)?;
        cols.push((col_name, data_type));
    }
    Ok(cols)
}

fn fetch_geometry_column(conn: &Connection, table_ref: &TableRef) -> Result<String, StoreError> {
    // SQL to find the first geometry column
    let sql = "
        SELECT column_name
        FROM duckdb_columns()
        WHERE schema_name = ?
           AND table_name = ?
           AND upper(data_type) LIKE '%GEOMETRY%'
        ORDER BY column_index
        LIMIT 1";

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([&table_ref.schema_name, &table_ref.table_name])?;

    match rows.next()? {
        Some(row) => Ok(row.get(0)?),
        None => Err(StoreError::NoGeometryColumn {
            table_name: table_ref.table_name.clone(),
        }),
    }
}
