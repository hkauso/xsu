//! Responds to API requests
use crate::model::{DocumentCreate, DocumentEdit, DatabaseError, DocumentEditMetadata, PublicDocument};
use crate::database::Database;
use axum::routing::{delete, put};
use dorsal::DefaultReturn;

use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::CookieJar;

pub fn routes(database: Database) -> Router {
    Router::new()
        .route("/new", post(create_document))
        // documents
        .route("/~:owner/*path", get(get_document))
        .route("/~:owner/*path", post(edit_document))
        .route("/~:owner/*path", put(edit_document_metadata))
        .route("/~:owner/*path", delete(delete_document))
        // ...
        .with_state(database)
}

/// Create a new document (`POST /api/new`)
async fn create_document(
    jar: CookieJar,
    State(database): State<Database>,
    Json(props): Json<DocumentCreate>,
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
    match database.create_document(props, auth_user.username).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: String::from("Document created"),
            payload: (),
        }),
        Err(e) => Json(DefaultReturn {
            success: false,
            message: e.to_string(),
            payload: (),
        }),
    }
}

/// Delete an existing document (`DELETE /api/~:owner/*path`)
async fn delete_document(
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
    match database.delete_document(path, owner, auth_user).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: String::from("Document deleted"),
            payload: (),
        }),
        Err(e) => Json(DefaultReturn {
            success: false,
            message: e.to_string(),
            payload: (),
        }),
    }
}

/// Edit an existing document (`POST /api/~:owner/*path`)
async fn edit_document(
    jar: CookieJar,
    State(database): State<Database>,
    Path((owner, path)): Path<(String, String)>,
    Json(props): Json<DocumentEdit>,
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
        .edit_document(path, owner, props.new_content, props.new_path, auth_user)
        .await
    {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: String::from("Document updated"),
            payload: (),
        }),
        Err(e) => Json(DefaultReturn {
            success: false,
            message: e.to_string(),
            payload: (),
        }),
    }
}

/// Edit an existing document's metadata (`PUT /api/~:owner/*path`)
async fn edit_document_metadata(
    jar: CookieJar,
    State(database): State<Database>,
    Path((owner, path)): Path<(String, String)>,
    Json(props): Json<DocumentEditMetadata>,
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
        .edit_document_metadata(path, owner, props.metadata, auth_user)
        .await
    {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: String::from("Document updated"),
            payload: (),
        }),
        Err(e) => Json(DefaultReturn {
            success: false,
            message: e.to_string(),
            payload: (),
        }),
    }
}

/// Get an existing document (`GET /api/~:owner/*path`)
pub async fn get_document(
    State(database): State<Database>,
    Path((owner, path)): Path<(String, String)>,
) -> Result<Json<DefaultReturn<PublicDocument>>, DatabaseError> {
    match database.get_document(path, owner).await {
        Ok(p) => Ok(Json(DefaultReturn {
            success: true,
            message: String::from("Document exists"),
            payload: p.into(),
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
