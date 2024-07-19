//! Sproc process manager
use clap::{Parser, Subcommand};
use server::APIReturn;
use std::io::{Error, ErrorKind, Result};

use xsu_util::{
    fs,
    process::{no, yes},
};

// ...
#[derive(Parser, Debug)]
#[command(version, about, long_about = Option::Some("Sproc process manager"))]
#[command(propagate_version = true)]
struct Sproc {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Load configuration file
    Pin { path: String },
    /// Run a configured service
    Run { names: Vec<String> },
    /// Spawn a service as a new task (HTTP server required: `srpoc serve`)
    Spawn { names: Vec<String> },
    /// Run all services
    RunAll {},
    /// Kill a running service
    Kill { names: Vec<String> },
    /// Kill all services
    KillAll {},
    /// Get information about a running service
    Info { name: String },
    /// Get information about all services
    InfoAll {},
    /// Wait for service to stop and update its state accordingly
    Track { name: String },
    /// Start server
    Serve {},
    /// View pinned config
    Pinned {},
    /// Merge services from given file into **source** configuration file (unpinned file)
    Merge { path: String },
    /// Pull services from given file into **pinned** configuration file (use `merge` for unpinned)
    Pull { path: String },
    /// Install services from the given remote registry address (HTTP assumed)
    Install {
        registry: String,
        names: Vec<String>,
    },
    /// "Uninstall" services given their names
    Uninstall { names: Vec<String> },
}

// ...
pub mod model;
pub mod server;

use model::{Service, ServiceState, ServiceType, ServicesConfiguration};

