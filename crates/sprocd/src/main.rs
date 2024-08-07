//! Sproc daemon
use axum::Router;

/// Main server process
#[tokio::main]
pub async fn main() {
    let config = sproc::model::ServicesConfiguration::get_config();

    // create app
    let app = Router::new().nest_service("/api/sproc", sproc::server::sproc_api(config.clone()));

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", config.server.port))
        .await
        .unwrap();

    println!(
        "Starting server at http://localhost:{}!",
        config.server.port
    );
    axum::serve(listener, app).await.unwrap();
}
