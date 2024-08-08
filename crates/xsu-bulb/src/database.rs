use crate::model::{RepositoryCreate, DatabaseError, Repository, RepositoryMetadata};

use xsu_dataman::utility;
use xsu_dataman::query as sqlquery;
use xsu_authman::model::{Permission, Profile};

pub type Result<T> = std::result::Result<T, DatabaseError>;

#[derive(Clone, Debug)]
pub struct ServerOptions {}

impl ServerOptions {
    /// Enable all options
    pub fn truthy() -> Self {
        Self {}
    }
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {}
    }
}

/// Database connector
#[derive(Clone)]
pub struct Database {
    pub base: xsu_dataman::StarterDatabase,
    pub auth: xsu_authman::Database,
    pub options: ServerOptions,
}

impl Database {
    pub async fn new(
        opts: xsu_dataman::DatabaseOpts,
        opts1: ServerOptions,
        auth: xsu_authman::Database,
    ) -> Self {
        Self {
            base: xsu_dataman::StarterDatabase::new(opts).await,
            auth,
            options: opts1,
        }
    }

    /// Init database
    pub async fn init(&self) {
        // create tables
        let c = &self.base.db.client;

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"xrepos\" (
                 name           TEXT,
                 owner          TEXT,
                 date_published TEXT,
                 metadata       TEXT
            )",
        )
        .execute(c)
        .await;
    }

    /// Pull [`xsu_dataman::DatabaseOpts`] from env
    pub fn env_options() -> xsu_dataman::DatabaseOpts {
        use std::env::var;
        xsu_dataman::DatabaseOpts {
            r#type: match var("DB_TYPE") {
                Ok(v) => Option::Some(v),
                Err(_) => Option::None,
            },
            host: match var("DB_HOST") {
                Ok(v) => Option::Some(v),
                Err(_) => Option::None,
            },
            user: var("DB_USER").unwrap_or(String::new()),
            pass: var("DB_PASS").unwrap_or(String::new()),
            name: var("DB_NAME").unwrap_or(String::new()),
        }
    }

    // ...

    /// Get an existing repository
    ///
    /// ## Arguments:
    /// * `path`
    /// * `owner`
    pub async fn get_repository(&self, name: String, owner: String) -> Result<Repository> {
        // check in cache
        match self
            .base
            .cachedb
            .get(format!("xsulib.bulb:{}:{}", owner, name))
            .await
        {
            Some(c) => return Ok(serde_json::from_str::<Repository>(c.as_str()).unwrap()),
            None => (),
        };

        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"xrepos\" WHERE \"name\" = ? AND \"owner\" = ?"
        } else {
            "SELECT * FROM \"xrepos\" WHERE \"name\" = $1 AND \"owner\" = $2"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&name.to_lowercase())
            .bind::<&String>(&owner.to_lowercase())
            .fetch_one(c)
            .await
        {
            Ok(p) => self.base.textify_row(p, Vec::new()).0,
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        let repo = Repository {
            name: res.get("name").unwrap().to_string(),
            owner: res.get("owner").unwrap().to_string(),
            date_published: res.get("date_published").unwrap().parse::<u128>().unwrap(),
            metadata: match serde_json::from_str(res.get("metadata").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(DatabaseError::ValueError),
            },
        };

        // store in cache
        self.base
            .cachedb
            .set(
                format!("xsulib.bulb:{}:{}", owner, name),
                serde_json::to_string::<Repository>(&repo).unwrap(),
            )
            .await;

        // return
        Ok(repo)
    }

    /// Get all existing repository by their owner
    ///
    /// ## Arguments:
    /// * `owner`
    pub async fn get_repositories_by_owner(&self, owner: String) -> Result<Vec<Repository>> {
        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"xrepos\" WHERE \"owner\" = ?"
        } else {
            "SELECT * FROM \"xrepos\" WHERE \"owner\" = $2"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&owner.to_lowercase())
            .fetch_all(c)
            .await
        {
            Ok(p) => {
                let mut out: Vec<Repository> = Vec::new();

                for row in p {
                    let res = self.base.textify_row(row, Vec::new()).0;
                    out.push(Repository {
                        name: res.get("name").unwrap().to_string(),
                        owner: res.get("owner").unwrap().to_string(),
                        date_published: res.get("date_published").unwrap().parse::<u128>().unwrap(),
                        metadata: match serde_json::from_str(res.get("metadata").unwrap()) {
                            Ok(m) => m,
                            Err(_) => return Err(DatabaseError::ValueError),
                        },
                    });
                }

                out
            }
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        Ok(res)
    }

    /// Create a new repository
    ///
    /// ## Arguments:
    /// * `props` - [`RepositoryCreate`]
    pub async fn create_repository(
        &self,
        mut props: RepositoryCreate,
        owner: String,
    ) -> Result<()> {
        // make sure repo doesn't already exist
        if let Ok(_) = self.get_repository(props.name.clone(), owner.clone()).await {
            return Err(DatabaseError::AlreadyExists);
        }

        // create name if not supplied
        if props.name.is_empty() {
            props.name = utility::random_id().chars().take(10).collect();
        }

        // check lengths
        if (props.name.len() > 250) | (props.name.len() < 3) {
            return Err(DatabaseError::ValueError);
        }

        // (characters used)
        let regex = regex::RegexBuilder::new("^[\\w\\_\\-\\.\\!]+$")
            .multi_line(true)
            .build()
            .unwrap();

        if regex.captures(&props.name).iter().len() < 1 {
            return Err(DatabaseError::ValueError);
        }

        // ...
        let repo = Repository {
            name: props.name,
            owner,
            date_published: utility::unix_epoch_timestamp(),
            metadata: RepositoryMetadata::default(),
        };

        // create repository
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "INSERT INTO \"xrepos\" VALUES (?, ?, ?, ?)"
        } else {
            "INSERT INTO \"xrepos\" VALEUS ($1, $2, $3, $4)"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&repo.name)
            .bind::<&String>(&repo.owner)
            .bind::<&String>(&repo.date_published.to_string())
            .bind::<&String>(match serde_json::to_string(&repo.metadata) {
                Ok(ref s) => s,
                Err(_) => return Err(DatabaseError::ValueError),
            })
            .execute(c)
            .await
        {
            Ok(_) => return Ok(()),
            Err(_) => return Err(DatabaseError::Other),
        };
    }

    /// Delete an existing repository
    ///
    /// ## Arguments:
    /// * `name`
    /// * `owner`
    /// * `user` - the user doing this
    pub async fn delete_repository(
        &self,
        name: String,
        owner: String,
        user: Profile,
    ) -> Result<()> {
        // make repository exists
        if let Err(e) = self.get_repository(name.clone(), owner.clone()).await {
            return Err(e);
        };

        // check username
        if user.username != owner {
            // check permission
            let group = match self.auth.get_group_by_id(user.group).await {
                Ok(g) => g,
                Err(_) => return Err(DatabaseError::Other),
            };

            if !group.permissions.contains(&Permission::Manager) {
                return Err(DatabaseError::NotAllowed);
            }
        }

        // delete repository
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "DELETE FROM \"xrepos\" WHERE \"name\" = ? AND \"owner\" = ?"
        } else {
            "DELETE FROM \"xrepos\" WHERE \"name\" = $1 AND \"owner\" = $2"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&name)
            .bind::<&String>(&owner)
            .execute(c)
            .await
        {
            Ok(_) => {
                // remove from cache
                self.base
                    .cachedb
                    .remove(format!("xsulib.bulb:{}:{}", owner, name))
                    .await;

                // return
                return Ok(());
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }

    /// Edit an existing repository's metadata
    ///
    /// ## Arguments:
    /// * `path`
    /// * `owner`
    /// * `metadata` - the new metadata of the repository
    /// * `user` - the user doing this
    pub async fn edit_repository_metadata(
        &self,
        name: String,
        owner: String,
        metadata: RepositoryMetadata,
        user: Profile,
    ) -> Result<()> {
        // make sure repository exists
        if let Err(e) = self.get_repository(name.clone(), owner.clone()).await {
            return Err(e);
        };

        // check username
        if user.username != owner {
            // check permission
            let group = match self.auth.get_group_by_id(user.group).await {
                Ok(g) => g,
                Err(_) => return Err(DatabaseError::Other),
            };

            if !group.permissions.contains(&Permission::Manager) {
                return Err(DatabaseError::NotAllowed);
            }
        }

        // edit repository
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "UPDATE \"xrepos\" SET \"metadata\" = ? WHERE \"name\" = ? AND \"owner\" = ?"
        } else {
            "UPDATE \"xrepos\" SET (\"metadata\" = $1) WHERE \"name\" = $2 AND \"owner\" = $3"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(match serde_json::to_string(&metadata) {
                Ok(ref m) => m,
                Err(_) => return Err(DatabaseError::ValueError),
            })
            .bind::<&String>(&name)
            .bind::<&String>(&owner)
            .execute(c)
            .await
        {
            Ok(_) => {
                // remove from cache
                self.base
                    .cachedb
                    .remove(format!("xsulib.bulb:{}:{}", owner, name))
                    .await;

                // return
                return Ok(());
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }
}
