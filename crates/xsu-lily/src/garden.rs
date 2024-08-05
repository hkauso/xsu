//! Lily gardens (repositories)
use crate::model::LilyError;
use crate::pack::Pack;
use crate::stage::Stage;

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

use xsu_util::fs;
use xsu_dataman::{StarterDatabase, DatabaseOpts};
use xsu_dataman::query as sqlquery;
use xsu_dataman::utility;

pub type Result<T> = std::result::Result<T, LilyError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeMode {
    /// Something was added
    Added,
    /// Something was deleted
    Deleted,
}

/// A single change to a file
///
/// ```
/// (line number, mode, line)
/// ```
pub type Change = (usize, ChangeMode, String);

/// A file inside of a [`Patch`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchFile(String, Vec<Change>);

impl Default for PatchFile {
    fn default() -> Self {
        Self(String::new(), Vec::new())
    }
}

impl PatchFile {
    /// Get a summary of the changes in this [`PatchFile`]
    ///
    /// # Returns
    /// `(total changes, additions, deletions)`
    pub fn summary(&self) -> (usize, usize, usize) {
        let mut additions = 0;
        let mut deletions = 0;

        for change in &self.1 {
            match change.1 {
                ChangeMode::Added => additions += 1,
                ChangeMode::Deleted => deletions += 1,
            }
        }

        (self.1.len(), additions, deletions)
    }
}

/// A list of changes to many files (paths are relative to the working tree)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// A list of files and their changes
    /// 0: old file source
    /// 1: changes
    pub files: HashMap<String, PatchFile>,
}

impl Patch {
    /// Create a [`Patch`] using the diff of two strings
    pub fn from_file(path: String, old: String, new: String) -> Patch {
        use similar::{ChangeTag, TextDiff};

        // create diff
        let diff = TextDiff::from_lines(&old, &new);

        // create patch
        let mut out = Patch {
            files: HashMap::new(),
        };

        out.files
            .insert(path.clone(), PatchFile(old.clone(), Vec::new()));

        let this = out.files.get_mut(&path).unwrap();

        for change in diff.iter_all_changes() {
            let mode = match change.tag() {
                ChangeTag::Insert => ChangeMode::Added,
                ChangeTag::Delete => ChangeMode::Deleted,
                ChangeTag::Equal => continue, // we don't store these, we only care about actual changes
            };

            this.1.push((
                change
                    .old_index()
                    .unwrap_or(change.new_index().unwrap_or(0)),
                mode,
                change.value().to_string(),
            ))
        }

        // return
        out
    }

    /// Render the patch into an array of strings
    pub fn render(&self) -> Vec<String> {
        let mut patches = Vec::new();

        for file in &self.files {
            let header = format!(
                // patch header
                "\x1b[1m{}:\n{}\n\x1b[0m",
                file.0, "───────────╮"
            );

            let mut out = String::new();

            let changes_iter = file.1 .1.iter();
            let mut consumed = Vec::new();
            for (i, line) in file.1 .0.split("\n").enumerate() {
                // check if the line was deleted
                if let Some(change) = changes_iter
                    .clone()
                    .find(|c| (c.0 == i) && (c.1 == ChangeMode::Deleted))
                {
                    out.push_str(&format!(
                        "\x1b[2m{}{}\x1b[0m \x1b[91m- │ {line}\n",
                        i + 1,
                        " ".repeat(8 - i.to_string().len())
                    ));

                    consumed.push(change);
                    continue; // this line was deleted so we shouldn't render the normal line
                }

                // push normal line
                out.push_str(&format!(
                    "\x1b[2m{}{} = │ {line}\n",
                    i + 1,
                    " ".repeat(8 - i.to_string().len())
                ));
            }

            // add new lines
            let mut lines: Vec<String> = Vec::new();

            for r#ref in out.split("\n") {
                // own split
                lines.push(r#ref.to_owned())
            }

            for change in changes_iter {
                if consumed.contains(&change) {
                    // don't process changes we've already consumed
                    // these should only be deletions
                    continue;
                }

                lines.insert(
                    // we're adding 1 to the position so that it is rendered after the removal
                    change.0 + 1,
                    format!(
                        "\x1b[2m{}{}\x1b[0m \x1b[92m+ │ {}",
                        change.0 + 1,
                        " ".repeat(8 - change.0.to_string().len()),
                        change.2.replace("\n", "")
                    ),
                );
            }

            // create footer
            let summary = file.1.summary();

            let mut footer = "\x1b[1m".to_string();
            footer.push_str("───────────╯");
            footer.push_str(&format!(
                "\x1b[0m\n{} total changes \u{2022} \x1b[92m{} additions\x1b[0m \u{2022} \x1b[91m{} deletions\x1b[0m",
                summary.0, // total
                summary.1, // additions
                summary.2, // deletions
            ));

            // ...
            patches.push(format!("{header}{}{footer}", lines.join("\n\x1b[0m")))
        }

        patches
    }
}

