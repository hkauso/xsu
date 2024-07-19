//! Application config manager
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, io::Result};
use xsu_util::fs;

/// Configuration file
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    pub mimes: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mimes: HashMap::new(),
        }
    }
}

impl Config {
    /// Read configuration file into [`Config`]
    pub fn read(contents: String) -> Self {
        toml::from_str::<Self>(&contents).unwrap()
    }

    /// Pull configuration file
    pub fn get_config() -> Self {
        let home = env::var("HOME").expect("failed to read $HOME");

        if let Err(_) = fs::read_dir(format!("{home}/.config/xsu-apps/appman")) {
            // make sure .config exists
            fs::mkdir(format!("{home}/.config")).expect("failed to create .config directory");

            // make sure .config/xsu-apps exists
            fs::mkdir(format!("{home}/.config/xsu-apps"))
                .expect("failed to create xsu-apps directory");

            // create .config/xsu-apps/appman
            fs::mkdir(format!("{home}/.config/xsu-apps/appman"))
                .expect("failed to create appman directory")
        }

        match fs::read(format!("{home}/.config/xsu-apps/appman/config.toml")) {
            Ok(c) => Config::read(c),
            Err(_) => {
                Self::update_config(Self::default()).expect("failed to write default config");
                Self::default()
            }
        }
    }

    /// Update configuration file
    pub fn update_config(contents: Self) -> Result<()> {
        let home = env::var("HOME").expect("failed to read $HOME");

        fs::write(
            format!("{home}/.config/xsu-apps/appman/config.toml"),
            toml::to_string_pretty::<Self>(&contents).unwrap(),
        )
    }
}
