use once_cell::sync::Lazy;
use simplelog::*;
use std::fs::OpenOptions;

use super::env;

/// Initialize the logger based on MVTS_LOG_FILE environment variable
/// If the variable is not set, no logging will occur
static LOGGER_INIT: Lazy<()> = Lazy::new(|| {
    if let Some(log_file_path) = env::get_log_file_path() {
        // Try to open/create the log file
        if let Ok(log_file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file_path)
        {
            // Initialize with WriteLogger for file output
            // Use LevelFilter::Trace to allow all log levels
            let config = ConfigBuilder::new()
                .set_time_format_rfc3339()
                .set_time_offset_to_local()
                .unwrap_or_else(|builder| builder)
                .build();
            
            let _ = WriteLogger::init(LevelFilter::Trace, config, log_file);
        }
    }
});

/// Ensure logger is initialized
pub fn init() {
    Lazy::force(&LOGGER_INIT);
}
