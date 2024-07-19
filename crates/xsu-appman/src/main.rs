//! Simple default application manager
use clap::{Parser, Subcommand};
use std::io::Result;
use xsu_util::process::{no, yes};

mod config;

// ...
#[derive(Parser, Debug)]
#[command(version, about, long_about = Option::Some("Appman application manager"))]
#[command(propagate_version = true)]
struct App {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Get the default application for a file
    File { path: String },
    /// Get the default application for a mime type
    Get { mime: String },
    /// Set the default application for a mime type
    Set { mime: String, command: String },
}

// real main
async fn appman<'a>() -> Result<&'a str> {
    // init
    let args = App::parse();

    // get current config
    let mut cnf = config::Config::get_config();

    // ...
    match &args.command {
        Commands::File { path } => {
            let guess = mime_guess::from_path(&path);
            let mime = guess.first().unwrap().to_string();
            match cnf.mimes.get(&mime) {
                Some(application) => {
                    println!("info: {application}");
                    Ok("Finished.")
                }
                None => Ok("No default application set."),
            }
        }
        Commands::Get { mime } => match cnf.mimes.get(mime) {
            Some(application) => {
                println!("info: {application}");
                Ok("Finished.")
            }
            None => Ok("No default application set."),
        },
        Commands::Set { mime, command } => {
            let _ = cnf.mimes.insert(mime.to_string(), command.to_string());
            config::Config::update_config(cnf.clone())?;
            Ok("Finished.")
        }
    }
}

// fake main
#[tokio::main]
async fn main() {
    match appman().await {
        Ok(s) => yes(s),
        Err(e) => no(&e.to_string()),
    }
}
