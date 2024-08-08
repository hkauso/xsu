//! Lily gardens (repositories)
use crate::model::LilyError;
use crate::pack::Pack;
use crate::stage::{LocalStage, Stage};
use crate::patch::Patch;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
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
    /// A [`u128`] ID which denotes the time the branch was created
    pub timestamp: u128,
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
    pub fn get_object(&self, source: String) -> File {
        File::open(format!("{source}/.garden/objects/{}", self.id))
            .unwrap_or(File::open(format!("{source}/.garden/objects/blank")).unwrap())
    }

    /// Render the commit to an HTML string
    pub fn render(&self, long: bool) -> String {
        let mut out = String::new();

        // add header
        out.push_str(&format!(
            "<header>
                <h3>{}</h3>
                <span class=\"lily:commit_status_line\">
                    <a href=\"../index.html\">&lt; Back</a> \u{2022} 
                    <span class=\"lily:commit_branch\">{}</span> \u{2022} 
                    <span class=\"lily:commit_author\">{}</span> \u{2022} 
                    <span class=\"lily:timestamp\">{}</span>
                </span>
                <hr />
                <p class=\"lily:commit_message\">{}</p>
                <hr />
            </header>",
            self.id, self.branch, self.author, self.timestamp, self.message
        ));

        // add patch
        for file in self.content.render_html(long) {
            out.push_str(&file);
        }

        // return
        out
    }

    /// Get the short ID of the commit
    pub fn short(&self) -> String {
        self.id.chars().take(10).collect()
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
    /// Configuration for garden branches
    pub branch: GardenBranchConfig,
    /// The URL of the remote repository to sync to
    #[serde(default)]
    pub remote: String,
}

