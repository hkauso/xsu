use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use serde::{Deserialize, Serialize};
use xsu_dataman::DefaultReturn;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Repository {
    /// The name of the repository
    pub name: String,
    /// The username of the repository owner
    pub owner: String,
    /// The date the repository was published
    pub date_published: u128,
    /// Extra repository options
    pub metadata: RepositoryMetadata,
}

/// Document metadata
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepositoryMetadata {}

impl Default for RepositoryMetadata {
    fn default() -> Self {
        Self {}
    }
}

// props

#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryCreate {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryEditMetadata {
    pub metadata: RepositoryMetadata,
}

/// General API errors
pub enum DatabaseError {
    AlreadyExists,
    NotAllowed,
    ValueError,
    NotFound,
    Other,
}

impl DatabaseError {
    pub fn to_string(&self) -> String {
        use DatabaseError::*;
        match self {
            AlreadyExists => String::from("A repository with this ID already exists."),
            NotAllowed => String::from("You are not allowed to do this."),
            ValueError => String::from("One of the field values given is invalid."),
            NotFound => String::from("No repository with this ID has been found."),
            _ => String::from("An unspecified error has occured"),
        }
    }
}

impl IntoResponse for DatabaseError {
    fn into_response(self) -> Response {
        use DatabaseError::*;
        match self {
            AlreadyExists => (
                StatusCode::BAD_REQUEST,
                Json(DefaultReturn::<u16> {
                    success: false,
                    message: self.to_string(),
                    payload: 400,
                }),
            )
                .into_response(),
            NotAllowed => (
                StatusCode::UNAUTHORIZED,
                Json(DefaultReturn::<u16> {
                    success: false,
                    message: self.to_string(),
                    payload: 401,
                }),
            )
                .into_response(),
            NotFound => (
                StatusCode::NOT_FOUND,
                Json(DefaultReturn::<u16> {
                    success: false,
                    message: self.to_string(),
                    payload: 404,
                }),
            )
                .into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DefaultReturn::<u16> {
                    success: false,
                    message: self.to_string(),
                    payload: 500,
                }),
            )
                .into_response(),
        }
    }
}
