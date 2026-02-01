use crate::api::errors::{store_error_to_response, InvalidTableReference};
use crate::api::run_blocking_db;
use crate::api::TEMPLATES;
use crate::server::ServerContext;
use crate::store::tables::{self, TableRef};
use axum::extract::Path;
use axum::http::{StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::{extract::State, response::Result, Json, Router};
use serde_json;
use std::f64::consts::PI;

pub fn router() -> Router<ServerContext> {
    Router::new()
        .route("/tables", axum::routing::get(list_tables))
        .route("/tables/", axum::routing::get(list_tables))
        .route("/tables/{table_name}", axum::routing::get(get_table))
        .route("/tables/{table_name}/", axum::routing::get(get_table))
        .route(
            "/tables/{table_name}/viewer",
            axum::routing::get(map_viewer),
        )
        .route(
            "/tables/{table_name}/viewer/",
            axum::routing::get(map_viewer),
        )
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableListBody {
    tables: Vec<tables::TableInfo>,
    table_count: usize,
}

/// REST API handler for listing all tables
/// Delegates to the store layer for database access
pub async fn list_tables(
    State(server_context): State<ServerContext>,
) -> Result<Json<TableListBody>> {
    let connection = server_context.connection().clone();

    // Delegate to store layer
    let tables = run_blocking_db("list_tables", move || tables::query_geometry_tables(&connection))
        .await
        .map_err(|e| {
            axum::response::Response::builder()
                .status(500)
                .body(format!("Database error: {}", e))
                .unwrap()
        })?;

    Ok(Json(TableListBody {
        table_count: tables.len(),
        tables,
    }))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableBody {
    table: tables::TableInfo,
}

pub async fn get_table(
    State(server_context): State<ServerContext>,
    Path(table_name): Path<String>,
) -> Response {
    let connection = server_context.connection().clone();

    let table_ref = match TableRef::from_str(&table_name) {
        Ok(ref_) => ref_,
        Err(msg) => return InvalidTableReference { message: msg }.into_response(),
    };
    let table_ref_for_query = table_ref.clone();
    let table = match run_blocking_db("get_table", move || {
        tables::query_geometry_table(&connection, &table_ref_for_query)
    })
    .await
    {
        Ok(table) => table,
        Err(e) => return store_error_to_response(e),
    };
    Json(TableBody { table }).into_response()
}

pub async fn map_viewer(
    State(server_context): State<ServerContext>,
    Path(table_name): Path<String>,
    uri: Uri,
) -> Response {
    let connection = server_context.connection().clone();

    let table_ref = match TableRef::from_str(&table_name) {
        Ok(ref_) => ref_,
        Err(msg) => return InvalidTableReference { message: msg }.into_response(),
    };

    let table_ref_for_query = table_ref.clone();
    let table_info = match run_blocking_db("map_viewer", move || {
        tables::query_geometry_table(&connection, &table_ref_for_query)
    })
    .await
    {
        Ok(table) => table,
        Err(e) => return store_error_to_response(e),
    };

    // Prepare template context
    let mut context = tera::Context::new();

    // Use full table reference (schema.table_name) for display
    context.insert("tableName", &table_ref.to_string());

    // Tile URL pattern: /tables/{schema.table_name}/tiles/{z}/{x}/{y}.pbf?{query}
    let mut tile_url = format!(
        "/tables/{}/tiles/{{z}}/{{x}}/{{y}}.pbf",
        table_ref.to_string()
    );

    if let Some(query) = uri.query() {
        tile_url.push_str("?");
        tile_url.push_str(query);
    }
    context.insert("tileUrl", &tile_url);

    // Add bounding box as JSON string for JavaScript
    // Convert from Web Mercator (EPSG:3857) to WGS84 (EPSG:4326)
    // Expand bounds by 5% to ensure all features are visible
    if let Some(bounds) = &table_info.bounds {
        let wgs84_bounds = web_mercator_to_wgs84(bounds);
        let [lon_min, lat_min, lon_max, lat_max] = wgs84_bounds;

        // Expand bounds by 5% in each direction to ensure all features are visible
        let lon_range = lon_max - lon_min;
        let lat_range = lat_max - lat_min;
        let expansion_factor = 0.05; // 5% expansion

        let expanded_bounds = [
            lon_min - lon_range * expansion_factor,
            lat_min - lat_range * expansion_factor,
            lon_max + lon_range * expansion_factor,
            lat_max + lat_range * expansion_factor,
        ];

        let bbox_json =
            serde_json::to_string(&expanded_bounds).unwrap_or_else(|_| "null".to_string());
        context.insert("bbox", &bbox_json);
    } else {
        context.insert("bbox", "null");
    }

    // Render template
    let rendered = match TEMPLATES.render("map-preview.html", &context) {
        Ok(html) => html,
        Err(e) => {
            log::error!("Template error details: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Template error: {} (Check console for details)", e),
            )
                .into_response();
        }
    };

    Html(rendered).into_response()
}

/// Convert Web Mercator (EPSG:3857) coordinates to WGS84 (EPSG:4326)
/// Input: [xmin, ymin, xmax, ymax] in Web Mercator meters
/// Output: [lon_min, lat_min, lon_max, lat_max] in WGS84 degrees
fn web_mercator_to_wgs84(bounds: &[f64; 4]) -> [f64; 4] {
    const EARTH_RADIUS: f64 = 6378137.0; // meters
    const MAX_EXTENT: f64 = EARTH_RADIUS * PI; // ~20037508.34

    let [xmin, ymin, xmax, ymax] = *bounds;

    // convert longtitude
    let lon_min = xmin / MAX_EXTENT * 180.0;
    let lon_max = xmax / MAX_EXTENT * 180.0;

    // convert latitude
    let lat_min = (ymin / MAX_EXTENT * PI).exp().atan() * 360.0 / PI - 90.0;
    let lat_max = (ymax / MAX_EXTENT * PI).exp().atan() * 360.0 / PI - 90.0;

    [lon_min, lat_min, lon_max, lat_max]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_mercator_to_wgs84_center() {
        let bounds = [0.0, 0.0, 0.0, 0.0];
        let result = web_mercator_to_wgs84(&bounds);
        assert_eq!(result, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_web_mercator_to_wgs84_positive_quadrant() {
        let bounds = [1000000.0, 1000000.0, 2000000.0, 2000000.0];
        let [lon_min, lat_min, lon_max, lat_max] = web_mercator_to_wgs84(&bounds);
        
        assert!(lon_min > 0.0 && lon_min < 180.0);
        assert!(lon_max > lon_min);

        assert!(lat_min > 0.0 && lat_min < 90.0);
        assert!(lat_max > lat_min);
    }

    #[test]
    fn test_web_mercator_to_wgs84_world_bounds() {
        const EARTH_RADIUS: f64 = 6378137.0;
        const MAX_EXTENT: f64 = EARTH_RADIUS * std::f64::consts::PI;

        let bounds = [-MAX_EXTENT, -MAX_EXTENT, MAX_EXTENT, MAX_EXTENT];
        let [lon_min, lat_min, lon_max, lat_max] = web_mercator_to_wgs84(&bounds);

        assert!((lon_min - (-180.0)).abs() < 0.1);
        assert!((lon_max - 180.0).abs() < 0.1);
        assert!((lat_min - (-85.0)).abs() < 1.0);  
        assert!((lat_max - 85.0).abs() < 1.0);
    }
}
