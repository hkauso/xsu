//! Sproc process management (service handling)
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    io::{Error, ErrorKind, Result},
    process::{Child, Command},
};
use sysinfo::{Pid, System};

pub type ServiceStates = HashMap<String, (ServiceState, u32)>;

/// [`Service`] metadata/extra information that isn't needed to run the service
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServiceMetadata {
    /// Source repository URL
    #[serde(default)]
    pub repository: String,
    /// Description
    #[serde(default)]
    pub description: String,
    /// Source license
    #[serde(default)]
    pub license: String,
}

impl Default for ServiceMetadata {
    fn default() -> Self {
        Self {
            repository: String::new(),
            description: "Unknown service".to_string(),
            license: "ISC".to_string(),
        }
    }
}

/// A single executable service
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Service {
    /// What command is run to start the service
    pub command: String,
    /// Where the `command` is run
    pub working_directory: String,
    /// Environment variables map
    pub environment: Option<HashMap<String, String>>,
    /// If the service should restart automatically when exited (HTTP server required)
    #[serde(default)]
    pub restart: bool,
    /// Metadata
    #[serde(default)]
    pub metadata: ServiceMetadata,
}

impl Service {
    /// Spawn service process
    pub fn run(name: String, config: ServicesConfiguration) -> Result<Child> {
        // check current state
        if let Some(s) = config.service_states.get(&name) {
            // make sure service isn't already running
            if s.0 == ServiceState::Running {
                return Err(Error::new(
                    ErrorKind::AlreadyExists,
                    format!("Service is already running. ({name})"),
                ));
            }
        };

        let service = match config.services.get(&name) {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Service does not exist. ({name})"),
                ))
            }
        };

        // create command
        let command_split: Vec<&str> = service.command.split(" ").collect();
        let mut cmd = Command::new(command_split.get(0).unwrap());

        for arg in command_split.iter().skip(1) {
            cmd.arg(arg);
        }

        if let Some(env) = service.environment.clone() {
            for var in env {
                cmd.env(var.0, var.1);
            }
        }

        cmd.current_dir(service.working_directory.clone());

        // spawn
        Ok(cmd.spawn()?)
    }

    /// Kill service process
    pub fn kill(name: String, config: ServicesConfiguration) -> Result<()> {
        let s = match config.service_states.get(&name) {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Service is not loaded. ({name})"),
                ))
            }
        };

        if s.0 != ServiceState::Running {
            return Err(Error::new(
                ErrorKind::NotConnected,
                "Service is not running.",
            ));
        }

        let mut config_c = config.clone();
        let service = match config_c.services.get_mut(&name) {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Service does not exist. ({name})"),
                ))
            }
        };

        // stop service
        let sys = System::new_all();

        match sys.process(Pid::from(s.1 as usize)) {
            Some(process) => {
                let supposed_to_restart = service.restart.clone();

                // if service is supposed to restart, toggle off and update config
                if supposed_to_restart {
                    // we must do this so threads that will restart this service don't
                    service.restart = false;
                    ServicesConfiguration::update_config(config_c.clone())?;
                }

                // kill process
                process.kill();
                std::thread::sleep(std::time::Duration::from_secs(1)); // wait for 1s so the server can catch up

                // if service was previously supposed to restart, re-enable restart
                if supposed_to_restart {
                    // set config back to original form
                    ServicesConfiguration::update_config(config.clone())?;
                }

                // return
                Ok(())
            }
            None => Err(Error::new(
                ErrorKind::NotConnected,
                format!("Failed to get process from PID. ({name})"),
            )),
        }
    }

    /// Get service process info
    pub fn info(name: String, service_states: ServiceStates) -> Result<String> {
        let s = match service_states.get(&name) {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Service is not loaded. ({name})"),
                ))
            }
        };

        if s.0 != ServiceState::Running {
            return Err(Error::new(
                ErrorKind::NotConnected,
                format!("Service is not running. ({name})"),
            ));
        }

        // get service info
        let sys = System::new_all();

        if let Some(process) = sys.process(Pid::from(s.1 as usize)) {
            let info = ServiceInfo {
                name: name.to_string(),
                pid: process.pid().to_string().parse().unwrap(),
                memory: process.memory(),
                cpu: process.cpu_usage(),
                status: process.status().to_string(),
                running_for_seconds: process.run_time(),
            };

            Ok(toml::to_string_pretty(&info).unwrap())
        } else {
            Err(Error::new(
                ErrorKind::NotConnected,
                format!("Failed to get process from PID. ({name})"),
            ))
        }
    }

    // exit handling

    /// Wait for a service process to stop and update its state when it does
    pub async fn observe(name: String, service_states: ServiceStates) -> Result<()> {
        let s = match service_states.get(&name) {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Service is not loaded. ({name})"),
                ))
            }
        };

        if s.0 != ServiceState::Running {
            return Err(Error::new(
                ErrorKind::NotConnected,
                format!("Service is not running. ({name})"),
            ));
        }

        // get service
        let sys = System::new_all();

        if let Some(process) = sys.process(Pid::from(s.1 as usize)) {
            // wait for process to stop
            process.wait();
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::NotConnected,
                format!("Failed to get process from PID. ({name})"),
            ))
        }
    }

    /// Start and observe a service
    async fn wait(name: String, config: &mut ServicesConfiguration) -> Result<()> {
        // start service
        let process = match Service::run(name.clone(), config.clone()) {
            Ok(p) => p,
            Err(e) => return Err(e),
        };

        // update config
        config
            .service_states
            .insert(name.to_string(), (ServiceState::Running, process.id()));

        ServicesConfiguration::update_config(config.clone()).expect("Failed to update config");
        Service::observe(name.clone(), config.service_states.clone())
            .await
            .expect("Failed to observe service");

        Ok(())
    }

    /// [`Service::wait`] in a new task
    pub async fn spawn(name: String) -> Result<()> {
        // spawn task
        tokio::task::spawn(async move {
            loop {
                // pull config from file
                let mut config = ServicesConfiguration::get_config();

                // start service
                Service::wait(name.clone(), &mut config)
                    .await
                    .expect("Failed to wait for service");

                // pull real config
                // we have to do this so we don't restart if it was disabled while the service was running
                let mut config = ServicesConfiguration::get_config();
                let service = match config.services.get(&name) {
                    Some(s) => s,
                    None => return,
                };

                // update config
                config.service_states.remove(&name);
                ServicesConfiguration::update_config(config.clone())
                    .expect("Failed to update config");

                // ...
                if service.restart == false {
                    // no need to loop again if we aren't supposed to restart the service
                    break;
                }

                // begin restart
                println!("info: auto-restarting service \"{}\"", name);
                continue; // service will be run again
            }
        });

        // return
        Ok(())
    }
}

