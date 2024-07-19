//! Open files and URIs in their correct application
use clap::{Parser, Subcommand};
use std::{
    process::Command,
    io::{Result, stdout, Write},
};
use xsu_util::process::{no, yes};
use xsu_appman::config;

// ...
#[derive(Parser, Debug)]
#[command(version, about, long_about = Option::Some("Open files and URIs in their correct application."))]
#[command(propagate_version = true)]
struct App {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Open a file
    File { path: String },
}

// real main
async fn open<'a>() -> Result<&'a str> {
    // init
    let args = App::parse();

    // get current config
    let cnf = config::Config::get_config();

    // ...
    match &args.command {
        Commands::File { path } => {
            let guess = mime_guess::from_path(&path);
            let mime = guess.first().unwrap().to_string();
            match cnf.mimes.get(&mime) {
                Some(application) => {
                    // run command
                    let mut child = Command::new(application)
                        .arg(path)
                        .spawn()
                        .expect("failed to spawn process");

                    // return
                    child.wait().expect("failed to wait for process");
                    Ok("Finished.")
                }
                None => Ok("No default application set."),
            }
        } // TODO: uri
    }
}

// fake main
#[tokio::main]
async fn main() {
    match open().await {
        Ok(s) => yes(s),
        Err(e) => no(&e.to_string()),
    }
}
