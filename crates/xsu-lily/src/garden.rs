//! Lily gardens (repositories)
use crate::model::LilyError;
use crate::pack::Pack;
use crate::stage::Stage;
use crate::patch::Patch;

use std::collections::BTreeMap;
use std::fs::File;
use serde::{Serialize, Deserialize};

use xsu_util::fs;
use xsu_dataman::{StarterDatabase, DatabaseOpts};
use xsu_dataman::query as sqlquery;
use xsu_dataman::utility;

pub type Result<T> = std::result::Result<T, LilyError>;

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
                files: BTreeMap::new(),
            },
        }
    }
}

impl Commit {
    /// Get the gzip archive object file for the commit
    pub fn get_object(&self) -> File {
        File::open(format!(".garden/objects/{}", self.id))
            .unwrap_or(File::open(".garden/objects/blank").unwrap())
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

            if let Err(_) = File::open(".garden/objects/blank") {
                // create blank pack with no files just so we don't panic
                Pack::new(Vec::new(), "blank".to_string());
            }

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
                content   BLOB
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
            Ok(u) => self.base.textify_row(u, Vec::new()).0,
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
            Ok(p) => self.base.textify_row(p, vec!["content".to_string()]),
            Err(_) => return Err(LilyError::Other),
        };

        let bytes_res = res.1;
        let res = res.0;

        // return
        Ok(Commit {
            id: res.get("id").unwrap().to_string(),
            branch: res.get("branch").unwrap().to_string(),
            timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
            author: res.get("author").unwrap().to_string(),
            message: res.get("message").unwrap().to_string(),
            content: match serde_json::from_str(&Pack::decode_vec(
                bytes_res.get("content").unwrap().to_owned(),
            )) {
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
            Ok(p) => self.base.textify_row(p, vec!["content".to_string()]),
            Err(_) => return Err(LilyError::Other),
        };

        let bytes_res = res.1;
        let res = res.0;

        // return
        Ok(Commit {
            id: res.get("id").unwrap().to_string(),
            branch: res.get("branch").unwrap().to_string(),
            timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
            author: res.get("author").unwrap().to_string(),
            message: res.get("message").unwrap().to_string(),
            content: match serde_json::from_str(&Pack::decode_vec(
                bytes_res.get("content").unwrap().to_owned(),
            )) {
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
            files: BTreeMap::new(),
        };

        let files = match self.stage.get_files() {
            Ok(r) => r,
            Err(_) => return Err(LilyError::Other),
        };

        let latest_commit = self.get_latest_commit().await.unwrap_or_default();
        let latest_pack = Pack::from_file(latest_commit.get_object());
        let mut file_names = Vec::new();

        for file in &files {
            if file.is_empty() {
                continue;
            }

            // get previous file content
            let previous = latest_pack.get(file);
            let previous = match previous {
                Some(previous) => previous.to_owned(),
                None => String::new(),
            };

            // ...
            file_names.push(file.clone());
            let file_patch = Patch::from_file(file.clone(), previous, fs::read(file).unwrap());

            for file in file_patch.files {
                // if the file literally did not change then we should skip here!
                // file.1.1 = changes vec
                if file.1 .1.len() == 0 {
                    continue;
                }

                // ...
                patch.files.insert(file.0, file.1);
            }
        }

        // check for deleted files
        for file in latest_pack {
            if file_names.contains(&file.0) {
                // if we've previously seen this file name then we don't need to
                // say it was fully deleted
                continue;
            }

            // this should just be a complete deletion on every line
            // this isn't dont by the previous for loop because that loops over the files
            // that we CURRENTLY have... which means anything deleted just isn't shown
            let file_patch = Patch::from_file(file.0.clone(), file.1, String::new());

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
            .bind::<&[u8]>(&Pack::from_string(match serde_json::to_string(&patch) {
                Ok(m) => m,
                Err(_) => return Err(LilyError::ValueError),
            }))
            .execute(c)
            .await
        {
            Ok(_) => Ok(id),
            Err(_) => Err(LilyError::Other),
        }
    }
}
