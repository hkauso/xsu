use askama_axum::Template;
use axum::extract::Path;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{extract::State, response::Html, Router};
use axum_extra::extract::CookieJar;

use xsu_authman::model::Profile;

use crate::config::Config;
use crate::database::Database;
use crate::model::{DatabaseError, Question, QuestionResponse};

pub async fn not_found() -> impl IntoResponse {
    DatabaseError::NotFound.to_string()
}

#[derive(Template)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    config: Config,
    profile: Option<Profile>,
}

#[derive(Template)]
#[template(path = "timeline.html")]
struct TimelineTemplate {
    config: Config,
    profile: Option<Profile>,
    unread: usize,
    responses: Vec<QuestionResponse>,
}

/// GET /
pub async fn homepage_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    // timeline
    if let Some(ref ua) = auth_user {
        let unread = match database
            .get_questions_by_recipient(ua.username.to_owned())
            .await
        {
            Ok(unread) => unread.len(),
            Err(_) => 0,
        };

        let responses = match database
            .get_responses_by_following(ua.username.to_owned())
            .await
        {
            Ok(responses) => responses,
            Err(_) => return Html(DatabaseError::Other.to_string()),
        };

        return Html(
            TimelineTemplate {
                config: database.server_options,
                profile: auth_user,
                unread,
                responses,
            }
            .render()
            .unwrap(),
        );
    }

    // homepage
    Html(
        HomepageTemplate {
            config: database.server_options,
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    config: Config,
    profile: Option<Profile>,
}

/// GET /login
pub async fn login_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(_) => return Html(DatabaseError::NotAllowed.to_string()),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        LoginTemplate {
            config: database.server_options,
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "sign_up.html")]
struct SignUpTemplate {
    config: Config,
    profile: Option<Profile>,
}

/// GET /sign_up
pub async fn sign_up_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(_) => return Html(DatabaseError::NotAllowed.to_string()),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        SignUpTemplate {
            config: database.server_options,
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "profile.html")]
struct ProfileTemplate {
    config: Config,
    profile: Option<Profile>,
    unread: usize,
    other: Profile,
    responses: Vec<QuestionResponse>,
    followers_count: usize,
    following_count: usize,
    is_following: bool,
    metadata: String,
    pinned: Option<QuestionResponse>,
    // ...
    lock_profile: bool,
    disallow_anonymous: bool,
    require_account: bool,
}

/// GET /@:username
pub async fn profile_request(
    jar: CookieJar,
    Path(username): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    let unread = if let Some(ref ua) = auth_user {
        match database
            .get_questions_by_recipient(ua.username.to_owned())
            .await
        {
            Ok(unread) => unread.len(),
            Err(_) => 0,
        }
    } else {
        0
    };

    let other = match database
        .auth
        .get_profile_by_username(username.clone())
        .await
    {
        Ok(ua) => ua,
        Err(e) => return Html(e.to_string()),
    };

    let is_following = if let Some(ref ua) = auth_user {
        match database
            .auth
            .get_follow(ua.username.to_owned(), other.username.clone())
            .await
        {
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    };

    let responses = match database
        .get_responses_by_author(other.username.to_owned())
        .await
    {
        Ok(responses) => responses,
        Err(_) => return Html(DatabaseError::Other.to_string()),
    };

    let pinned = if let Some(pinned) = other.metadata.kv.get("sparkler:pinned") {
        if pinned.is_empty() {
            None
        } else {
            match database.get_response(pinned.to_string()).await {
                Ok(response) => Some(response),
                Err(_) => None,
            }
        }
    } else {
        None
    };

    Html(
        ProfileTemplate {
            config: database.server_options,
            profile: auth_user,
            unread,
            other: other.clone(),
            responses,
            followers_count: database
                .auth
                .get_followers(username.clone())
                .await
                .unwrap_or(Vec::new())
                .len(),
            following_count: database
                .auth
                .get_following(username)
                .await
                .unwrap_or(Vec::new())
                .len(),
            is_following,
            metadata: serde_json::to_string(&other.metadata).unwrap(),
            pinned,
            // ...
            lock_profile: other
                .metadata
                .kv
                .get("sparkler:lock_profile")
                .unwrap_or(&"false".to_string())
                == "true",
            disallow_anonymous: other
                .metadata
                .kv
                .get("sparkler:disallow_anonymous")
                .unwrap_or(&"false".to_string())
                == "true",
            require_account: other
                .metadata
                .kv
                .get("sparkler:require_account")
                .unwrap_or(&"false".to_string())
                == "true",
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "inbox.html")]
struct InboxTemplate {
    config: Config,
    profile: Option<Profile>,
    unread: Vec<Question>,
}

/// GET /inbox
pub async fn inbox_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_string()),
        },
        None => return Html(DatabaseError::NotAllowed.to_string()),
    };

    let unread = match database
        .get_questions_by_recipient(auth_user.username.to_owned())
        .await
    {
        Ok(unread) => unread,
        Err(_) => return Html(DatabaseError::Other.to_string()),
    };

    Html(
        InboxTemplate {
            config: database.server_options,
            profile: Some(auth_user),
            unread,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "account_settings.html")]
