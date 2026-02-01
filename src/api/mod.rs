use crate::server::ServerContext;
use crate::store::errors::StoreError;
use crate::store::tables as store_tables;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Router,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use tera::{Context, Tera};
use tower::limit::ConcurrencyLimitLayer;

pub mod errors;
mod tables;
mod tiles;

// Initialize Tera template engine
pub(crate) static TEMPLATES: Lazy<Arc<Tera>> = Lazy::new(|| {
    let mut tera = Tera::default();
    tera.add_raw_template(
        "index.html",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/index.html")),
    )
    .expect("Failed to parse embedded index.html template");
    tera.add_raw_template(
        "map-preview.html",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/map-preview.html"
        )),
    )
    .expect("Failed to parse embedded map-preview.html template");
    Arc::new(tera)
});

pub fn api_router() -> Router<ServerContext> {
    Router::new()
        .route("/", axum::routing::get(root_handler))
        .merge(tables::router())
        .merge(tiles::router())
        .layer(ConcurrencyLimitLayer::new(8))
}

pub(crate) async fn run_blocking_db<F, T>(label: &str, f: F) -> Result<T, StoreError>
where
    F: FnOnce() -> Result<T, StoreError> + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(result) => result,
        Err(e) => {
            log::error!("[DB] Error joining {} task: {:?}", label, e);
            Err(StoreError::DatabaseError {
                source: Box::new(e),
            })
        }
    }
}

/// Root handler - renders index.html template with table data
async fn root_handler(State(server_context): State<ServerContext>) -> Response {
    let connection = server_context.connection().clone();

    // Get tables from store layer
    let tables_data = match run_blocking_db("root_handler", move || {
        store_tables::query_geometry_tables(&connection)
    })
    .await
    {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", e),
            )
                .into_response();
        }
    };

    // Prepare template context
    let mut context = Context::new();
    context.insert("tables", &tables_data);

    // Render template
    let rendered = match TEMPLATES.render("index.html", &context) {
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

/// Add CORS headers to a response
pub(crate) fn add_cors(mut resp: Response) -> Response {
    let headers = resp.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET, HEAD, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "Content-Type".parse().unwrap(),
    );
    resp
}