// real main
async fn sproc<'a>() -> Result<&'a str> {
    // init
    let args = Sproc::parse();

    // get current config
    let mut services = ServicesConfiguration::get_config();

    // ...
    match &args.command {
        // pin
        Commands::Pin { path } => {
            match fs::read(path) {
                Ok(s) => {
                    // make sure no services are running
                    for service in services.service_states {
                        if service.1 .0 == ServiceState::Running {
                            return Err(Error::new(ErrorKind::Other, "Cannot pin config with active service. Please run \"sproc kill-all\""));
                        }
                    }

                    // ...
                    let mut config: ServicesConfiguration = toml::from_str(&s).unwrap();

                    // set source to absolute path
                    config.source = fs::canonicalize(path)?
                        .as_path()
                        .to_str()
                        .unwrap()
                        .to_string();

                    // return
                    ServicesConfiguration::update_config(config)?;
                    Ok("Services loaded.")
                }
                Err(e) => Err(e),
            }
        }
        // run
        Commands::Run { names } => {
            if names.len() == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Please provide at least 1 service name.",
                ));
            }

            for name in names {
                match services.services.get(name) {
                    Some(_) => {
                        let mut process = Service::run(name.to_string(), services.clone())?;

                        // if this is an application, wait for it to close and then continue
                        if process.0.r#type == ServiceType::Application {
                            process.1.wait()?;
                            continue; // we must continue so we don't try to add the service pid
                        }

                        // ...
                        services
                            .service_states
                            .insert(name.to_string(), (ServiceState::Running, process.1.id()));
                    }
                    None => return Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
                }
            }

            ServicesConfiguration::update_config(services)?;
            Ok("Started all requested services.")
        }
        // spawn
        Commands::Spawn { names } => {
            if names.len() == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Please provide at least 1 service name.",
                ));
            }

            // post request
            let client = reqwest::Client::new();

            for name in names {
                match services.services.get(name) {
                    Some(_) => {
                        match client
                            .post(format!("http://localhost:{}/start", services.server.port))
                            .body(format!(
                                "{{ \"service\":\"{}\",\"key\":\"{}\" }}",
                                name, services.server.key
                            ))
                            .header("Content-Type", "application/json")
                            .send()
                            .await
                        {
                            Ok(r) => {
                                let res = r.text().await.expect("Failed to read body");
                                println!("info: body: {}", res);
                            }
                            Err(e) => {
                                return Err(Error::new(ErrorKind::NotConnected, e.to_string()))
                            }
                        }
                    }
                    None => return Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
                }
            }

            Ok("Sent all requested requests.")
        }
        // runall
        Commands::RunAll {} => {
            for service in &services.services {
                let mut process = Service::run(service.0.to_string(), services.clone())?;

                // if this is an application, immediately exit
                if process.0.r#type == ServiceType::Application {
                    process.1.kill()?;
                    continue;
                }

                // ...
                services.service_states.insert(
                    service.0.to_string(),
                    (ServiceState::Running, process.1.id()),
                );
            }

            ServicesConfiguration::update_config(services)?;
            Ok("Started all services.")
        }
        // kill
        Commands::Kill { names } => {
            if names.len() == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Please provide at least 1 service name.",
                ));
            }

            for name in names {
                match services.services.get(name) {
                    Some(_) => {
                        Service::kill(name.to_string(), services.clone())?;
                        services.service_states.remove(name);
                    }
                    None => return Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
                }
            }

            // return
            ServicesConfiguration::update_config(services.clone())?;
            Ok("Stopped all given services.")
        }
        // kill-all
        Commands::KillAll {} => {
            for service in &services.services {
                if let Err(e) = Service::kill(service.0.to_string(), services.clone()) {
                    println!("warn: {}", e.to_string());
                }

                // if we couldn't get the pid then the service probably ran and exited already
                services.service_states.remove(service.0);
            }

            // return
            ServicesConfiguration::update_config(services)?;
            Ok("Stopped all services.")
        }
        // info
        Commands::Info { name } => match services.service_states.get(name) {
            Some(_) => {
                println!(
                    "{}",
                    Service::info(name.to_string(), services.service_states.clone())?
                );

                Ok("Finished.")
            }
            None => Err(Error::new(ErrorKind::NotFound, "Service is not loaded.")),
        },
        // info-all
        Commands::InfoAll {} => {
            for service in &services.service_states {
                if let Ok(i) = Service::info(service.0.to_string(), services.service_states.clone())
                {
                    println!("{i}");
                }
            }

            // return
            Ok("Finished.")
        }
        // track
        Commands::Track { name } => match services.services.get(name) {
            Some(_) => {
                Service::observe(name.to_string(), services.service_states.clone()).await?;

                services.service_states.remove(name);
                ServicesConfiguration::update_config(services)?;

                // return
                Ok("Service stopped.")
            }
            None => Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
        },
        // serve
        Commands::Serve {} => {
            server::server(services).await;
            Ok("Finished.")
        }
        // pinned
        Commands::Pinned {} => {
            println!("{}", toml::to_string_pretty(&services).unwrap());
            Ok("Finished.")
        }
        // merge
        Commands::Merge { path } => {
            // read file
            let other_config = ServicesConfiguration::read(std::fs::read_to_string(path)?);

            // merge and write
            services.merge_config(other_config);
            std::fs::write(
                services.source.clone(),
                toml::to_string_pretty(&services).unwrap(),
            )?;

            // return
            Ok("Merged configuration. (source + other)")
        }
        // pull
        Commands::Pull { path } => {
            // read file
            let other_config = ServicesConfiguration::read(std::fs::read_to_string(path)?);

            // merge and write
            services.merge_config(other_config);
            ServicesConfiguration::update_config(services)?;

            // return
            Ok("Pulled configuration. (pinned + other)")
        }
        // install
        Commands::Install { registry, names } => {
            if names.len() == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Please provide at least 1 service name.",
                ));
            }

            // post requests
            let client = reqwest::Client::new();

            for name in names {
                match client
                    .get(format!("http://{}/registry/{}", registry, name))
                    .send()
                    .await
                {
                    Ok(r) => {
                        let res: APIReturn<String> = r.json().await.expect("Failed to read body");

                        if res.ok == false {
                            return Err(Error::new(
                                ErrorKind::Other,
                                format!("remote: {}", res.data),
                            ));
                        }

                        // add service
                        let home = std::env::var("HOME").expect("failed to read $HOME");
                        let mut service: Service = match toml::from_str(&res.data) {
                            Ok(s) => s,
                            Err(e) => {
                                return Err(Error::new(ErrorKind::InvalidData, e.to_string()))
                            }
                        };

                        // run build
                        service.bootstrap(name.to_owned()).await?;

                        // make relative home exact
                        service.working_directory = service.working_directory.replace("~", &home);

                        // make build dir exact
                        service.working_directory = service.working_directory.replace(
                            "@",
                            &format!("{home}/.config/xsu-apps/sproc/modules/{name}"),
                        );

                        // push service
                        services.services.insert(name.to_owned(), service);

                        // log
                        println!("info: installed service to pinned file: {}", name);
                    }
                    Err(e) => return Err(Error::new(ErrorKind::NotConnected, e.to_string())),
                }
            }

            ServicesConfiguration::update_config(services.clone())?;
            Ok("Sent all requested requests.")
        }
        // uninstall
        Commands::Uninstall { names } => {
            if names.len() == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Please provide at least 1 service name.",
                ));
            }

            for name in names {
                if services.service_states.contains_key(name) {
                    // kill service if it's running
                    Service::kill(name.to_owned(), services.clone())?;
                }

                // remove directory
                let home = std::env::var("HOME").expect("failed to read $HOME");
                if let Ok(_) = fs::read_dir(format!("{home}/.config/xsu-apps/sproc/modules/{name}"))
                {
                    fs::remove_dir_all(format!("{home}/.config/xsu-apps/sproc/modules/{name}"))?
                }

                // remove service
                services.services.remove(name);
            }

            ServicesConfiguration::update_config(services.clone())?;
            Ok("Finished.")
        }
    }
}

// fake main
#[tokio::main]
async fn main() {
    match sproc().await {
        Ok(s) => yes(s),
        Err(e) => no(&e.to_string()),
    }
}
