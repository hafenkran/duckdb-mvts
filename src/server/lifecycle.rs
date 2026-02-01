use crate::api::api_router;
use duckdb::Connection;
use once_cell::sync::Lazy;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle as StdJoinHandle;
use tokio::sync::oneshot;

// Shared server state
struct ServerState {
    port: Option<u16>,
    handle: Option<StdJoinHandle<()>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    connection: Option<Arc<Mutex<Connection>>>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            port: None,
            handle: None,
            shutdown_tx: None,
            connection: None,
        }
    }

    fn is_running(&self) -> bool {
        self.port.is_some()
    }

    fn get_port(&self) -> Option<u16> {
        self.port
    }
}

// Global server state
static SERVER_STATE: Lazy<Arc<Mutex<ServerState>>> = Lazy::new(|| {
    Arc::new(Mutex::new(ServerState::new())) //
});

// Server context for axum handlers
// Contains shared state that handlers can access
#[derive(Clone)]
pub struct ServerContext {
    // Shared DuckDB connection wrapped in Arc<Mutex> for thread safety
    // Connection is Send but not Sync, so we need Mutex for interior mutability
    connection: Arc<Mutex<Connection>>,
}

/// Get a reference to the connection for use in handlers
impl ServerContext {
    pub fn connection(&self) -> &Arc<Mutex<Connection>> {
        &self.connection
    }
}

pub(crate) fn start_server(port: u16) -> Result<String, Box<dyn Error>> {
    let mut server_state = SERVER_STATE.lock().unwrap();

    // Check if server is already running
    if server_state.is_running() {
        return Err(format!(
            "Server is already running on port {}",
            server_state.get_port().unwrap()
        )
        .into());
    }

    // Get the connection from state
    let connection = server_state.connection.clone().ok_or(
        "No database connection available. Please ensure the extension is properly initialized.",
    )?;

    // Create server context with connection
    let server_context = ServerContext { connection };

    // Build router with server context
    let router = api_router().with_state(server_context);

    // Create shutdown channel for graceful shutdown
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Spawn a thread with its own tokio runtime
    let handle = std::thread::spawn(move || -> () {
        // Create a new tokio runtime for this thread
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        rt.block_on(async {
            let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .expect("Failed to bind to address");

            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .expect("Server error");
        });
    });

    // Store state
    server_state.port = Some(port);
    server_state.handle = Some(handle);
    server_state.shutdown_tx = Some(shutdown_tx);

    Ok(format!("Server started on http://localhost:{}", port))
}

pub(crate) fn stop_server() -> Result<String, Box<dyn Error>> {
    let mut server_state = SERVER_STATE.lock().unwrap();
    if !server_state.is_running() {
        return Err("Server is not running".into());
    }

    let port = server_state.get_port().unwrap();

    // Send shutdown signal for graceful shutdown
    if let Some(shutdown_tx) = server_state.shutdown_tx.take() {
        let _ = shutdown_tx.send(());
    }

    // Wait for the server thread to finish (with a timeout would be better, but keep it simple)
    if let Some(handle) = server_state.handle.take() {
        // Wait for the thread to finish gracefully
        // In a production system, you might want to add a timeout here
        let _ = handle.join();
    }

    server_state.port = None;

    Ok(format!("Server stopped (was running on port {})", port))
}

pub(crate) fn server_status() -> String {
    let server_state = SERVER_STATE.lock().unwrap();

    if server_state.is_running() {
        format!(
            "Server is running on port {}",
            server_state.get_port().unwrap()
        )
    } else {
        "Server is not running".to_string()
    }
}

/// Set the database connection for the server.
pub(crate) fn set_connection(connection: Connection) {
    log::info!("[DuckDB] Server connection: {:?}", connection);
    if let Ok(version) = connection.version() {
        log::info!("[DuckDB] Version: {}", version);
    }

    let mut server_state = SERVER_STATE.lock().unwrap();
    server_state.connection = Some(Arc::new(Mutex::new(connection)));
}
