//! Markdown manager
use clap::{Parser, Subcommand};
use std::io::Result;
use xsu_util::{
    fs,
    process::{no, yes},
};

extern crate xsu_slime;

// ...
#[derive(Parser, Debug)]
#[command(version, about, long_about = Option::Some("Slime Markdown"))]
#[command(propagate_version = true)]
struct App {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Render and view a file's contents as Markdown
    View { path: String },
}

// real main
async fn slime<'a>() -> Result<&'a str> {
    // init
    let args = App::parse();

    // get current config
    let cnf = xsu_slime::config::Config::get_config();

    // ...
    match &args.command {
        Commands::View { path } => {
            let contents = match fs::read(&path) {
                Ok(c) => c,
                Err(e) => return Err(e),
            };

            // render
            let html = xsu_util::ui::render_markdown(&contents);
            let ansi = xsu_slime::transform(&cnf, html);

            // return
            println!("{ansi}");
            Ok("Finished.")
        }
    }
}

// fake main
#[tokio::main]
async fn main() {
    match slime().await {
        Ok(s) => yes(s),
        Err(e) => no(&e.to_string()),
    }
}
