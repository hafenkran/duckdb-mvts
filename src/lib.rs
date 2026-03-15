extern crate duckdb;
extern crate duckdb_loadable_macros;

pub(crate) mod utils;
pub(crate) mod api;
pub(crate) mod server;
pub(crate) mod store;

use duckdb::{Connection, Result};
use duckdb_loadable_macros::duckdb_entrypoint_c_api;
use server::{ServerStatusScalar, StartServerScalar, StopServerScalar};
use std::error::Error;
use std::io;

#[duckdb_entrypoint_c_api()]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    // Initialize logger
    utils::logger::init();

    // Fail fast if spatial is unavailable. This avoids deferred runtime errors
    // when ST_* functions are first used.
    con.execute_batch("LOAD spatial;").map_err(|e| {
        let msg = format!(
            "mvts requires DuckDB spatial extension. Run: INSTALL spatial; LOAD spatial; ({e})"
        );
        Box::<dyn Error>::from(io::Error::other(msg))
    })?;

    // Register server functions
    con.register_scalar_function::<StartServerScalar>("mvts_start")
        .expect("Failed to register start server function");
    con.register_scalar_function::<StopServerScalar>("mvts_stop")
        .expect("Failed to register stop server function");
    con.register_scalar_function::<ServerStatusScalar>("mvts_status")
        .expect("Failed to register server status function");

    // Create a dedicated connection for the server thread.
    // This avoids using the session connection across threads.
    let server_connection = con
        .try_clone()
        .map_err(|e| -> Box<dyn Error> { Box::new(e) })?;
    server::set_connection(server_connection);

    Ok(())
}
