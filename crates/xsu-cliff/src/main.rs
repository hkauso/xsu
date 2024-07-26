//! Cliff server
use askama_axum::Template;
use axum::extract::Path;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, get_service};
use axum::{extract::State, response::Html, Router};
use axum_extra::extract::CookieJar;

use xsu_authman::{Database as AuthDatabase, api as AuthApi, model::Profile};
use xsu_docshare::{Database as DsDatabase, api as DsApi, model::DatabaseError as DsError};
use xsu_dataman::config::Config as DataConf;

use sproc::model::{Registry, RegistryConfiguration, ServicesConfiguration as ServConf};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct APIReturn<T> {
    pub ok: bool,
    pub data: T,
}

#[derive(Template)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    config: RegistryConfiguration,
}

#[derive(Template)]
#[template(path = "auth.html")]
struct AuthTemplate {
    config: RegistryConfiguration,
}

#[derive(Template)]
#[template(path = "myaccount.html")]
struct MyAccountTemplate {
    config: RegistryConfiguration,
    me: Profile,
    my_metadata: String,
}

#[derive(Template)]
#[template(path = "profile.html")]
struct ProfileViewTemplate {
    config: RegistryConfiguration,
    profile: Profile,
}

#[derive(Template)]
#[template(path = "ds_listing.html")]
struct DocshareListingTemplate {
    config: RegistryConfiguration,
    pages: Vec<xsu_docshare::model::Document>,
    username: String,
}

#[derive(Template)]
#[template(path = "ds_view.html")]
struct DocshareViewTemplate {
    config: RegistryConfiguration,
    doc: xsu_docshare::model::Document,
}

#[derive(Template)]
#[template(path = "ds_edit.html")]
struct DocshareEditTemplate {
    config: RegistryConfiguration,
    doc: xsu_docshare::model::Document,
}

#[derive(Template)]
#[template(path = "ds_create.html")]
struct DocshareCreateTemplate {
    config: RegistryConfiguration,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    config: RegistryConfiguration,
    error: String,
}

/// GET /
pub async fn homepage_request(
    State((registry, _)): State<(Registry, AuthDatabase)>,
) -> impl IntoResponse {
    Html(
        HomepageTemplate {
            config: registry.0.registry,
        }
        .render()
        .unwrap(),
    )
}

/// GET /account
pub async fn auth_request(
    jar: CookieJar,
    State((registry, database)): State<(Registry, AuthDatabase)>,
) -> impl IntoResponse {
    // my account
    if let Some(cookie) = jar.get("__Secure-Token") {
        match database
            .get_profile_by_unhashed(cookie.value_trimmed().to_string())
            .await
        {
            Ok(ua) => {
                return Html(
                    MyAccountTemplate {
                        config: registry.0.registry.clone(),
                        me: ua.clone(),
                        my_metadata: serde_json::to_string(&ua.metadata).unwrap(),
                    }
                    .render()
                    .unwrap(),
                )
            }
            Err(e) => return Html(e.to_string()),
        }
    };

    // default
    Html(
        AuthTemplate {
            config: registry.0.registry.clone(),
        }
        .render()
        .unwrap(),
    )
}

/// GET /~:username
pub async fn profile_view_request(
    Path(username): Path<String>,
    State((registry, database)): State<(Registry, AuthDatabase)>,
) -> impl IntoResponse {
    match database.get_profile_by_username(username).await {
        Ok(ua) => {
            return Html(
                ProfileViewTemplate {
                    config: registry.0.registry.clone(),
                    profile: ua,
                }
                .render()
                .unwrap(),
            )
        }
        Err(e) => return Html(e.to_string()),
    }
}

/// GET /doc
pub async fn docshare_index_request(
    jar: CookieJar,
    State((registry, database)): State<(Registry, DsDatabase)>,
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
                return Html(
                    ErrorTemplate {
                        config: registry.0.registry,
                        error: e.to_string(),
                    }
                    .render()
                    .unwrap(),
                );
            }
        },
        None => {
            return Html(
                ErrorTemplate {
                    config: registry.0.registry,
                    error: DsError::NotAllowed.to_string(),
                }
                .render()
                .unwrap(),
            );
        }
    };

    // get pages
    let pages = match database
        .get_documents_by_owner(auth_user.username.clone())
        .await
    {
        Ok(ls) => ls,
        Err(e) => {
            return Html(
                ErrorTemplate {
                    config: registry.0.registry,
                    error: e.to_string(),
                }
                .render()
                .unwrap(),
            )
        }
    };

    // return
    Html(
        DocshareListingTemplate {
            config: registry.0.registry,
            pages,
            username: auth_user.username,
        }
        .render()
        .unwrap(),
    )
}

