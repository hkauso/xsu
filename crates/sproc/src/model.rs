//! Sproc process management (service handling)
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Error, ErrorKind, Result},
    process::{Child, Command, Stdio},
};
use sysinfo::{Pid, System};
use xsu_util::fs;

pub type ServiceStates = HashMap<String, (ServiceState, u32)>;

/// [`Service`] metadata/extra information that isn't needed to run the service
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ServiceMetadata {
    /// Service owner
    #[serde(default)]
    pub owner: String,
    /// Source repository URL
    #[serde(default)]
    pub repository: String,
    /// Description
    #[serde(default)]
    pub description: String,
    /// Source license
    #[serde(default)]
    pub license: String,
    /// Service build steps run in `~/.config/xsu-apps/sproc/modules/:name`
    #[serde(default)]
    pub build: Vec<String>,
}

impl Default for ServiceMetadata {
    fn default() -> Self {
        Self {
            owner: String::new(),
            repository: String::new(),
            description: "Unknown service".to_string(),
            license: "ISC".to_string(),
            build: Vec::new(),
        }
    }
}

/// [`Service`] type
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ServiceType {
    /// A service that is run in the background and tracks the PID
    Service,
    /// A service that does not run in the background and does not track PID
    Application,
}

impl Default for ServiceType {
    fn default() -> Self {
        Self::Service
    }
}

