use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use xsu_dataman::DefaultReturn;
use serde::{Deserialize, Serialize};

/// Basic user structure
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub username: String,
    pub metadata: ProfileMetadata,
    pub group: i32,
    pub joined: u128,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: String::new(),
            username: String::new(),
            metadata: ProfileMetadata::default(),
            group: 0,
            joined: xsu_dataman::utility::unix_epoch_timestamp(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileMetadata {
    /// A brief description provided by the user
    #[serde(default)]
    pub about: String,
    /// A secondary token that can be used to authenticate as the account
    #[serde(default)]
    pub secondary_token: String,
}

impl Default for ProfileMetadata {
    fn default() -> Self {
        Self {
            about: String::new(),
            secondary_token: String::new(),
        }
    }
}

/// xsu system permission
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Permission {
    /// Permission to manage the server through the Sproc UI
    Admin,
    /// Permission to manage other users
    Manager,
}

/// Basic permission group
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Group {
    pub name: String,
    pub id: i32,
    pub permissions: Vec<Permission>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            id: 0,
            permissions: Vec::new(),
        }
    }
}

// props
#[derive(Serialize, Deserialize, Debug)]
pub struct ProfileCreate {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProfileLogin {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetProfileMetadata {
    pub metadata: ProfileMetadata,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetProfileGroup {
    pub group: i32,
}

/// General API errors
pub enum AuthError {
    MustBeUnique,
    NotAllowed,
    ValueError,
    NotFound,
    Other,
}

impl AuthError {
    pub fn to_string(&self) -> String {
        use AuthError::*;
        match self {
            MustBeUnique => String::from("One of the given values must be unique."),
            NotAllowed => String::from("You are not allowed to access this resource."),
            ValueError => String::from("One of the field values given is invalid."),
            NotFound => String::from("No asset with this ID could be found."),
            _ => String::from("An unspecified error has occured"),
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        use crate::model::AuthError::*;
        match self {
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