/// The state of a [`Service`]
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub enum ServiceState {
    Running,
    Stopped,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self::Stopped
    }
}

/// General information about a [`ServiceState`]
#[derive(Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub pid: u32,
    pub memory: u64,
    pub cpu: f32,
    pub status: String,
    pub running_for_seconds: u64,
}

/// Configuration for `sproc serve`'s registry
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RegistryConfiguration {
    pub enabled: bool,
}

impl Default for RegistryConfiguration {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Configuration for `sproc serve`
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServerConfiguration {
    /// The port to serve the HTTP server on (6374 by default)
    pub port: u16,
    /// The key that is required to run operations from the HTTP server
    pub key: String,
    /// Configuration for the registry
    #[serde(default)]
    pub registry: RegistryConfiguration,
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        Self {
            port: 6374,
            key: String::new(),
            registry: RegistryConfiguration::default(),
        }
    }
}

/// `services.toml` file
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServicesConfiguration {
    /// The source file location
    #[serde(default)]
    pub source: String,
    /// Inherited service definition files
    pub inherit: Option<Vec<String>>,
    /// Server configuration (`sproc serve`)
    #[serde(default)]
    pub server: ServerConfiguration,
    /// Service definitions
    pub services: HashMap<String, Service>,
    /// Service states
    #[serde(default)]
    pub service_states: ServiceStates,
}

impl Default for ServicesConfiguration {
    fn default() -> Self {
        Self {
            source: String::new(),
            inherit: None,
            services: HashMap::new(),
            server: ServerConfiguration::default(),
            service_states: HashMap::new(),
        }
    }
}