struct AccountSettingsTemplate {
    config: Config,
    profile: Option<Profile>,
    unread: usize,
    metadata: String,
}

/// GET /settings
pub async fn account_settings_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_string()),
        },
        None => return Html(DatabaseError::NotAllowed.to_string()),
    };

    let unread = match database
        .get_questions_by_recipient(auth_user.username.to_owned())
        .await
    {
        Ok(unread) => unread.len(),
        Err(_) => 0,
    };

    Html(
        AccountSettingsTemplate {
            config: database.server_options,
            metadata: serde_json::to_string(&auth_user.metadata).unwrap(),
            profile: Some(auth_user),
            unread,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "profile_settings.html")]
struct ProfileSettingsTemplate {
    config: Config,
    profile: Option<Profile>,
    unread: usize,
    metadata: String,
}

/// GET /settings/profile
pub async fn profile_settings_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_string()),
        },
        None => return Html(DatabaseError::NotAllowed.to_string()),
    };

    let unread = match database
        .get_questions_by_recipient(auth_user.username.to_owned())
        .await
    {
        Ok(unread) => unread.len(),
        Err(_) => 0,
    };

    Html(
        ProfileSettingsTemplate {
            config: database.server_options,
            metadata: serde_json::to_string(&auth_user.metadata).unwrap(),
            profile: Some(auth_user),
            unread,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "privacy_settings.html")]
struct PrivacySettingsTemplate {
    config: Config,
    profile: Option<Profile>,
    unread: usize,
    metadata: String,
}

/// GET /settings/privacy
pub async fn privacy_settings_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_string()),
        },
        None => return Html(DatabaseError::NotAllowed.to_string()),
    };

    let unread = match database
        .get_questions_by_recipient(auth_user.username.to_owned())
        .await
    {
        Ok(unread) => unread.len(),
        Err(_) => 0,
    };

    Html(
        PrivacySettingsTemplate {
            config: database.server_options,
            metadata: serde_json::to_string(&auth_user.metadata).unwrap(),
            profile: Some(auth_user),
            unread,
        }
        .render()
        .unwrap(),
    )
}

// ...
pub async fn routes(database: Database) -> Router {
    Router::new()
        .route("/", get(homepage_request))
        .route("/inbox", get(inbox_request))
        .route("/@:username", get(profile_request))
        // settings
        .route("/settings", get(account_settings_request))
        .route("/settings/profile", get(profile_settings_request))
        .route("/settings/privacy", get(privacy_settings_request))
        // auth
        .route("/login", get(login_request))
        .route("/sign_up", get(sign_up_request))
        // ...
        .with_state(database)
}