impl Default for GardenInfo {
    fn default() -> Self {
        Self {
            branch: GardenBranchConfig::default(),
            remote: String::new(),
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
    pub base: Option<xsu_dataman::StarterDatabase>,
    /// The `tracker.db` connection pool
    pub tracker: Option<xsu_dataman::StarterDatabase>,
    /// The stagefile
    pub stage: Stage,
    /// The localfdile
    pub local: LocalStage,
}

impl Garden {
    /// Create a new [`Garden`]
    pub async fn new(path: String) -> Self {
        if let Err(_) = fs::read_dir(format!("{path}/.garden")) {
            fs::mkdir(format!("{path}/.garden")).unwrap();
            fs::mkdir(format!("{path}/.garden/objects")).unwrap();

            if let Err(_) = File::open(format!("{path}/.garden/objects/blank")) {
                // create blank pack with no files just so we don't panic
                Pack::new(
                    format!("{path}/.garden/objects"),
                    Vec::new(),
                    "blank".to_string(),
                );
            }

            fs::touch(format!("{path}/.garden/lily.db")).unwrap();
            fs::touch(format!("{path}/.garden/tracker.db")).unwrap();

            fs::write(
                format!("{path}/.garden/info"),
                toml::to_string_pretty(&GardenInfo::default()).unwrap(),
            )
            .unwrap();

            fs::touch(format!("{path}/.garden/stagefile")).unwrap(); // files that are waiting to be included with a commit
            fs::touch(format!("{path}/.garden/localfile")).unwrap(); // commit hashes that haven't been synced to remote yet
        }

        // ...
        let source = fs::canonicalize(path)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        Self {
            source: source.clone(),
            info: toml::from_str(&fs::read(format!("{source}/.garden/info")).unwrap()).unwrap(),
            base: Some(
                StarterDatabase::new(DatabaseOpts {
                    name: format!("{source}/.garden/lily"),
                    ..Default::default()
                })
                .await,
            ),
            tracker: Some(
                StarterDatabase::new(DatabaseOpts {
                    name: format!("{source}/.garden/tracker"),
                    ..Default::default()
                })
                .await,
            ),
            stage: Stage(format!("{source}/.garden/stagefile")),
            local: LocalStage(format!("{source}/.garden/stagefile")),
        }
    }

    /// Create a new bare garden (no databases or stages)
    ///
    /// # Arguments
    /// * `path` - the path to the garden
    pub fn bare(path: String) -> Self {
        if let Err(_) = fs::read_dir(format!("{path}/.garden")) {
            fs::mkdir(format!("{path}/.garden")).unwrap();
            fs::mkdir(format!("{path}/.garden/objects")).unwrap();

            fs::write(
                format!("{path}/.garden/info"),
                toml::to_string_pretty(&GardenInfo::default()).unwrap(),
            )
            .unwrap();
        }

        // ...
        let source = fs::canonicalize(path)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        Self {
            source: source.clone(),
            info: toml::from_str(&fs::read(format!("{}/.garden/info", source)).unwrap()).unwrap(),
            base: None,
            tracker: None,
            stage: Stage(String::new()),
            local: LocalStage(String::new()),
        }
    }

    /// Init garden database
    pub async fn init(&self) -> () {
        let base = &self.base.as_ref().unwrap();

        // base
        let c = &base.db.client;

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"branches\" (
                id        TEXT,
                name      TEXT,
                timestamp TEXT
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

        // create initial branch
        self.create_branch("main".to_string()).await.unwrap();
    }

    /// Render the garden to HTML and write it to `.garden/web/{branch}`
    pub async fn render(&self, branch: String, verbose: bool) -> () {
        fs::mkdir(format!("{}/.garden/web", self.source)).unwrap();
        fs::rmdirr(format!("{}/.garden/web/{branch}", self.source)).unwrap(); // clear web/branch directory recursively
        fs::mkdir(format!("{}/.garden/web/{branch}", self.source)).unwrap();
        fs::mkdir(format!("{}/.garden/web/{branch}/commits", self.source)).unwrap();

        // render commits
        let commits = self.get_all_commits(branch.clone()).await.unwrap();

        let mut commits_index = String::new(); // list of all commits
        commits_index.push_str(
            "<table><thead><tr><th>ID</th><th>Branch</th><th>Author</th><th>Timestamp</th></tr></thead><tbody>",
        );

        for commit in &commits {
            // commit meta stuff
            fs::mkdir(format!(
                "{}/.garden/web/{branch}/commits/{}",
                self.source, commit.id
            ))
            .unwrap();

            // commit files
            fs::mkdir(format!(
                "{}/.garden/web/{branch}/commits/{}/tree",
                self.source, commit.id
            ))
            .unwrap();

            // add to index
            commits_index.push_str(&format!(
                "<tr class=\"lily:commit_listing\" role=\"commit\">
                    <td>
                        <a href=\"{}/index.html\">{}</a> (<a href=\"{}/tree.html\">tree</a>)
                    </td>
                    <td>{}</td>
                    <td>{}</td>
                    <td class=\"lily:timestamp\">{}</td>
                </tr>",
                commit.id,
                commit.short(),
                commit.id,
                commit.branch,
                commit.author,
                commit.timestamp
            ));

            // render main commit page
            let path = format!(
                "{}/.garden/web/{branch}/commits/{}/index.html",
                self.source, commit.id
            );

            if verbose {
                println!("{path}");
            }

            fs::write(path, commit.render(false)).unwrap();

            // render file index
            let mut file_index = String::new(); // listing of all files
            file_index.push_str("<a href=\"index.html\">&lt; Back</a><table><thead><tr><th>Path</th><th>Commit</th><th>Branch</th></tr></thead><tbody>");

            let commit_pack = Pack::from_file(commit.get_object(self.source.clone()));

            // render files
            for file in commit_pack {
                let mut out = String::new();

                // add header
                out.push_str(&format!(
                    "<header>
                         <h3>{}</h3>
                        <span class=\"lily:file_status_line\">
                            <span class=\"lily:file_index_link\"><a href=\"../tree.html\">&lt; Back</a></span> \u{2022} 
                            <span class=\"lily:file_commit\"><a href=\"../tree.html\">{}</a></span> \u{2022} 
                            <span class=\"lily:file_branch\">{}</span>
                        </span>
                        <hr />
                    </header>",
                    file.0, commit.id, commit.branch
                ));

                // add file
                let mut line_numbers = String::new();
                let mut lines = String::new();

                for line in file.1.split("\n").enumerate() {
                    // line number
                    line_numbers.push_str(&format!("<span style=\"color: blue\" class=\"lily:94m\" role=\"extra\"><code class=\"lily:u0098\" id=\"ln-{}\">{}</code></span>\n", line.0 + 1, line.0 + 1));

                    // line
                    lines.push_str(&format!(
                        "{}\n",
                        line.1.replace("<", "&lt;").replace(">", "&gt;")
                    ));
                }

                // ...
                out.push_str(&format!("<article style=\"display: flex; gap: 0.5rem\"><pre role=\"line-numbers\" class=\"lily:file_line_numbers\">{line_numbers}</pre>
                <pre role=\"source-display\" class=\"lily:file_lines\" style=\"width: 100%; overflow-x: auto\">{lines}</pre></article>"));

                // write
                let path = format!(
                    "{}/.garden/web/{branch}/commits/{}/tree/{}.html",
                    self.source,
                    commit.id,
                    file.0.replace("/", ">")
                );

                if verbose {
                    println!("{path}");
                }

                fs::write(path, out).unwrap();

                // add to index
                file_index.push_str(&format!(
                    "<tr class=\"lily:file_listing\" role=\"file\">
                        <td><a href=\"tree/{}.html\">{}</a></td>
                        <td>{}</td>
                        <td>{}</td>
                    </tr>",
                    file.0.replace("/", ">"),
                    file.0,
                    commit.short(),
                    commit.branch
                ))
            }

            // finish file index
            file_index.push_str("</tbody></table>");
            fs::write(
                format!(
                    "{}/.garden/web/{branch}/commits/{}/tree.html",
                    self.source, commit.id
                ),
                file_index,
            )
            .unwrap();
        }

        // finish commits index
        commits_index.push_str("</tbody></table>");
        fs::write(
            format!("{}/.garden/web/{branch}/commits/index.html", self.source),
            commits_index,
        )
        .unwrap();
    }

    /// Convert the garden database into plain files for [`Pack`]ing
    pub async fn serialize(&self, verbose: bool) -> () {
        fs::rmdirr(format!("{}/.garden/bin", self.source)).unwrap(); // clear existing
        fs::mkdir(format!("{}/.garden/bin", self.source)).unwrap();
        fs::mkdir(format!("{}/.garden/bin/branches", self.source)).unwrap();

        // get branches
        for branch in self.get_all_branches().await.unwrap() {
            let path = format!("{}/.garden/bin/branches/{}", self.source, branch.name);

            if verbose {
                println!("{path}");
            }

            fs::mkdir(&path).unwrap();
            fs::mkdir(format!("{path}/commits")).unwrap();

            // get commits
            let commits = self.get_all_commits(branch.name.clone()).await.unwrap();

            for commit in commits {
                let path = format!("{path}/commits/{}.json.gz", commit.id);

                if verbose {
                    println!("{path}");
                }

                fs::write(
                    path,
                    Pack::from_string(serde_json::to_string(&commit).unwrap()),
                )
                .unwrap();
            }

            // write branch
            fs::write(
                format!("{path}/branch.json"),
                serde_json::to_string(&branch).unwrap(),
            )
            .unwrap();
        }
    }

    /// Convert a serialized garden database into a SQLite file
    pub async fn deserialize(&self, path: String, verbose: bool) -> () {
        // get branches
        for branch in fs::read_dir(format!("{path}/branches")).unwrap() {
            let branch = branch.unwrap().file_name().into_string().unwrap();
            let branch_data: Branch = serde_json::from_str(
                &fs::read(format!("{path}/branches/{branch}/branch.json")).unwrap(),
            )
            .unwrap();

            let path = format!("{}/.garden/bin/branches/{}", self.source, branch);

            if verbose {
                println!("{path}");
            }

            // create branch
            let base = &self.base.as_ref().unwrap();
            let query: &str = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
                "INSERT INTO \"branches\" VALUES (?, ?, ?)"
            } else {
                "INSERT INTO \"branches\" VALUES ($1, $2, $3)"
            };

            let c = &base.db.client;
            if let Err(e) = sqlquery(query)
                .bind::<&String>(&branch_data.id)
                .bind::<&String>(&branch_data.name)
                .bind::<&String>(&branch_data.timestamp.to_string())
                .execute(c)
                .await
            {
                panic!("{e}")
            }

            // get commits
            for commit in fs::read_dir(format!("{path}/commits")).unwrap() {
                let commit = commit.unwrap().file_name().into_string().unwrap();

                let mut bytes = Vec::new();
                for byte in File::bytes(File::open(format!("{path}/commits/{commit}")).unwrap()) {
                    // why?
                    bytes.push(byte.unwrap());
                }

                let commit_data: Commit = serde_json::from_str(&Pack::decode_vec(bytes)).unwrap();

                let path = format!("{path}/commits/{commit}");

                if verbose {
                    println!("{path}");
                }

                // create commit
                let query: &str = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
                    "INSERT INTO \"commits\" VALUES (?, ?, ?, ?, ?, ?)"
                } else {
                    "INSERT INTO \"commits\" VALUES ($1, $2, $3, $5, $6)"
                };

                let c = &base.db.client;
                if let Err(e) = sqlquery(query)
                    .bind::<&String>(&commit_data.id)
                    .bind::<&String>(&commit_data.branch)
                    .bind::<&String>(&commit_data.timestamp.to_string())
                    .bind::<&String>(&commit_data.author)
                    .bind::<&String>(&commit_data.message)
                    .bind::<&[u8]>(&Pack::from_string(
                        match serde_json::to_string(&commit_data.content) {
                            Ok(m) => m,
                            Err(e) => panic!("{e}"),
                        },
                    ))
                    .execute(c)
                    .await
                {
                    panic!("{e}")
                }
            }
        }
    }