// ...

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    /// A unique ID to identify the branch
    pub id: String,
    /// A name to make the branch easier to reference, doesn't even have to be unique
    pub name: String,
}

/// A change to the garden state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    /// A unique ID to identify the commit
    pub id: String,
    /// The ID of the branch the commit was created in
    pub branch: String,
    /// A [`u128`] ID which denotes the time the commit was created
    pub timestamp: u128,
    /// The user who created the commit (`username@server`)
    pub author: String,
    /// A description of the changes provided by the author
    #[serde(default)]
    pub message: String,
    /// A description of the changes done to the garden state
    pub content: Patch,
}

impl Default for Commit {
    fn default() -> Self {
        Self {
            id: utility::random_id(),
            branch: utility::random_id(),
            timestamp: utility::unix_epoch_timestamp(),
            author: "system@localhost".to_string(),
            message: "Filler commit".to_string(),
            content: Patch {
                files: HashMap::new(),
            },
        }
    }
}

/// Information about a [`Garden`]'s branches
#[derive(Clone, Serialize, Deserialize)]
pub struct GardenBranchConfig {
    /// The default branch
    pub default: String,
    /// The local current branch
    pub current: String,
}

impl Default for GardenBranchConfig {
    fn default() -> Self {
        Self {
            default: "main".to_string(),
            current: "main".to_string(),
        }
    }
}

/// Information about a [`Garden`]
#[derive(Clone, Serialize, Deserialize)]
pub struct GardenInfo {
    pub branch: GardenBranchConfig,
}

impl Default for GardenInfo {
    fn default() -> Self {
        Self {
            branch: GardenBranchConfig::default(),
        }
    }
}

/// A repository of files that are tracked for version control
#[derive(Clone)]
pub struct Garden {
    /// The source location of the garden's files
    pub source: String,
    /// The info file
    pub info: GardenInfo,
    /// The `lily.db` connection pool
    pub base: xsu_dataman::StarterDatabase,
    /// The `tracker.db` connection pool
    pub tracker: xsu_dataman::StarterDatabase,
    /// The stagefile
    pub stage: Stage,
}

impl Garden {
    /// Create a new [`Garden`]
    pub async fn new() -> Self {
        if let Err(_) = fs::read_dir(".garden") {
            fs::mkdir(".garden").unwrap();
            fs::mkdir(".garden/objects").unwrap();

            fs::touch(".garden/lily.db").unwrap();
            fs::touch(".garden/tracker.db").unwrap();

            fs::write(
                ".garden/info",
                toml::to_string_pretty(&GardenInfo::default()).unwrap(),
            )
            .unwrap();

            fs::touch(".garden/stagefile").unwrap();
        }

        // ...
        Self {
            source: fs::canonicalize("./")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            info: toml::from_str(&fs::read(format!(".garden/info")).unwrap()).unwrap(),
            base: StarterDatabase::new(DatabaseOpts {
                name: format!(".garden/lily"),
                ..Default::default()
            })
            .await,
            tracker: StarterDatabase::new(DatabaseOpts {
                name: format!(".garden/tracker"),
                ..Default::default()
            })
            .await,
            stage: Stage(format!(".garden/stagefile")),
        }
    }

