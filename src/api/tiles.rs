use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::api::errors::{InvalidTableReference, store_error_to_response};
use crate::api::run_blocking_db;
use crate::server::ServerContext;
use crate::store::tables;
use crate::store::tiles::TileXYZ;

use super::add_cors;

pub fn router() -> Router<ServerContext> {
    Router::new()
        .route("/tables/{table}/tiles/{z}/{x}/{y_ext}", get(get_tile))
}

async fn get_tile(
    Path((table, z, x, y_ext)): Path<(String, u32, u32, String)>,
    State(server_context): State<ServerContext>,
) -> Response {
    let connection = server_context.connection().clone();
    log::info!("[TILE] Request: table={}, z={}, x={}, y_ext={}", table, z, x, y_ext);

    // Validate zoom level against configured min/max
    let min_zoom = crate::utils::env::get_min_zoom();
    let max_zoom = crate::utils::env::get_max_zoom();
    
    if z < min_zoom || z > max_zoom {
        log::warn!("[TILE] Zoom level {} outside valid range {}-{}", z, min_zoom, max_zoom);
        return (
            StatusCode::NOT_FOUND,
            format!("Zoom level {} not supported. Valid range: {}-{}", z, min_zoom, max_zoom)
        ).into_response();
    }

    // Parse y and validate extension
    let y = match parse_y_with_extension(&y_ext) {
        Ok(y) => y,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    // Parse table reference
    let table_ref = match tables::TableRef::from_str(&table) {
        Ok(ref_) => ref_,
        Err(msg) => return InvalidTableReference { message: msg }.into_response(),
    };

    let tile_xyz: TileXYZ = TileXYZ::new(x, y, z);
    let start = std::time::Instant::now();
    let table_ref_for_query = table_ref.clone();
    let tile = match run_blocking_db("tile", move || {
        crate::store::tiles::query_tile(&connection, &table_ref_for_query, tile_xyz)
    })
    .await
    {
        Ok(tile) => tile,
        Err(e) => {
            log::error!("[TILE] Error querying tile: {:?}", e);
            return store_error_to_response(e);
        }
    };
    let duration = start.elapsed();
    log::info!("[TILE] Query completed in {:?}, size={} bytes", duration, tile.as_ref().map(|d| d.len()).unwrap_or(0));

    // Handle empty tiles - return 204 No Content instead of empty tile data
    match tile {
        Some(data) if !data.is_empty() => tile_response(StatusCode::OK, data),
        _ => tile_response(StatusCode::NO_CONTENT, vec![]),
    }
}

/// Parse y coordinate and validate file extension from a string like "123.pbf"
/// Returns the y coordinate as u32
///
/// # Errors
/// - Returns BAD_REQUEST if extension is missing or not .pbf/.mvt
/// - Returns BAD_REQUEST if y is not a valid integer
fn parse_y_with_extension(y_ext: &str) -> Result<u32, (StatusCode, String)> {
    if let Some(dot_pos) = y_ext.rfind('.') {
        let (y_str, ext_str) = y_ext.split_at(dot_pos);
        let ext = &ext_str[1..]; // Remove the dot

        // Validate extension
        if ext != "pbf" && ext != "mvt" {
            let error = format!(
                "Invalid file extension: .{}. Only .pbf and .mvt are allowed",
                ext
            );
            return Err((StatusCode::BAD_REQUEST, error));
        }

        // Parse y as integer
        y_str.parse::<u32>().map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid y coordinate: {}", e),
            )
        })
    } else {
        // No extension found
        Err((
            StatusCode::BAD_REQUEST,
            "Missing file extension. Expected .pbf or .mvt".to_string(),
        ))
    }
}

/// Create a tile response with proper headers
fn tile_response(status: StatusCode, data: Vec<u8>) -> Response {
    let mut resp = (status, data).into_response();
    let headers = resp.headers_mut();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/vnd.mapbox-vector-tile"),
    );
    if status == StatusCode::OK {
        headers.insert(
            "Cache-Control",
            HeaderValue::from_static("public, max-age=3600"),
        );
    }
    add_cors(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_y_with_valid_pbf_extension() {
        let result = parse_y_with_extension("123.pbf");
        assert_eq!(result, Ok(123));
    }

    #[test]
    fn test_parse_y_with_valid_mvt_extension() {
        let result = parse_y_with_extension("456.mvt");
        assert_eq!(result, Ok(456));
    }

    #[test]
    fn test_parse_y_with_invalid_extension() {
        let result = parse_y_with_extension("123.txt");
        assert!(result.is_err());
        if let Err((status, msg)) = result {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(msg.contains("Invalid file extension"));
        }
    }

    #[test]
    fn test_parse_y_with_missing_extension() {
        let result = parse_y_with_extension("123");
        assert!(result.is_err());
        if let Err((status, msg)) = result {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(msg.contains("Missing file extension"));
        }
    }

    #[test]
    fn test_parse_y_with_invalid_number() {
        let result = parse_y_with_extension("abc.pbf");
        assert!(result.is_err());
        if let Err((status, msg)) = result {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(msg.contains("Invalid y coordinate"));
        }
    }

    #[test]
    fn test_parse_y_with_zero() {
        let result = parse_y_with_extension("0.pbf");
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn test_parse_y_with_large_number() {
        let result = parse_y_with_extension("999999.mvt");
        assert_eq!(result, Ok(999999));
    }
}
