use once_cell::sync::Lazy;
use std::path::PathBuf;

/// Environment variables
const ENV_LOG_FILE: &str = "MVTS_LOG_FILE";
const ENV_MIN_ZOOM: &str = "MVTS_MIN_ZOOM";
const ENV_MAX_ZOOM: &str = "MVTS_MAX_ZOOM";
const ENV_MAX_FEATURES_PER_TILE: &str = "MVTS_MAX_FEATURES_PER_TILE";
const ENV_CLIP_GEOMETRIES: &str = "MVTS_CLIP_GEOMETRIES";

/// Default values
const DEFAULT_MIN_ZOOM: u32 = 0;
const DEFAULT_MAX_ZOOM: u32 = 22;
const DEFAULT_MAX_FEATURES_PER_TILE: u32 = 50000;
const DEFAULT_CLIP_GEOMETRIES: bool = true;

/// Lazy-loaded log file path from environment variable
/// Returns Some(PathBuf) if MVTS_LOG_FILE is set, None otherwise
pub(crate) static LOG_FILE_PATH: Lazy<Option<PathBuf>> = Lazy::new(|| {
    std::env::var(ENV_LOG_FILE)
        .ok()
        .map(PathBuf::from)
});

/// Get the log file path if configured
pub(crate) fn get_log_file_path() -> Option<&'static PathBuf> {
    LOG_FILE_PATH.as_ref()
}

/// Get the minimum zoom level
pub(crate) fn get_min_zoom() -> u32 {
    std::env::var(ENV_MIN_ZOOM)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MIN_ZOOM)
}

/// Get the maximum zoom level
pub(crate) fn get_max_zoom() -> u32 {
    std::env::var(ENV_MAX_ZOOM)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_ZOOM)
}

/// Get the maximum number of features per tile
pub(crate) fn get_max_features_per_tile() -> u32 {
    std::env::var(ENV_MAX_FEATURES_PER_TILE)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_FEATURES_PER_TILE)
}

/// Get whether geometry clipping is enabled for ST_AsMVTGeom
pub(crate) fn get_clip_geometries() -> bool {
    std::env::var(ENV_CLIP_GEOMETRIES)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CLIP_GEOMETRIES)
}
