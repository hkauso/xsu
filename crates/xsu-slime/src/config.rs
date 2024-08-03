//! Application config manager
use serde::{Deserialize, Serialize};
use std::{env, io::Result};
use xsu_util::fs;

/// Configuration file
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    /// Mappings for general HTML tags and what ANSI sequences they convert to
    pub map: Vec<(String, String)>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            map: vec![
                ("<b>".to_string(), "\x1b[1m".to_string()),
                ("<i>".to_string(), "\x1b[3m".to_string()),
                ("<strong>".to_string(), "\x1b[1m".to_string()),
                ("<em>".to_string(), "\x1b[3m".to_string()),
                ("<h1>".to_string(), "\x1b[1;4m".to_string()),
                ("<h2>".to_string(), "\x1b[1m".to_string()),
                ("<h3>".to_string(), "\x1b[1m".to_string()),
                ("<h4>".to_string(), "\x1b[1m".to_string()),
                ("<h5>".to_string(), "\x1b[1m".to_string()),
                ("<h6>".to_string(), "\x1b[1m".to_string()),
                ("<li>".to_string(), "\u{2022} ".to_string()),
                ("<hr>".to_string(), "-".repeat(15).to_string()),
                ("<br>".to_string(), "\n".to_string()),
                ("<code>".to_string(), "\x1b[47;30m".to_string()),
                ("<pre>".to_string(), "\x1b[47;30m".to_string()),
                ("<p>".to_string(), "".to_string()),
                ("<ul>".to_string(), "".to_string()),
                ("<ol>".to_string(), "".to_string()),
                ("<blockquote>".to_string(), "".to_string()),
                // closing/reset
                ("</b>".to_string(), "\x1b[0m".to_string()),
                ("</i>".to_string(), "\x1b[0m".to_string()),
                ("</strong>".to_string(), "\x1b[0m".to_string()),
                ("</em>".to_string(), "\x1b[0m".to_string()),
                ("</h1>".to_string(), "\x1b[0m\n".to_string()),
                ("</h2>".to_string(), "\x1b[0m\n".to_string()),
                ("</h3>".to_string(), "\x1b[0m\n".to_string()),
                ("</h4>".to_string(), "\x1b[0m\n".to_string()),
                ("</h5>".to_string(), "\x1b[0m\n".to_string()),
                ("</h6>".to_string(), "\x1b[0m\n".to_string()),
                ("</li>".to_string(), "".to_string()),
                ("</code>".to_string(), "\x1b[0m".to_string()),
                ("</pre>".to_string(), "\x1b[0m".to_string()),
                ("</p>".to_string(), "\n".to_string()),
                ("</ul>".to_string(), "".to_string()),
                ("</ol>".to_string(), "".to_string()),
                ("</blockquote>".to_string(), "".to_string()),
            ],
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

        if let Err(_) = fs::read_dir(format!("{home}/.config/xsu-apps/slime")) {
            // make sure .config exists
            fs::mkdir(format!("{home}/.config")).expect("failed to create .config directory");

            // make sure .config/xsu-apps exists
            fs::mkdir(format!("{home}/.config/xsu-apps"))
                .expect("failed to create xsu-apps directory");

            // create .config/xsu-apps/slime
            fs::mkdir(format!("{home}/.config/xsu-apps/slime"))
                .expect("failed to create appman directory")
        }

        match fs::read(format!("{home}/.config/xsu-apps/slime/config.toml")) {
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
            format!("{home}/.config/xsu-apps/slime/config.toml"),
            toml::to_string_pretty::<Self>(&contents).unwrap(),
        )
    }
}