impl ServicesConfiguration {
    /// Read configuration file into [`ServicesConfiguration`]
    pub fn read(contents: String) -> Self {
        let mut res = toml::from_str::<Self>(&contents).unwrap();

        // handle inherits
        if let Some(ref inherit) = res.inherit {
            for path in inherit {
                if let Ok(c) = fs::read_to_string(path) {
                    for service in toml::from_str::<Self>(&c).unwrap().services {
                        // push service to main service stack
                        res.services.insert(service.0, service.1);
                    }
                }
            }
        }

        // return
        res
    }

    /// Pull configuration file
    pub fn get_config() -> Self {
        let home = env::var("HOME").expect("failed to read $HOME");

        if let Err(_) = fs::read_dir(format!("{home}/.config/sproc")) {
            // make sure .config exists
            if let Err(_) = fs::read_dir(format!("{home}/.config")) {
                if let Err(e) = fs::create_dir(format!("{home}/.config")) {
                    panic!("{:?}", e);
                }
            }

            // create .config/sproc
            if let Err(e) = fs::create_dir(format!("{home}/.config/sproc")) {
                panic!("{:?}", e)
            };
        }

        let path = format!("{home}/.config/sproc/services.toml");
        match fs::read_to_string(path.clone()) {
            Ok(c) => ServicesConfiguration::read(c),
            Err(_) => Self::default(),
        }
    }

    /// Update configuration file
    pub fn update_config(contents: Self) -> Result<()> {
        let home = env::var("HOME").expect("failed to read $HOME");

        fs::write(
            format!("{home}/.config/sproc/services.toml"),
            format!("# DO **NOT** MANUALLY EDIT THIS FILE! Please edit the source instead and run `sproc pin {{path}}`.\n{}", toml::to_string_pretty::<Self>(&contents).unwrap()),
        )
    }

    /// Merge services from other [`ServicesConfiguration`]
    pub fn merge_config(&mut self, other: Self) -> () {
        for service in other.services {
            // push service to main service stack
            self.services.insert(service.0, service.1);
        }
    }
}

/// Request body for updating a service
#[derive(Serialize, Deserialize)]
pub struct RegistryPushRequestBody {
    /// Auth key
    pub key: String,
    /// The service's content
    pub content: Service,
}

/// Request body for deleting a service
#[derive(Serialize, Deserialize)]
pub struct RegistryDeleteRequestBody {
    /// Auth key
    pub key: String,
}

/// A simple registry for service files
#[derive(Debug, Clone)]
pub struct Registry(ServerConfiguration, pub String);

impl Registry {
    /// Create a new [`Registry`]
    pub fn new(config: ServerConfiguration) -> Self {
        let home = env::var("HOME").expect("failed to read $HOME");
        let dir = format!("{home}/.config/sproc/registry"); // registry file storage location

        // create registry dir
        if let Err(_) = fs::read_dir(&dir) {
            if let Err(e) = fs::create_dir(&dir) {
                panic!("{:?}", e);
            }
        }

        // return
        Self(config, dir)
    }

    /// Get a service given its name
    pub fn get(&self, service: String) -> Result<String> {
        if self.0.registry.enabled == false {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Registry is disabled",
            ));
        }

        // return
        fs::read_to_string(format!("{}/{}.toml", self.1, service))
    }

    /// Update (or create) a service given its name and value
    pub fn push(&self, props: RegistryPushRequestBody, service: String) -> Result<()> {
        if self.0.registry.enabled == false {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Registry is disabled",
            ));
        }

        // check key
        if props.key != self.0.key {
            return Err(Error::new(ErrorKind::PermissionDenied, "Key is invalid"));
        }

        // return
        fs::write(
            format!("{}/{}.toml", self.1, service),
            match toml::to_string_pretty(&props.content) {
                Ok(s) => s,
                Err(_) => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Could not serialize service content",
                    ))
                }
            },
        )
    }

    /// Delete a service given its name
    pub fn delete(&self, props: RegistryDeleteRequestBody, service: String) -> Result<()> {
        if self.0.registry.enabled == false {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Registry is disabled",
            ));
        }

        // check key
        if props.key != self.0.key {
            return Err(Error::new(ErrorKind::PermissionDenied, "Key is invalid"));
        }

        // return
        fs::remove_file(format!("{}/{}.toml", self.1, service))
    }
}
