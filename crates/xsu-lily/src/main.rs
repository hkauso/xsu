//! Lily CLI
use clap::{Parser, Subcommand, ArgAction};
use std::io::{Error, ErrorKind, Result};
use xsu_util::{
    fs,
    process::{no, yes},
};

use xsu_lily::{patch::Patch, garden::Garden, pack::Pack};

// ...
#[derive(Parser, Debug)]
#[command(version, about, long_about = Option::Some("Lily Version Control"))]
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
    /// Create a new branch and switch to it (or switch to an existing branch)
    Checkout { name: String },
    /// Set the remote location
    Remote { url: String },
    /// View a commit diff
    Diff {
        /// The ID of the commit
        commit: String,
        /// If we should return the diff as HTML
        #[arg(short = 'H', long, action = ArgAction::SetTrue)]
        html: bool,
        /// If large diffs should be rendered
        #[arg(short, long, action = ArgAction::SetTrue)]
        long: bool,
    },
    /// View the difference between two files
    FDiff {
        first: String,
        second: String,
        /// If we should return the diff as HTML
        #[arg(short = 'H', long, action = ArgAction::SetTrue)]
        html: bool,
    },
    /// Create a repo pack
    Pack { name: String },
    /// Render garden HTML
    Render {
        branch: String,
        #[arg(short, long, action = ArgAction::SetTrue)]
        verbose: bool,
    },
    /// Create a serialized version of the main database file
    Bin {
        #[arg(short, long, action = ArgAction::SetTrue)]
        verbose: bool,
    },
    /// Take the generated bin at the given path and fill it into the main database (see: `bin`)
    Extract {
        path: String,
        #[arg(short, long, action = ArgAction::SetTrue)]
        verbose: bool,
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
        Commands::Checkout { name } => {
            let mut garden = Garden::new().await;

            if let Err(_) = garden.get_branch_by_name(name.clone()).await {
                // create branch
                garden.create_branch(name.clone()).await.unwrap();
            };

            garden.set_branch(name.clone()).await;
            Ok("Finished.")
        }
        Commands::Remote { url } => {
            let mut garden = Garden::new().await;
            garden.set_remote(url.clone()).await;
            Ok("Finished.")
        }
        Commands::Diff { commit, html, long } => {
            let garden = Garden::new().await;

            if let Ok(commit) = garden.get_commit(commit.to_string()).await {
                for output in if html == &true {
                    commit.content.render_html(long.to_owned())
                } else {
                    commit.content.render(long.to_owned())
                } {
                    println!("{output}");
                }

                return Ok("Finished.");
            }

            Err(Error::new(ErrorKind::NotFound, "Invalid commit ID."))
        }
        Commands::FDiff {
            first,
            second,
            html,
        } => {
            let patch = Patch::from_file(
                format!("{first}+{second}"),
                fs::read(first)?,
                fs::read(second)?,
            );

            for output in if html == &true {
                patch.render_html(true)
            } else {
                patch.render(true)
            } {
                println!("{output}")
            }

            Ok("Finished.")
        }
        Commands::Pack { name } => {
            println!("{}", Pack::from_repo(name.to_owned()).await.0);
            Ok("Finished.")
        }
        Commands::Render { branch, verbose } => {
            let garden = Garden::new().await;
            garden.render(branch.to_owned(), verbose.to_owned()).await;
            Ok("Finished.")
        }
        Commands::Bin { verbose } => {
            let garden = Garden::new().await;
            garden.serialize(verbose.to_owned()).await;
            Ok("Finished.")
        }
        Commands::Extract { path, verbose } => {
            let garden = Garden::new().await;
            garden
                .deserialize(path.to_owned(), verbose.to_owned())
                .await;
            Ok("Finished.")
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
