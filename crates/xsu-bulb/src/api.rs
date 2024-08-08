//! Responds to API requests
use crate::model::{DatabaseError, Repository, RepositoryCreate, RepositoryEditMetadata};
use crate::database::Database;
use axum::routing::{delete, put};
use xsu_dataman::DefaultReturn;

use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::CookieJar;

pub fn routes(database: Database) -> Router {
    Router::new()
        .route("/new", post(create_repository))
        // repositories
        .route("/~:owner/*name", get(get_repository))
        // .route("/~:owner/*name", post(upload_repository_pack))
        .route("/~:owner/*name", put(edit_repository_metadata))
        .route("/~:owner/*name", delete(delete_repository))
        // ...
        .with_state(database)
}

/// Create a new repository (`POST /api/new`)
async fn create_repository(
    jar: CookieJar,
    State(database): State<Database>,
    Json(props): Json<RepositoryCreate>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: (),
                });
            }
        },
        None => {
            return Json(DefaultReturn {
                success: false,
                message: DatabaseError::NotAllowed.to_string(),
                payload: (),
            });
        }
    };

    // ...
    match database.create_repository(props, auth_user.username).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: String::from("Repository created"),
            payload: (),
        }),
        Err(e) => Json(DefaultReturn {
            success: false,
            message: e.to_string(),
            payload: (),
        }),
    }
}

/// Delete an existing repository (`DELETE /api/~:owner/*name`)
async fn delete_repository(
    jar: CookieJar,
    State(database): State<Database>,
    Path((owner, path)): Path<(String, String)>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: (),
                });
            }
        },
        None => {
            return Json(DefaultReturn {
                success: false,
                message: DatabaseError::NotAllowed.to_string(),
                payload: (),
            });
        }
    };

    // ...
    match database.delete_repository(path, owner, auth_user).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: String::from("Repository deleted"),
            payload: (),
        }),
        Err(e) => Json(DefaultReturn {
            success: false,
            message: e.to_string(),
            payload: (),
        }),
    }
}

/// Edit an existing document's metadata (`PUT /api/~:owner/*name`)
async fn edit_repository_metadata(
    jar: CookieJar,
    State(database): State<Database>,
    Path((owner, path)): Path<(String, String)>,
    Json(props): Json<RepositoryEditMetadata>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: (),
                });
            }
        },
        None => {
            return Json(DefaultReturn {
                success: false,
                message: DatabaseError::NotAllowed.to_string(),
                payload: (),
            });
        }
    };

    // ...
    match database
        .edit_repository_metadata(path, owner, props.metadata, auth_user)
        .await
    {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: String::from("Repository updated"),
            payload: (),
        }),
        Err(e) => Json(DefaultReturn {
            success: false,
            message: e.to_string(),
            payload: (),
        }),
    }
}

/// Get an existing document (`GET /api/~:owner/*name`)
pub async fn get_repository(
    State(database): State<Database>,
    Path((owner, path)): Path<(String, String)>,
) -> Result<Json<DefaultReturn<Repository>>, DatabaseError> {
    match database.get_repository(path, owner).await {
        Ok(p) => Ok(Json(DefaultReturn {
            success: true,
            message: String::from("Repository exists"),
            payload: p,
        })),
        Err(e) => Err(e),
    }
}

// general
pub async fn not_found() -> impl IntoResponse {
    Json(DefaultReturn::<u16> {
        success: false,
        message: String::from("Path does not exist"),
        payload: 404,
    })
}