/// GET /doc/~:owner/*path
pub async fn docshare_view_request(
    Path((owner, path)): Path<(String, String)>,
    State((registry, database)): State<(Registry, DsDatabase)>,
) -> impl IntoResponse {
    let doc = match database.get_document(path, owner).await {
        Ok(ls) => ls,
        Err(e) => {
            return Html(
                ErrorTemplate {
                    config: registry.0.registry,
                    error: e.to_string(),
                }
                .render()
                .unwrap(),
            )
        }
    };

    // return
    Html(
        DocshareViewTemplate {
            config: registry.0.registry,
            doc,
        }
        .render()
        .unwrap(),
    )
}

/// GET /doc/edit/~:owner/*path
pub async fn docshare_edit_request(
    Path((owner, path)): Path<(String, String)>,
    State((registry, database)): State<(Registry, DsDatabase)>,
) -> impl IntoResponse {
    let doc = match database.get_document(path, owner).await {
        Ok(ls) => ls,
        Err(e) => {
            return Html(
                ErrorTemplate {
                    config: registry.0.registry,
                    error: e.to_string(),
                }
                .render()
                .unwrap(),
            )
        }
    };

    // return
    Html(
        DocshareEditTemplate {
            config: registry.0.registry,
            doc,
        }
        .render()
        .unwrap(),
    )
}

/// GET /doc/new
pub async fn docshare_create_request(
    jar: CookieJar,
    State((registry, database)): State<(Registry, DsDatabase)>,
) -> impl IntoResponse {
    // get user from token
    match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed().to_string())
            .await
        {
            Ok(ua) => ua,
            Err(e) => {
                return Html(
                    ErrorTemplate {
                        config: registry.0.registry,
                        error: e.to_string(),
                    }
                    .render()
                    .unwrap(),
                );
            }
        },
        None => {
            return Html(
                ErrorTemplate {
                    config: registry.0.registry,
                    error: DsError::NotAllowed.to_string(),
                }
                .render()
                .unwrap(),
            );
        }
    };

    // return
    Html(
        DocshareCreateTemplate {
            config: registry.0.registry,
        }
        .render()
        .unwrap(),
    )
}

/// Docshare page routes
pub fn ds_public(config: ServConf, database: DsDatabase) -> Router {
    Router::new()
        .route("/", get(docshare_index_request))
        .route("/new", get(docshare_create_request))
        .route("/edit/~:owner/*path", get(docshare_edit_request))
        .route("/~:owner/*path", get(docshare_view_request))
        // ...
        .with_state((Registry::new(config.server), database))
}

/// Redirect /~:username/*path to /doc
async fn ds_redirect(Path((username, path)): Path<(String, String)>) -> impl IntoResponse {
    Redirect::to(&format!("/doc/~{username}/{path}"))
}

/// Redirect /doc/~:username to /~username
async fn profile_redirect(Path(username): Path<String>) -> impl IntoResponse {
    Redirect::to(&format!("/~{username}"))
}

/// Main server process
#[tokio::main]
pub async fn main() {
    let config = sproc::model::ServicesConfiguration::get_config();

    let home = std::env::var("HOME").expect("failed to read $HOME");
    let static_dir = format!("{home}/.config/xsu-apps/sproc/static");

    // create databases
    let auth_database = AuthDatabase::new(
        DataConf::get_config().connection, // pull connection config from config file
        xsu_authman::ServerOptions::truthy(),
    )
    .await;
    auth_database.init().await;

    let ds_database = DsDatabase::new(
        DataConf::get_config().connection, // pull connection config from config file
        xsu_docshare::ServerOptions::truthy(),
        auth_database.clone(),
    )
    .await;
    ds_database.init().await;

    // create app
    let app = Router::new()
        .route("/", get(homepage_request))
        .route("/account", get(auth_request))
        // api
        .nest_service("/api/sproc", sproc::server::sproc_api(config.clone()))
        .nest_service("/api/registry", sproc::server::registry_api(config.clone()))
        .nest_service("/api/auth", AuthApi::routes(auth_database.clone()))
        .nest_service("/api/ds", DsApi::routes(ds_database.clone()))
        // extras
        .nest_service(
            "/registry",
            sproc::server::registry_public(config.clone(), auth_database.clone()),
        )
        .nest_service("/doc", ds_public(config.clone(), ds_database.clone()))
        .route("/~:username/*path", get(ds_redirect))
        .route("/doc/~:username", get(profile_redirect))
        .route("/~:username", get(profile_view_request))
        // ...
        .nest_service(
            "/static",
            get_service(tower_http::services::ServeDir::new(static_dir)),
        )
        .with_state((Registry::new(config.clone().server), auth_database.clone()));

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", config.server.port))
        .await
        .unwrap();

    println!(
        "Starting server at http://localhost:{}!",
        config.server.port
    );
    axum::serve(listener, app).await.unwrap();
}
