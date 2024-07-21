use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use serde::{Deserialize, Serialize};
use dorsal::DefaultReturn;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Document {
    /// The path to the document under the owner
    pub path: String,
    /// The username of the document's owner
    pub owner: String,
    /// The markdown content of the document
    pub content: String,
    /// The date the document was published
    pub date_published: u128,
    /// The datse the document was edited
    pub date_edited: u128,
    /// Extra document options
    pub metadata: DocumentMetadata,
}

/// Document visibility
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DocumentVisibility {
    /// Visible to anybody
    Public,
}

impl Default for DocumentVisibility {
    fn default() -> Self {
        Self::Public
    }
}

/// Document metadata
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DocumentMetadata {
    /// The visiblity of the document
    #[serde(default)]
    pub visibility: DocumentVisibility,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            visibility: DocumentVisibility::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublicDocument {
    pub path: String,
    pub owner: String,
    pub content: String,
    pub date_published: u128,
    pub date_edited: u128,
    pub metadata: DocumentMetadata,
}

impl From<Document> for PublicDocument {
    fn from(value: Document) -> Self {
        Self {
            path: value.path,
            owner: value.owner,
            content: value.content,
            date_published: value.date_published,
            date_edited: value.date_edited,
            metadata: value.metadata,
        }
    }
}

// props

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentCreate {
    #[serde(default)]
    pub path: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentEdit {
    pub new_content: String,
    #[serde(default)]
    pub new_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentEditMetadata {
    pub password: String,
    pub metadata: DocumentMetadata,
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
            AlreadyExists => String::from("A document with this path already exists."),
            NotAllowed => String::from("You are not allowed to do this."),
            ValueError => String::from("One of the field values given is invalid."),
            NotFound => String::from("No document with this path has been found."),
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
