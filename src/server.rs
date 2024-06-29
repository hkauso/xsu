//! Sproc HTTP socket
use axum::response::IntoResponse;
use axum::{extract::State, routing::post, Json, Router};

use crate::model::{Service, ServicesConfiguration as ServConf};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct APIReturn<T> {
    pub ok: bool,
    pub data: T,
}

/// Basic request body for operations on a specific service
#[derive(Serialize, Deserialize)]
pub struct BasicServiceRequestBody {
    pub service: String,
    pub key: String,
}

/// Default 404 response
/// { "ok": false, "data": (http status) }
pub async fn not_found() -> impl IntoResponse {
    Json(APIReturn::<u16> {
        ok: false,
        data: 404,
    })
}

/// Start and observe a service (POST /start)
pub async fn observe_request(
    State(config): State<ServConf>,
    Json(body): Json<BasicServiceRequestBody>,
) -> impl IntoResponse {
    // check key
    if body.key != config.server.key {
        return Json(APIReturn::<u16> {
            ok: false,
            data: 401,
        });
    }

    // start
    if let Err(_) = Service::spawn(body.service.clone()).await {
        return Json(APIReturn::<u16> {
            ok: false,
            data: 400,
        });
    };

    // return
    Json(APIReturn::<u16> {
        ok: true,
        data: 200,
    })
}

/// Main server process
pub async fn server(config: ServConf) {
    let app = Router::new()
        .route("/start", post(observe_request))
        .fallback(not_found)
        .with_state(config.clone());

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", config.server.port))
        .await
        .unwrap();

    println!(
        "Starting server at http://localhost:{}!",
        config.server.port
    );
    axum::serve(listener, app).await.unwrap();
}
