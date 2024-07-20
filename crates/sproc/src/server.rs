//! Sproc HTTP endpoints
use askama_axum::Template;
use axum::extract::{Form, Path};
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::{extract::State, response::Html, routing::post, Json, Router};
use axum_extra::extract::CookieJar;
use std::process::Command;
use xsu_authman::{Database, api, model::Profile};

use crate::model::{
    Registry, RegistryConfiguration, RegistryDeleteRequestBody, RegistryPushRequestBody, Service,
    ServicesConfiguration as ServConf,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct APIReturn<T> {
    pub ok: bool,
    pub data: T,
}

/// Basic request body for operations on a specific service
#[derive(Serialize, Deserialize)]
pub struct BasicServiceRequestBody {
    /// The name of the service
    pub service: String,
    /// Auth key
    pub key: String,
}

/// Basic request body for operations on a specific service
#[derive(Serialize, Deserialize)]
pub struct InstallRequestBody {
    /// The registry to install from
    pub registry: String,
    /// The name of the service
    pub service: String,
    /// Auth key
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
    State(config): State<ServConf>, // inital config from server start
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

/// Kill a service (POST /kill)
pub async fn kill_request(
    State(config): State<ServConf>, // inital config from server start
    Json(body): Json<BasicServiceRequestBody>,
) -> impl IntoResponse {
    // check key
    if body.key != config.server.key {
        return Json(APIReturn::<u16> {
            ok: false,
            data: 401,
        });
    }

    // get updated config
    let mut config = ServConf::get_config();

    // kill
    // TODO: try to clone less
    if let Err(_) = Service::kill(body.service.clone(), config.clone()) {
        return Json(APIReturn::<u16> {
            ok: false,
            data: 400,
        });
    };

    // update config
    config.service_states.remove(&body.service);
    ServConf::update_config(config.clone()).unwrap();

    // return
    Json(APIReturn::<u16> {
        ok: true,
        data: 200,
    })
}

/// Get service info (POST /info)
pub async fn info_request(
    State(config): State<ServConf>, // inital config from server start
    Json(body): Json<BasicServiceRequestBody>,
) -> impl IntoResponse {
    // check key
    if body.key != config.server.key {
        return Json(APIReturn::<String> {
            ok: false,
            data: String::new(),
        });
    }

    // get updated config
    let config = ServConf::get_config();

    // return
    Json(APIReturn::<String> {
        ok: true,
        data: match Service::info(body.service.clone(), config.service_states) {
            Ok(i) => i,
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

/// Install a service (POST /install)
pub async fn install_request(
    State(config): State<ServConf>, // inital config from server start
    Json(body): Json<InstallRequestBody>,
) -> impl IntoResponse {
    // check key
    if body.key != config.server.key {
        return Json(APIReturn::<String> {
            ok: false,
            data: String::new(),
        });
    }

    // run sproc command
    let mut cmd = Command::new("sproc");
    cmd.arg("install");
    cmd.arg(body.registry.replace("https://", "").replace("http://", ""));
    cmd.arg(body.service);
    cmd.spawn().expect("failed to spawn");

    // ...
    Json(APIReturn::<String> {
        ok: true,
        data: String::new(),
    })
}

/// Uninstall a service (POST /uninstall)
pub async fn uninstall_request(
    State(config): State<ServConf>, // inital config from server start
    Json(body): Json<BasicServiceRequestBody>,
) -> impl IntoResponse {
    // check key
    if body.key != config.server.key {
        return Json(APIReturn::<String> {
            ok: false,
            data: String::new(),
        });
    }

    // run sproc command
    let mut cmd = Command::new("sproc");
    cmd.arg("uninstall");
    cmd.arg(body.service);

    // ...
    Json(APIReturn::<String> {
        ok: true,
        data: match cmd.output() {
            Ok(s) => s.status.to_string(),
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

// registry

#[derive(Template)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    config: RegistryConfiguration,
}

#[derive(Template)]
#[template(path = "listing.html")]
struct ListingTemplate {
    config: RegistryConfiguration,
    packages: Vec<String>,
}

#[derive(Template)]
#[template(path = "create.html")]
struct CreateTemplate {
    config: RegistryConfiguration,
}

#[derive(Template)]
#[template(path = "view.html")]
struct ViewTemplate {
    config: RegistryConfiguration,
    package: (String, Service, String),
}

#[derive(Template)]
#[template(path = "edit.html")]
struct EditTemplate {
    config: RegistryConfiguration,
    package: (String, Service, String),
}

#[derive(Template)]
#[template(path = "manage.html")]
struct ManageTemplate {
    config: RegistryConfiguration,
    services: Vec<(String, Service, bool)>,
    key: String,
}

#[derive(Template)]
#[template(path = "page.html")]
struct PageTemplate {
    config: RegistryConfiguration,
    page: (String, String),
}

#[derive(Template)]
#[template(path = "edit_page.html")]
struct EditPageTemplate {
    config: RegistryConfiguration,
    page: (String, String),
}

#[derive(Template)]
#[template(path = "create_page.html")]
struct PageCreateTemplate {
    config: RegistryConfiguration,
}

#[derive(Template)]
#[template(path = "page_listing.html")]
struct BookPageListingTemplate {
    config: RegistryConfiguration,
    path: String,
    pages: Vec<String>,
}

#[derive(Template)]
#[template(path = "custom.html")]
struct CustomTemplate {
    config: RegistryConfiguration,
    content: String,
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
}

#[derive(Deserialize)]
pub struct IndexBody {
    key: String,
}

/// GET /
pub async fn registry_index_request(
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    Html(
        HomepageTemplate {
            config: registry.0.registry,
        }
        .render()
        .unwrap(),
    )
}

/// GET /registry/:name
pub async fn registry_service_view_request(
    Path(service): Path<String>,
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    Html(
        ViewTemplate {
            config: registry.0.registry.clone(),
            package: match registry.get(service.clone().replace(".toml", "")) {
                Ok(p) => (service, toml::from_str(&p).unwrap(), p),
                Err(e) => return Html(e.to_string()),
            },
        }
        .render()
        .unwrap(),
    )
}

/// GET /registry/:name/edit
pub async fn registry_service_edit_request(
    Path(service): Path<String>,
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    Html(
        EditTemplate {
            config: registry.0.registry.clone(),
            package: match registry.get(service.clone().replace(".toml", "")) {
                Ok(p) => (service, toml::from_str(&p).unwrap(), p),
                Err(e) => return Html(e.to_string()),
            },
        }
        .render()
        .unwrap(),
    )
}

/// GET /registry/new
pub async fn registry_service_create_request(
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    Html(
        CreateTemplate {
            config: registry.0.registry.clone(),
        }
        .render()
        .unwrap(),
    )
}

/// GET /registry
pub async fn registry_listing_request(
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    // get services
    let mut packages = Vec::new();

    for package in match std::fs::read_dir(registry.1) {
        Ok(ls) => ls,
        Err(e) => return Html(e.to_string()),
    } {
        // what in the world
        packages.push(package.unwrap().file_name().to_string_lossy().to_string());
    }

    // return
    Html(
        ListingTemplate {
            config: registry.0.registry,
            packages,
        }
        .render()
        .unwrap(),
    )
}

/// GET /book/:path
pub async fn registry_book_page_view_request(
    Path(path): Path<String>,
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    // if this path links to a directory, show listing
    let metadata = match registry.get_page_metadata(&path) {
        Ok(m) => m,
        Err(e) => return Html(e.to_string()),
    };

    if metadata.is_dir() {
        // get pages
        let mut pages = Vec::new();

        for page in match std::fs::read_dir(format!("{}/{path}", registry.2)) {
            Ok(ls) => ls,
            Err(e) => return Html(e.to_string()),
        } {
            // what in the world
            pages.push(page.unwrap().file_name().to_string_lossy().to_string());
        }

        // return
        return Html(
            BookPageListingTemplate {
                config: registry.0.registry,
                pages,
                path,
            }
            .render()
            .unwrap(),
        );
    }

    // return
    Html(
        PageTemplate {
            config: registry.0.registry.clone(),
            page: match registry.get_page(path.clone()) {
                Ok(p) => (path, p),
                Err(e) => return Html(e.to_string()),
            },
        }
        .render()
        .unwrap(),
    )
}

/// GET /book
pub async fn registry_base_listing_request(
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    // get pages
    let mut pages = Vec::new();

    for page in match std::fs::read_dir(format!("{}", registry.2)) {
        Ok(ls) => ls,
        Err(e) => return Html(e.to_string()),
    } {
        // what in the world
        pages.push(page.unwrap().file_name().to_string_lossy().to_string());
    }

    // return
    return Html(
        BookPageListingTemplate {
            config: registry.0.registry,
            pages,
            path: String::new(),
        }
        .render()
        .unwrap(),
    );
}

/// GET /book_admin/edit/:path
pub async fn registry_book_page_edit_request(
    Path(path): Path<String>,
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    Html(
        EditPageTemplate {
            config: registry.0.registry.clone(),
            page: match registry.get_page(path.clone()) {
                Ok(p) => (path, p),
                Err(e) => return Html(e.to_string()),
            },
        }
        .render()
        .unwrap(),
    )
}

/// GET /book_admin/new
pub async fn registry_book_page_create_request(
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    Html(
        PageCreateTemplate {
            config: registry.0.registry.clone(),
        }
        .render()
        .unwrap(),
    )
}

/// POST /
pub async fn registry_manage_server_request(
    State((registry, _)): State<(Registry, Database)>,
    Form(body): Form<IndexBody>,
) -> impl IntoResponse {
    // check key
    if body.key != registry.0.key {
        return Html("Not allowed".to_string());
    }

    // service manager
    let mut services = Vec::new();
    let config = ServConf::get_config();

    for service in config.services {
        services.push((
            service.0.clone(),
            service.1,
            config.service_states.contains_key(&service.0),
        ));
    }

    // return
    Html(
        ManageTemplate {
            config: registry.0.registry.clone(),
            services,
            key: body.key.clone(),
        }
        .render()
        .unwrap(),
    )
}

/// GET /custom/
///
/// Used for custom app pages for services.
pub async fn registry_custom_request(
    Path(path): Path<String>,
    State((registry, _)): State<(Registry, Database)>,
) -> impl IntoResponse {
    Html(
        CustomTemplate {
            config: registry.0.registry.clone(),
            content: match registry.get_html(path.clone().replace(".html", "")) {
                Ok(p) => p,
                Err(e) => return Html(e.to_string()),
            },
        }
        .render()
        .unwrap(),
    )
}

/// GET /account
pub async fn registry_auth_request(
    jar: CookieJar,
    State((registry, database)): State<(Registry, Database)>,
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
                        me: ua,
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

/// [`Registry::get`]
pub async fn registry_get_request(
    Path(name): Path<String>,
    State(registry): State<Registry>, // inital config from server start
) -> impl IntoResponse {
    Json(APIReturn::<String> {
        ok: true,
        data: match registry.get(name) {
            Ok(i) => i,
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

/// [`Registry::push`]
pub async fn registry_push_request(
    Path(name): Path<String>,
    State(registry): State<Registry>, // inital config from server start
    Json(props): Json<RegistryPushRequestBody>,
) -> impl IntoResponse {
    Json(APIReturn::<String> {
        ok: true,
        data: match registry.push(props, name) {
            Ok(_) => String::new(),
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

/// [`Registry::delete`]
pub async fn registry_delete_request(
    Path(name): Path<String>,
    State(registry): State<Registry>, // inital config from server start
    Json(props): Json<RegistryDeleteRequestBody>,
) -> impl IntoResponse {
    Json(APIReturn::<String> {
        ok: true,
        data: match registry.delete(props, name) {
            Ok(_) => String::new(),
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

/// [`Registry::get_page`]
pub async fn registry_get_page_request(
    Path(path): Path<String>,
    State(registry): State<Registry>, // inital config from server start
) -> impl IntoResponse {
    // if this path links to a directory, return error
    let metadata = match registry.get_page_metadata(&path) {
        Ok(m) => m,
        Err(e) => {
            return Json(APIReturn::<String> {
                ok: false,
                data: e.to_string(),
            })
        }
    };

    if metadata.is_dir() {
        return Json(APIReturn::<String> {
            ok: false,
            data: "Cannot read directory".to_string(),
        });
    }

    // view specific page
    Json(APIReturn::<String> {
        ok: true,
        data: match registry.get_page(path) {
            Ok(i) => i,
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

/// [`Registry::push_page`]
pub async fn registry_push_page_request(
    Path(path): Path<String>,
    State(registry): State<Registry>, // inital config from server start
    Json(props): Json<RegistryPushRequestBody>,
) -> impl IntoResponse {
    Json(APIReturn::<String> {
        ok: true,
        data: match registry.push_page(props, path) {
            Ok(_) => String::new(),
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

/// [`Registry::delete_page`]
pub async fn registry_delete_page_request(
    Path(path): Path<String>,
    State(registry): State<Registry>, // inital config from server start
    Json(props): Json<RegistryDeleteRequestBody>,
) -> impl IntoResponse {
    Json(APIReturn::<String> {
        ok: true,
        data: match registry.delete_page(props, path) {
            Ok(_) => String::new(),
            Err(e) => {
                return Json(APIReturn::<String> {
                    ok: false,
                    data: e.to_string(),
                })
            }
        },
    })
}

// ...
/// Registry API routes
pub fn registry_api(config: ServConf) -> Router {
    Router::new()
        .route("/:service", get(registry_get_request))
        .route("/:service", post(registry_push_request))
        .route("/:service", delete(registry_delete_request))
        .route("/book/*name", get(registry_get_page_request))
        .route("/book/*name", post(registry_push_page_request))
        .route("/book/*name", delete(registry_delete_page_request))
        .with_state(Registry::new(config.server))
}

/// Public registry page routes
pub fn registry_public(config: ServConf, database: Database) -> Router {
    Router::new()
        .route("/", get(registry_index_request))
        .route("/", post(registry_manage_server_request))
        .route("/account", get(registry_auth_request))
        .route("/custom/*path", get(registry_custom_request))
        // registry
        .route("/registry", get(registry_listing_request))
        .route("/registry/new", get(registry_service_create_request))
        .route(
            "/registry/:service/edit",
            get(registry_service_edit_request),
        )
        .route("/registry/:service", get(registry_service_view_request))
        // books
        .route("/book_admin/new", get(registry_book_page_create_request))
        .route(
            "/book_admin/edit/*path",
            get(registry_book_page_edit_request),
        )
        .route("/book", get(registry_base_listing_request))
        .route("/book/*path", get(registry_book_page_view_request))
        // ...
        .with_state((Registry::new(config.server), database))
}

/// Main server process
pub async fn server(config: ServConf) {
    // create database
    let database = Database::new(
        Database::env_options(),
        xsu_authman::ServerOptions::truthy(),
    )
    .await;
    database.init().await;

    // create app
    let app = Router::new()
        .route("/start", post(observe_request))
        .route("/kill", post(kill_request))
        .route("/info", post(info_request))
        .route("/install", post(install_request))
        .route("/uninstall", post(uninstall_request))
        .nest_service("/api/registry", registry_api(config.clone()))
        .nest_service("/api/auth", api::routes(database.clone()))
        .nest_service("/", registry_public(config.clone(), database.clone()))
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
