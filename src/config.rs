use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    io::{Error, ErrorKind, Result},
    process::{Child, Command},
};
use sysinfo::{Pid, System};

pub type ServiceStates = HashMap<String, (ServiceState, u32)>;

#[derive(Clone, Serialize, Deserialize)]
/// `services.toml` file
pub struct ServicesConfiguration {
    /// Service definitions
    pub services: HashMap<String, Service>,
    /// Service states
    #[serde(default)]
    pub service_states: ServiceStates,
}

impl Default for ServicesConfiguration {
    fn default() -> Self {
        Self {
            services: HashMap::new(),
            service_states: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub enum ServiceState {
    Running,
    Stopped,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Clone, Serialize, Deserialize)]
/// A single service
pub struct Service {
    /// What command is run to start the service
    pub command: String,
    /// Where the `command` is run
    pub working_directory: String,
    /// Environment variables map
    pub environment: Option<HashMap<String, String>>,
}

impl Service {
    /// Spawn service process
    pub fn run(name: String, cnf: ServicesConfiguration) -> Result<Child> {
        // check current state
        if let Some(s) = cnf.service_states.get(&name) {
            // make sure service isn't already running
            if s.0 == ServiceState::Running {
                return Err(Error::new(
                    ErrorKind::AlreadyExists,
                    "Service is already running.",
                ));
            }
        };

        let service = match cnf.services.get(&name) {
            Some(s) => s,
            None => return Err(Error::new(ErrorKind::NotFound, "Service does not exist.")),
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
    pub fn kill(name: String, service_states: ServiceStates) -> Result<()> {
        let s = match service_states.get(&name) {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "Service has never been run.",
                ))
            }
        };

        if s.0 != ServiceState::Running {
            return Err(Error::new(
                ErrorKind::NotConnected,
                "Service is not running.",
            ));
        }

        // stop service
        let sys = System::new_all();

        match sys.process(Pid::from(s.1 as usize)) {
            Some(process) => {
                process.kill();
                Ok(())
            }
            None => Err(Error::new(
                ErrorKind::NotConnected,
                "Failed to get process from PID.",
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
                    "Service has never been run.",
                ))
            }
        };

        if s.0 != ServiceState::Running {
            return Err(Error::new(
                ErrorKind::NotConnected,
                "Service is not running.",
            ));
        }

        // stop service
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
                "Failed to get process from PID.",
            ))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub pid: u32,
    pub memory: u64,
    pub cpu: f32,
    pub status: String,
    pub running_for_seconds: u64,
}

impl ServicesConfiguration {
    pub fn get_config() -> ServicesConfiguration {
        let home = env::var("HOME").expect("failed to read $HOME");

        if let Err(_) = fs::read_dir(format!("{home}/.config/sproc")) {
            if let Err(e) = fs::create_dir(format!("{home}/.config/sproc")) {
                panic!("{:?}", e)
            };
        }

        match fs::read_to_string(format!("{home}/.config/sproc/services.toml")) {
            Ok(c) => toml::from_str::<Self>(&c).unwrap(),
            Err(_) => Self::default(),
        }
    }

    pub fn update_config(contents: Self) -> std::io::Result<()> {
        let home = env::var("HOME").expect("failed to read $HOME");

        fs::write(
            format!("{home}/.config/sproc/services.toml"),
            format!("# DO **NOT** MANUALLY EDIT THIS FILE! Please edit the source instead and run `sproc pin {{path}}`.\n{}", toml::to_string_pretty::<Self>(&contents).unwrap()),
        )
    }
}