    // Set the current remote
    ///
    /// # Arguments
    /// * `url` - the url of the remote
    pub async fn set_remote(&mut self, url: String) -> () {
        self.info.remote = url;

        fs::write(
            format!("{}/.garden/info", self.source),
            toml::to_string_pretty(&self.info).unwrap(),
        )
        .unwrap()
    }

    // branches

    // GET
    /// Get a [`Branch`] by its ID
    ///
    /// # Arguments
    /// * `id` - `String` of the branch ID
    pub async fn get_branch(&self, id: String) -> Result<Branch> {
        // fetch from database
        let base = &self.base.as_ref().unwrap();
        let query: &str = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "SELECT * FROM \"branches\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"branches\" WHERE \"id\" = $1"
        };

        let c = &base.db.client;
        let row = match sqlquery(query).bind::<&String>(&id).fetch_one(c).await {
            Ok(u) => base.textify_row(u, Vec::new()).0,
            Err(_) => return Err(LilyError::Other),
        };

        // return
        Ok(Branch {
            id: row.get("id").unwrap().to_string(),
            name: row.get("name").unwrap().to_string(),
            timestamp: row.get("timestamp").unwrap().parse::<u128>().unwrap(),
        })
    }

    /// Get a [`Branch`] by its name
    ///
    /// # Arguments
    /// * `name` - `String` of the branch name
    pub async fn get_branch_by_name(&self, name: String) -> Result<Branch> {
        // fetch from database
        let base = &self.base.as_ref().unwrap();
        let query: &str = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "SELECT * FROM \"branches\" WHERE \"name\" = ?"
        } else {
            "SELECT * FROM \"branches\" WHERE \"name\" = $1"
        };

        let c = &base.db.client;
        let row = match sqlquery(query).bind::<&String>(&name).fetch_one(c).await {
            Ok(u) => base.textify_row(u, Vec::new()).0,
            Err(_) => return Err(LilyError::Other),
        };

        // return
        Ok(Branch {
            id: row.get("id").unwrap().to_string(),
            name: row.get("name").unwrap().to_string(),
            timestamp: row.get("timestamp").unwrap().parse::<u128>().unwrap(),
        })
    }

    /// Get all branches stored in the database
    pub async fn get_all_branches(&self) -> Result<Vec<Branch>> {
        // pull from database
        let base = &self.base.as_ref().unwrap();
        let query: String = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "SELECT * FROM \"branches\" ORDER BY \"timestamp\" DESC"
        } else {
            "SELECT * FROM \"branches\" ORDER BY \"timestamp\" DESC"
        }
        .to_string();

        let c = &base.db.client;
        let res = match sqlquery(&query).fetch_all(c).await {
            Ok(p) => {
                let mut out = Vec::new();

                for row in p {
                    let res = base.textify_row(row, Vec::new()).0;

                    out.push(Branch {
                        id: res.get("id").unwrap().to_string(),
                        name: res.get("name").unwrap().to_string(),
                        timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
                    })
                }

                out
            }
            Err(_) => return Err(LilyError::Other),
        };

        // return
        Ok(res)
    }

    // SET
    /// Create a new [`Branch`]
    ///
    /// # Arguments
    /// * `name` - the name of the branch
    pub async fn create_branch(&self, name: String) -> Result<String> {
        let base = &self.base.as_ref().unwrap();
        let query: &str = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "INSERT INTO \"branches\" VALUES (?, ?, ?)"
        } else {
            "INSERT INTO \"branches\" VALUES ($1, $2, $3)"
        };

        let id: String = utility::random_id();

        let c = &base.db.client;
        match sqlquery(query)
            .bind::<&String>(&id)
            .bind::<&String>(&name)
            .bind::<&String>(&utility::unix_epoch_timestamp().to_string())
            .execute(c)
            .await
        {
            Ok(_) => Ok(id),
            Err(_) => Err(LilyError::Other),
        }
    }

    /// Set the current branch
    ///
    /// # Arguments
    /// * `name` - the name of the branch
    pub async fn set_branch(&mut self, name: String) -> () {
        self.info.branch.current = name;

        fs::write(
            format!("{}/.garden/info", self.source),
            toml::to_string_pretty(&self.info).unwrap(),
        )
        .unwrap()
    }

    // TODO: delete branch

    // commits

    // GET
    /// Get an existing [`Commit`]
    ///
    /// ## Arguments
    /// * `id` - the ID of the commit to select
    pub async fn get_commit(&self, id: String) -> Result<Commit> {
        // pull from database
        let base = &self.base.as_ref().unwrap();
        let query: String = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "SELECT * FROM \"commits\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"commits\" WHERE \"id\" = $1"
        }
        .to_string();

        let c = &base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&id.to_lowercase())
            .fetch_one(c)
            .await
        {
            Ok(p) => base.textify_row(p, vec!["content".to_string()]),
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
        let base = &self.base.as_ref().unwrap();
        let query: String = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "SELECT * FROM \"commits\" ORDER BY \"timestamp\" DESC LIMIT 1"
        } else {
            "SELECT * FROM \"commits\" ORDER BY \"timestamp\" DESC LIMIT 1"
        }
        .to_string();

        let c = &base.db.client;
        let res = match sqlquery(&query).fetch_one(c).await {
            Ok(p) => base.textify_row(p, vec!["content".to_string()]),
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

    /// Get all commits stored in the database
    ///
    /// # Arguments
    /// * `branch` - the name of the branch to filter by
    pub async fn get_all_commits(&self, branch: String) -> Result<Vec<Commit>> {
        // pull from database
        let base = &self.base.as_ref().unwrap();
        let query: String = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "SELECT * FROM \"commits\" WHERE \"branch\" = ? ORDER BY \"timestamp\" DESC"
        } else {
            "SELECT * FROM \"commits\" WHERE \"branch\" = $1 ORDER BY \"timestamp\" DESC"
        }
        .to_string();

        let c = &base.db.client;
        let res = match sqlquery(&query).bind::<&String>(&branch).fetch_all(c).await {
            Ok(p) => {
                let mut out = Vec::new();

                for row in p {
                    let res = base.textify_row(row, vec!["content".to_string()]);

                    let bytes_res = res.1;
                    let res = res.0;

                    out.push(Commit {
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

                out
            }
            Err(_) => return Err(LilyError::Other),
        };

        // return
        Ok(res)
    }

    // SET
    /// Create a new [`Commit`]
    ///
    /// # Arguments
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
        let latest_pack = Pack::from_file(latest_commit.get_object(self.source.clone()));
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
            let file_patch = Patch::from_file(
                file.clone(),
                previous,
                fs::read(file).unwrap_or(String::new()),
            );

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
        self.local.add(id.clone()).unwrap(); // since we just created this commit, chances are we haven't sent it to remote yet

        println!("Creating pack...");
        Pack::new(
            format!("{}/.garden/objects", self.source),
            files,
            id.clone(),
        );
        println!("Saving commit...");

        // ...
        let base = &self.base.as_ref().unwrap();
        let query: &str = if (base.db.r#type == "sqlite") | (base.db.r#type == "mysql") {
            "INSERT INTO \"commits\" VALUES (?, ?, ?, ?, ?, ?)"
        } else {
            "INSERT INTO \"commits\" VALUES ($1, $2, $3, $5, $6)"
        };

        let c = &base.db.client;
        match sqlquery(query)
            .bind::<&String>(&id)
            .bind::<&String>(&branch)
            .bind::<&String>(&utility::unix_epoch_timestamp().to_string())
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
