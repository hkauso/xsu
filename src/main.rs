//! Sproc process manager

use clap::{Parser, Subcommand};
use std::{
    fs,
    io::{Error, ErrorKind, Result},
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
    Run { name: String },
    /// Run all services
    RunAll {},
    /// Kill a running service
    Kill { name: String },
    /// Kill all services
    KillAll {},
    /// Get information about a running service
    Info { name: String },
    /// Get information about all services
    InfoAll {},
    /// Wait for service to stop and update its state accordingly
    Track { name: String },
}

// ...
pub mod model;
use model::{Service, ServiceState, ServicesConfiguration};

// real main
async fn sproc<'a>() -> Result<&'a str> {
    // init
    let args = Sproc::parse();

    // get current config
    let mut services = ServicesConfiguration::get_config();

    // ...
    match &args.command {
        // pin
        Commands::Pin { path } => match fs::read_to_string(path) {
            Ok(s) => {
                ServicesConfiguration::update_config(toml::from_str(&s).unwrap())?;
                Ok("Services loaded.")
            }
            Err(e) => Err(e),
        },
        // run
        Commands::Run { name } => match services.services.get(name) {
            Some(_) => {
                let process = Service::run(name.to_string(), services.clone())?;

                services
                    .service_states
                    .insert(name.to_string(), (ServiceState::Running, process.id()));
                ServicesConfiguration::update_config(services)?;

                // return
                Ok("Service started.")
            }
            None => Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
        },
        // runall
        Commands::RunAll {} => {
            for service in &services.services {
                let process = Service::run(service.0.to_string(), services.clone())?;

                services
                    .service_states
                    .insert(service.0.to_string(), (ServiceState::Running, process.id()));
            }

            ServicesConfiguration::update_config(services)?;
            Ok("Started all services.")
        }
        // kill
        Commands::Kill { name } => match services.services.get(name) {
            Some(_) => {
                Service::kill(name.to_string(), services.service_states.clone())?;

                services.service_states.remove(name);
                ServicesConfiguration::update_config(services)?;

                // return
                Ok("Service stopped.")
            }
            None => Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
        },
        // kill-all
        Commands::KillAll {} => {
            for service in &services.services {
                if let Err(e) =
                    Service::kill(service.0.to_string(), services.service_states.clone())
                {
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
            None => Err(Error::new(
                ErrorKind::NotFound,
                "Service has never been run.",
            )),
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
                // TODO: use this to create a server that can observe services and wait for them to stop (in a new thread)
                // TODO: add "restart" to service definition, allowing observed services to automatically be restarted when stopped
                Service::observe(name.to_string(), services.service_states.clone()).await?;

                services.service_states.remove(name);
                ServicesConfiguration::update_config(services)?;

                // return
                Ok("Service stopped.")
            }
            None => Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
        },
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

fn no(msg: &str) -> () {
    println!("\x1b[91m{}\x1b[0m", format!("error:\x1b[0m {msg}"));
    std::process::exit(1);
}

fn yes(msg: &str) -> () {
    println!("\x1b[92m{}\x1b[0m", format!("success:\x1b[0m {msg}"));
    std::process::exit(0);
}
