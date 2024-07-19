//! Sproc HTTP endpoints
use askama_axum::Template;
use axum::extract::{Form, Path, Query};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{delete, get};
use axum::{extract::State, response::Html, routing::post, Json, Router};
use std::process::Command;

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
#[template(path = "noresults.html")]
struct NoResultsTemplate {
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

/// A sub-action on the [`IndexTemplate`]
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexSubAction {
    /// Nothing
    None,
    /// Service listing
    List,
    /// Service editor
    Edit,
    /// Service creator
    Create,
    /// Manage running services
    Manage,
}

impl Default for IndexSubAction {
    fn default() -> Self {
        IndexSubAction::None
    }
}

#[derive(Deserialize)]
pub struct IndexQuery {
    #[serde(default)]
    read: String,
    #[serde(default)]
    action: IndexSubAction,
}

#[derive(Deserialize)]
pub struct IndexBody {
    key: String,
}

pub async fn registry_index_request(
    Query(props): Query<IndexQuery>,
    State(registry): State<Registry>,
    body: Option<Form<IndexBody>>,
) -> impl IntoResponse {
    // POST
    if let Some(body) = body {
        // check key
        if body.key != registry.0.key {
            return Html("Not allowed".to_string());
        }

        // service manager
        if props.action == IndexSubAction::Manage {
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
            return Html(
                ManageTemplate {
                    config: registry.0.registry.clone(),
                    services,
                    key: body.key.clone(),
                }
                .render()
                .unwrap(),
            );
        }
    }

    // view specific service
    if !props.read.is_empty() {
        // edit
        if props.action == IndexSubAction::Edit {
            return Html(
                EditTemplate {
                    config: registry.0.registry.clone(),
                    package: match registry.get(props.read.clone().replace(".toml", "")) {
                        Ok(p) => (props.read, toml::from_str(&p).unwrap(), p),
                        Err(e) => return Html(e.to_string()),
                    },
                }
                .render()
                .unwrap(),
            );
        }

        // view
        return Html(
            ViewTemplate {
                config: registry.0.registry.clone(),
                package: match registry.get(props.read.clone().replace(".toml", "")) {
                    Ok(p) => (props.read, toml::from_str(&p).unwrap(), p),
                    Err(e) => return Html(e.to_string()),
                },
            }
            .render()
            .unwrap(),
        );
    }

    // create
    if props.action == IndexSubAction::Create {
        return Html(
            CreateTemplate {
                config: registry.0.registry.clone(),
            }
            .render()
            .unwrap(),
        );
    }
    // list
    else if props.action == IndexSubAction::List {
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
        return Html(
            ListingTemplate {
                config: registry.0.registry,
                packages,
            }
            .render()
            .unwrap(),
        );
    }

    // default
    return Html(
        NoResultsTemplate {
            config: registry.0.registry,
        }
        .render()
        .unwrap(),
    );
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

// ...
/// Registry routes
pub fn registry(config: ServConf) -> Router {
    Router::new()
        .route("/", get(registry_index_request))
        .route("/", post(registry_index_request))
        .route("/:service", get(registry_get_request))
        .route("/:service", post(registry_push_request))
        .route("/:service", delete(registry_delete_request))
        .with_state(Registry::new(config.server))
}

/// Main server process
pub async fn server(config: ServConf) {
    let app = Router::new()
        .route("/start", post(observe_request))
        .route("/kill", post(kill_request))
        .route("/info", post(info_request))
        .route("/install", post(install_request))
        .route("/uninstall", post(uninstall_request))
        .route("/", get(|| async { Redirect::to("/registry") }))
        .nest_service("/registry", registry(config.clone()))
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