/// A single executable service
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Service {
    /// What the type of the service is
    #[serde(default)]
    pub r#type: ServiceType,
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
    pub fn run(name: String, config: ServicesConfiguration) -> Result<(Service, Child)> {
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
        println!("info: cmd: {}", service.command);
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

        cmd.current_dir(&service.working_directory);

        // spawn
        Ok((service.to_owned(), cmd.spawn()?))
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
            .insert(name.to_string(), (ServiceState::Running, process.1.id()));

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

    // package manager

    /// Run and init a [`Service`]'s [`BuildConfiguration`]
    pub async fn bootstrap(&self, name: String) -> Result<()> {
        let home = env::var("HOME").expect("failed to read $HOME");

        // verify modules directory
        if let Err(_) = fs::read_dir(format!("{home}/.config/xsu-apps/sproc/modules")) {
            if let Err(e) = fs::create_dir(format!("{home}/.config/xsu-apps/sproc/modules")) {
                panic!("{:?}", e);
            }
        }

        // check for existing directory
        let dir = format!("{home}/.config/xsu-apps/sproc/modules/{}", name);

        if let Ok(_) = fs::read_dir(&dir) {
            return Err(Error::new(ErrorKind::AlreadyExists, "The requested service has already run its build commands or its build directory already exists."));
        }

        // create directory
        fs::create_dir(&dir)?;

        // create build file
        // TODO: make this work on other platforms
        let build_file = format!("{dir}/build.artifact.sh");
        fs::write(&build_file, self.metadata.build.join("\n"))?;

        // run build file
        let command = format!("bash {build_file}");
        let command_split: Vec<&str> = command.split(" ").collect();
        let mut cmd = Command::new(command_split.get(0).unwrap());

        for arg in command_split.iter().skip(1) {
            cmd.arg(arg);
        }

        cmd.current_dir(&dir);

        // capture out
        let child_stdout = cmd
            .stdout(Stdio::piped())
            .spawn()?
            .stdout
            .expect("failed to capture command output");

        let reader = BufReader::new(child_stdout);

        reader
            .lines()
            .filter_map(|l| l.ok())
            .for_each(|l| println!("build: {l}"));

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
    /// If the registry is enabled
    pub enabled: bool,
    /// Registry description, shown on the homepage
    #[serde(default)]
    pub description: String,
    /// Registry name, shown on homepage
    #[serde(default = "registry_default")]
    pub name: String,
    /// Navigation bar buttons, `(Text, Url)`
    #[serde(default)]
    pub nav: Vec<(String, String)>,
}

fn registry_default() -> String {
    "Registry".to_owned()
}

impl Default for RegistryConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            description: String::new(),
            name: registry_default(),
            nav: Vec::new(),
        }
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

        if let Err(_) = fs::read_dir(format!("{home}/.config/xsu-apps/sproc")) {
            // make sure .config exists
            fs::mkdir(format!("{home}/.config")).expect("failed to create .config directory");

            // make sure .config/xsu-apps exists
            fs::mkdir(format!("{home}/.config/xsu-apps"))
                .expect("failed to create xsu-apps directory");

            // create .config/xsu-apps/sproc
            fs::mkdir(format!("{home}/.config/xsu-apps/sproc"))
                .expect("failed to create sproc directory")
        }

        match fs::read(format!("{home}/.config/xsu-apps/sproc/services.toml")) {
            Ok(c) => ServicesConfiguration::read(c),
            Err(_) => Self::default(),
        }
    }

    /// Update configuration file
    pub fn update_config(contents: Self) -> Result<()> {
        let home = env::var("HOME").expect("failed to read $HOME");

        fs::write(
            format!("{home}/.config/xsu-apps/sproc/services.toml"),
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
    /// The service's content in TOML form
    pub content: String,
}

/// Request body for deleting a service
#[derive(Serialize, Deserialize)]
pub struct RegistryDeleteRequestBody {
    /// Auth key
    pub key: String,
}

/// A simple registry for service files
#[derive(Debug, Clone)]
pub struct Registry(pub ServerConfiguration, pub String, pub String, pub String);

impl Registry {
    /// Create a new [`Registry`]
    pub fn new(config: ServerConfiguration) -> Self {
        let home = env::var("HOME").expect("failed to read $HOME");

        let dir = format!("{home}/.config/xsu-apps/sproc/registry"); // registry file storage location
        let book_dir = format!("{home}/.config/xsu-apps/sproc/book"); // simple markdown directory
        let html_dir = format!("{home}/.config/xsu-apps/sproc/html"); // custom html files to serve

        // create registry dir
        fs::mkdir(&dir).expect("failed to create directory");
        fs::mkdir(&book_dir).expect("failed to create book directory");
        fs::mkdir(&html_dir).expect("failed to create html directory");

        // return
        Self(config, dir, book_dir, html_dir)
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
        fs::read(format!("{}/{}.toml", self.1, service))
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

        // validate
        if let Err(e) = toml::from_str::<Service>(&props.content) {
            return Err(Error::new(ErrorKind::InvalidInput, e.to_string()));
        };

        // return
        fs::write(format!("{}/{}.toml", self.1, service), &props.content)
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
        fs::rm(format!("{}/{}.toml", self.1, service))
    }

    // book

    /// Get a page's metadata given its path (path should include extension)
    pub fn get_page_metadata(&self, path: &String) -> Result<fs::Metadata> {
        if self.0.registry.enabled == false {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Registry is disabled",
            ));
        }

        // return
        fs::fstat(format!("{}/{path}", self.2))
    }

    /// Get a page given its path (path should include extension)
    pub fn get_page(&self, path: String) -> Result<String> {
        if self.0.registry.enabled == false {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Registry is disabled",
            ));
        }

        // return
        fs::read(format!("{}/{path}", self.2))
    }

    /// Update (or create) a page in the book given its path and value (path should include extension)
    pub fn push_page(&self, props: RegistryPushRequestBody, path: String) -> Result<()> {
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

        // write directories
        let mut dir_path: String = String::new();
        for dir in path.split("/") {
            if dir.contains(".") {
                // file extension
                break;
            }

            fs::mkdir(format!("{}/{dir_path}{dir}", self.2))?;
            dir_path.push_str(&format!("{dir}/")) // this will make sure the next directory we create will include the previous too
        }

        // return
        fs::write(format!("{}/{path}", self.2), &props.content)
    }

    /// Delete a page from the book given its path (path should include extension)
    pub fn delete_page(&self, props: RegistryDeleteRequestBody, path: String) -> Result<()> {
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

        // delete
        fs::rm(format!("{}/{path}", self.2))?;

        // remove empty directories
        let path_split = path.split("/").collect::<Vec<&str>>();
        for (i, dir) in path_split.iter().rev().enumerate() {
            // we need to go through this in reverse
            if dir.contains(".") {
                // file extension, already removed, just continue
                continue;
            }

            // this is an awful way to do this
            let all_previous = path_split
                .iter()
                .take(path_split.len() - i)
                .collect::<Vec<&&str>>();

            let mut joined = String::new();

            for prev in all_previous {
                // couldn't just let me use .join()?
                joined.push_str(&format!("{}/", prev.replace("/", "")));
            }

            // check if it's empty
            if joined == "/" {
                // don't delete root!
                break;
            }

            let path = format!("{}/{joined}", self.2);

            if let Ok(r) = fs::read_dir(&path) {
                if r.into_iter().count() == 0 {
                    fs::rmdirr(path)?;
                } else {
                    // once we reached the first non-empty directory we'll need to stop
                    // we're iterating in reverse so the first non-empty directory means all the
                    // directories after is are ALSO not empty
                    break;
                }
            }
        }

        // return
        Ok(())
    }

    // html

    /// Get an html page given its name
    pub fn get_html(&self, path: String) -> Result<String> {
        if self.0.registry.enabled == false {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Registry is disabled",
            ));
        }

        // return
        fs::read(format!("{}/{path}.html", self.3))
    }
}
