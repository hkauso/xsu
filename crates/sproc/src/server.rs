//! Sproc HTTP endpoints
use askama_axum::Template;
use axum::extract::Path;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Form, Router};
use axum::{extract::State, response::Html, Json};
use std::process::Command;

use crate::model::{
    Registry, RegistryConfiguration, RegistryDeleteRequestBody, RegistryPushRequestBody, Service,
    ServicesConfiguration as ServConf,
};
use xsu_authman::{Database as AuthDatabase, model::AuthError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct APIReturn<T> {
    pub ok: bool,
    pub data: T,
}

#[derive(Deserialize)]
pub struct IndexBody {
    key: String,
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
#[template(path = "error.html")]
struct ErrorTemplate {
    config: RegistryConfiguration,
    error: String,
}

/// Registry 404 response
pub async fn registry_not_found(
    State((registry, _)): State<(Registry, AuthDatabase)>,
) -> impl IntoResponse {
    Html(
        ErrorTemplate {
            config: registry.0.registry,
            error: AuthError::NotFound.to_string(),
        }
        .render()
        .unwrap(),
    )
}

/// GET /registry/:name
pub async fn registry_service_view_request(
    Path(service): Path<String>,
    State((registry, _)): State<(Registry, AuthDatabase)>,
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
    State((registry, _)): State<(Registry, AuthDatabase)>,
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
    State((registry, _)): State<(Registry, AuthDatabase)>,
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
    State((registry, _)): State<(Registry, AuthDatabase)>,
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

/// POST /
pub async fn registry_manage_server_request(
    State((registry, _)): State<(Registry, AuthDatabase)>,
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

// registry api

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

// ...

/// Sproc API endpoints
pub fn sproc_api(config: ServConf) -> Router {
    Router::new()
        .route("/start", post(observe_request))
        .route("/kill", post(kill_request))
        .route("/info", post(info_request))
        .route("/install", post(install_request))
        .route("/uninstall", post(uninstall_request))
        .with_state(config)
}

/// Registry API routes
pub fn registry_api(config: ServConf) -> Router {
    Router::new()
        .route("/:service", get(registry_get_request))
        .route("/:service", post(registry_push_request))
        .route("/:service", delete(registry_delete_request))
        .with_state(Registry::new(config.server))
}

/// Public registry page routes
pub fn registry_public(config: ServConf, database: AuthDatabase) -> Router {
    Router::new()
        .route("/", get(registry_listing_request))
        .route("/", post(registry_manage_server_request))
        .route("/new", get(registry_service_create_request))
        .route("/:service/edit", get(registry_service_edit_request))
        .route("/:service", get(registry_service_view_request))
        // ...
        .fallback(registry_not_found)
        .with_state((Registry::new(config.server), database))
}