    /// Init garden database
    pub async fn init(&self) -> () {
        // base
        let c = &self.base.db.client;

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"branches\" (
                id   TEXT,
                name TEXT
            )",
        )
        .execute(c)
        .await;

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"commits\" (
                id        TEXT,
                branch    TEXT,
                timestamp TEXT,
                author    TEXT,
                message   TEXT,
                content   TEXT
            )",
        )
        .execute(c)
        .await;

        // TODO: tracker

        // stage
        if let Err(e) = self.stage.init() {
            panic!("STAGE ERROR: {:?}", e)
        }
    }

    // branches

    // GET
    /// Get a [`Branch`] by its ID
    ///
    /// # Arguments:
    /// * `id` - `String` of the branch ID
    pub async fn get_branch(&self, id: String) -> Result<Branch> {
        // fetch from database
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"branches\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"branches\" WHERE \"id\" = $1"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query).bind::<&String>(&id).fetch_one(c).await {
            Ok(u) => self.base.textify_row(u).0,
            Err(_) => return Err(LilyError::Other),
        };

        // return
        Ok(Branch {
            id: row.get("id").unwrap().to_string(),
            name: row.get("name").unwrap().to_string(),
        })
    }

    // SET
    /// Create a new [`Branch`]
    ///
    /// # Arguments:
    /// * `name` - the name of the branch
    pub async fn create_branch(&self, name: String) -> Result<String> {
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "INSERT INTO \"branches\" VALUES (?, ?)"
        } else {
            "INSERT INTO \"branches\" VALUES ($1, $2)"
        };

        let id: String = utility::random_id();

        let c = &self.base.db.client;
        match sqlquery(query)
            .bind::<&String>(&id)
            .bind::<&String>(&name)
            .execute(c)
            .await
        {
            Ok(_) => Ok(id),
            Err(_) => Err(LilyError::Other),
        }
    }

    // TODO: delete branch

    // commits

    // GET
    /// Get an existing [`Commit`]
    ///
    /// ## Arguments:
    /// * `id` - the ID of the commit to select
    pub async fn get_commit(&self, id: String) -> Result<Commit> {
        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"commits\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"commits\" WHERE \"id\" = $1"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&id.to_lowercase())
            .fetch_one(c)
            .await
        {
            Ok(p) => self.base.textify_row(p).0,
            Err(_) => return Err(LilyError::Other),
        };

        // return
        Ok(Commit {
            id: res.get("id").unwrap().to_string(),
            branch: res.get("branch").unwrap().to_string(),
            timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
            author: res.get("author").unwrap().to_string(),
            message: res.get("message").unwrap().to_string(),
            content: match serde_json::from_str(res.get("content").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(LilyError::ValueError),
            },
        })
    }

    /// Get a the latest existing [`Commit`]
    pub async fn get_latest_commit(&self) -> Result<Commit> {
        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"commits\" ORDER BY \"timestamp\" DESC LIMIT 1"
        } else {
            "SELECT * FROM \"commits\" ORDER BY \"timestamp\" DESC LIMIT 1"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query).fetch_one(c).await {
            Ok(p) => self.base.textify_row(p).0,
            Err(_) => return Err(LilyError::Other),
        };

        // return
        Ok(Commit {
            id: res.get("id").unwrap().to_string(),
            branch: res.get("branch").unwrap().to_string(),
            timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
            author: res.get("author").unwrap().to_string(),
            message: res.get("message").unwrap().to_string(),
            content: match serde_json::from_str(res.get("content").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(LilyError::ValueError),
            },
        })
    }

    // SET
    /// Create a new [`Commit`]
    ///
    /// # Arguments:
    /// * `branch` - the branch of the commit
    /// * `message` - the message of the commit
    /// * `author` - the author of the commit
    pub async fn create_commit(
        &self,
        branch: String,
        message: String,
        author: String,
    ) -> Result<String> {
        // build patch
        let mut patch = Patch {
            files: HashMap::new(),
        };

        let files = match self.stage.get_files() {
            Ok(r) => r,
            Err(_) => return Err(LilyError::Other),
        };

        let latest_commit = self.get_latest_commit().await.unwrap_or_default();
        for file in &files {
            if file.is_empty() {
                continue;
            }

            // generate file patch
            let previous = latest_commit.content.files.get(file.as_str());
            let previous = match previous {
                Some(previous) => previous,
                None => &PatchFile::default(),
            };

            let file_patch = Patch::from_file(
                file.clone(),
                previous
                    .1
                    .get(if previous.1.len() > 0 {
                        previous.1.len() - 1
                    } else {
                        0
                    })
                    .unwrap_or(&(0, ChangeMode::Added, String::new()))
                    .2
                    .clone(),
                fs::read(file).unwrap(),
            );

            for file in file_patch.files {
                patch.files.insert(file.0, file.1);
            }
        }

        // create pack
        let id: String = utility::random_id();

        println!("Creating pack...");
        Pack::new(files, id.clone());
        println!("Saving commit...");

        // ...
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "INSERT INTO \"commits\" VALUES (?, ?, ?, ?, ?, ?)"
        } else {
            "INSERT INTO \"commits\" VALUES ($1, $2, $3, $5, $6)"
        };

        let c = &self.base.db.client;
        match sqlquery(query)
            .bind::<&String>(&id)
            .bind::<&String>(&branch)
            .bind::<i32>(utility::unix_epoch_timestamp() as i32)
            .bind::<&String>(&author)
            .bind::<&String>(&message)
            .bind::<&String>(&match serde_json::to_string(&patch) {
                Ok(m) => m,
                Err(_) => return Err(LilyError::ValueError),
            })
            .execute(c)
            .await
        {
            Ok(_) => Ok(id),
            Err(_) => Err(LilyError::Other),
        }
    }
}
