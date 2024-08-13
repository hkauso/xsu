use crate::database::Database;
use axum::body::Bytes;

use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};

pub fn routes(database: Database) -> Router {
    Router::new()
        .route("/:username/avatar", get(avatar_request))
        .route("/:username/banner", get(banner_request))
        // ...
        .with_state(database)
}

// routes

/// Get a profile's avatar image
pub async fn avatar_request(
    Path(username): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user
    let auth_user = match database.auth.get_profile_by_username(username).await {
        Ok(ua) => ua,
        Err(_) => {
            return Bytes::from_static(&[0x0u8]);
        }
    };

    // ...
    let avatar_url = match auth_user.metadata.kv.get("sparkler:avatar_url") {
        Some(r) => r,
        None => "",
    };

    // get profile image
    if avatar_url.is_empty() {
        return Bytes::from_static(&[0]);
    }

    match database.auth.http.get(avatar_url).send().await {
        Ok(r) => r.bytes().await.unwrap(),
        Err(_) => Bytes::from_static(&[0x0u8]),
    }
}

/// Get a profile's banner image
pub async fn banner_request(
    Path(username): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user
    let auth_user = match database.auth.get_profile_by_username(username).await {
        Ok(ua) => ua,
        Err(_) => {
            return Bytes::from_static(&[0x0u8]);
        }
    };

    // ...
    let banner_url = match auth_user.metadata.kv.get("sparkler:banner_url") {
        Some(r) => r,
        None => "",
    };

    // get profile image
    if banner_url.is_empty() {
        return Bytes::from_static(&[0]);
    }

    match database.auth.http.get(banner_url).send().await {
        Ok(r) => r.bytes().await.unwrap(),
        Err(_) => Bytes::from_static(&[0x0u8]),
    }
}
