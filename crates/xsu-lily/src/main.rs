//! Lily CLI
use clap::{Parser, Subcommand};
use std::io::{Error, ErrorKind, Result};
use xsu_util::{
    fs,
    process::{no, yes},
};

use xsu_lily::garden::Garden;

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
    /// Create a garden in the current directory
    Init {},
    /// Add files to the garden stage
    Add { files: Vec<String> },
    /// Clear the stage
    Clear {},
    /// Create a new commit
    Commit {
        /// The commit branch
        #[arg(short, long)]
        branch: String,
        /// The commit message
        #[arg(short, long)]
        message: String,
    },
    /// View a commit diff
    Diff {
        /// The ID of the commit
        commit: String,
    },
}

// real main
async fn lily<'a>() -> Result<&'a str> {
    // init
    let args = App::parse();

    // get current config
    let cnf = xsu_lily::config::Config::get_config();

    // ...
    match &args.command {
        Commands::Init {} => {
            Garden::new().await.init().await;
            Ok("Finished.")
        }
        Commands::Add { files } => {
            let garden = Garden::new().await;

            for file in files {
                if file == "." {
                    // add everything
                    let mut ignored = Vec::new();

                    for glob in fs::read(".weeds").unwrap_or("".to_string()).split("\n") {
                        ignored.push(glob.to_owned());
                    }

                    garden.stage.add_glob(ignored)?;
                    break;
                }

                // add single
                garden.stage.add(file.to_string())?;
            }

            println!("{}", fs::read(".garden/stagefile")?);
            Ok("Finished.")
        }
        Commands::Clear {} => {
            let garden = Garden::new().await;
            garden.stage.clear()?;
            Ok("Finished.")
        }
        Commands::Commit { branch, message } => {
            let garden = Garden::new().await;

            // check author id
            if cnf.user_id.is_empty() {
                return Err(Error::new(ErrorKind::Other, "Invalid user_id, please edit $HOME/.config/xsu-apps/lily/config.toml and update your user_id."));
            }

            // commit
            if let Ok(id) = garden
                .create_commit(branch.to_string(), message.to_string(), cnf.user_id)
                .await
            {
                garden.stage.clear()?; // clear stage

                // return
                println!("{id}");
                return Ok("Finished.");
            }

            Err(Error::new(ErrorKind::Other, "Failed to create commit."))
        }
        #[rustfmt::ignore]
        Commands::Diff { commit } => {
            let garden = Garden::new().await;

            if let Ok(commit) = garden.get_commit(commit.to_string()).await {
                let mut total_changes = 0;
                let mut total_additions = 0;
                let mut total_deletions = 0;

                for patch in &commit.content.files {
                    let summary = patch.1.summary();
                    total_changes += summary.0;
                    total_additions += summary.1;
                    total_deletions += summary.2;
                }

                for output in commit.content.render() {
                    println!("{output}");
                }

                println!(
                    "\x1b[0m\n{} \u{2022} {} total changes \u{2022} \x1b[92m{} additions\x1b[0m \u{2022} \x1b[91m{} deletions\x1b[0m",
                    commit.id.chars().take(10).collect::<String>(),
                    total_changes,
                    total_additions,
                    total_deletions
                );

                return Ok("Finished.");
            }

            Err(Error::new(ErrorKind::NotFound, "Invalid commit ID."))
        }
    }
}

// fake main
#[tokio::main]
async fn main() {
    match lily().await {
        Ok(s) => yes(s),
        Err(e) => no(&e.to_string()),
    }
}
