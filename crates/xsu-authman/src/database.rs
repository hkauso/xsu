use crate::model::Group;
use crate::model::{Profile, ProfileMetadata, AuthError};

use reqwest::Client as HttpClient;
use xsu_dataman::query as sqlquery;
use xsu_dataman::utility;

pub type Result<T> = std::result::Result<T, AuthError>;

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
    pub config: ServerOptions,
    pub http: HttpClient,
}

impl Database {
    /// Create a new [`Database`]
    pub async fn new(
        database_options: xsu_dataman::DatabaseOpts,
        server_options: ServerOptions,
    ) -> Self {
        let base = xsu_dataman::StarterDatabase::new(database_options).await;

        Self {
            base: base.clone(),
            config: server_options,
            http: HttpClient::new(),
        }
    }

    /// Pull [`dorsal::DatabaseOpts`] from env
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

    /// Init database
    pub async fn init(&self) {
        // create tables
        let c = &self.base.db.client;

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"xprofiles\" (
                id       TEXT,
                username TEXT,
                metadata TEXT,
                joined   TEXT,
                gid      TEXT
            )",
        )
        .execute(c)
        .await;

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"xgroups\" (
                name        TEXT,
                id          TEXT,
                permissions TEXT            
            )",
        )
        .execute(c)
        .await;
    }

    // profiles

    // GET
    /// Get a [`Profile`] by their hashed ID
    ///
    /// # Arguments:
    /// * `hashed` - `String` of the profile's hashed ID
    pub async fn get_profile_by_hashed(&self, hashed: String) -> Result<Profile> {
        // fetch from database
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xprofiles\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"xprofiles\" WHERE \"id\" = $1"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query).bind::<&String>(&hashed).fetch_one(c).await {
            Ok(u) => self.base.textify_row(u).0,
            Err(_) => return Err(AuthError::Other),
        };

        // return
        Ok(Profile {
            id: row.get("id").unwrap().to_string(),
            username: row.get("username").unwrap().to_string(),
            metadata: match serde_json::from_str(row.get("metadata").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(AuthError::ValueError),
            },
            group: row.get("gid").unwrap().parse::<i32>().unwrap_or(0),
            joined: row.get("joined").unwrap().parse::<u128>().unwrap(),
        })
    }

    /// Get a user by their unhashed ID (hashes ID and then calls [`Database::get_profile_by_hashed()`])
    ///
    /// # Arguments:
    /// * `unhashed` - `String` of the user's unhashed ID
    pub async fn get_profile_by_unhashed(&self, unhashed: String) -> Result<Profile> {
        match self
            .get_profile_by_hashed(utility::hash(unhashed.clone()))
            .await
        {
            Ok(r) => Ok(r),
            Err(_) => self.get_profile_by_unhashed_st(unhashed).await,
        }
    }

    /// Get a user by their unhashed secondary token
    ///
    /// # Arguments:
    /// * `unhashed` - `String` of the user's unhashed secondary token
    pub async fn get_profile_by_unhashed_st(&self, unhashed: String) -> Result<Profile> {
        // fetch from database
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xprofiles\" WHERE \"metadata\" LIKE ?"
        } else {
            "SELECT * FROM \"xprofiles\" WHERE \"metadata\" LIKE $1"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query)
            .bind::<&String>(&format!(
                "%\"secondary_token\":\"{}\"%",
                utility::hash(unhashed)
            ))
            .fetch_one(c)
            .await
        {
            Ok(r) => self.base.textify_row(r).0,
            Err(_) => return Err(AuthError::Other),
        };

        // return
        Ok(Profile {
            id: row.get("id").unwrap().to_string(),
            username: row.get("username").unwrap().to_string(),
            metadata: match serde_json::from_str(row.get("metadata").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(AuthError::ValueError),
            },
            group: row.get("gid").unwrap().parse::<i32>().unwrap_or(0),
            joined: row.get("joined").unwrap().parse::<u128>().unwrap(),
        })
    }

    /// Get a user by their username
    ///
    /// # Arguments:
    /// * `username` - `String` of the user's username
    pub async fn get_profile_by_username(&self, mut username: String) -> Result<Profile> {
        username = username.to_lowercase();

        // check in cache
        let cached = self
            .base
            .cachedb
            .get(format!("xsulib.authman.profile:{}", username))
            .await;

        if cached.is_some() {
            return Ok(serde_json::from_str::<Profile>(cached.unwrap().as_str()).unwrap());
        }

        // ...
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xprofiles\" WHERE \"username\" = ?"
        } else {
            "SELECT * FROM \"xprofiles\" WHERE \"username\" = $1"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query)
            .bind::<&String>(&username)
            .fetch_one(c)
            .await
        {
            Ok(r) => self.base.textify_row(r).0,
            Err(_) => return Err(AuthError::NotFound),
        };

        // store in cache
        let user = Profile {
            id: row.get("id").unwrap().to_string(),
            username: row.get("username").unwrap().to_string(),
            metadata: match serde_json::from_str(row.get("metadata").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(AuthError::ValueError),
            },
            group: row.get("gid").unwrap().parse::<i32>().unwrap_or(0),
            joined: row.get("joined").unwrap().parse::<u128>().unwrap(),
        };

        self.base
            .cachedb
            .set(
                format!("xsulib.authman.profile:{}", username),
                serde_json::to_string::<Profile>(&user).unwrap(),
            )
            .await;

        // return
        Ok(user)
    }

    // SET
    /// Create a new user given their username. Returns their hashed ID
    ///
    /// # Arguments:
    /// * `username` - `String` of the user's `username`
    pub async fn create_profile(&self, username: String) -> Result<String> {
        // make sure user doesn't already exists
        if let Ok(_) = &self.get_profile_by_username(username.clone()).await {
            return Err(AuthError::MustBeUnique);
        };

        // check username
        let regex = regex::RegexBuilder::new("^[\\w\\_\\-\\.\\!]+$")
            .multi_line(true)
            .build()
            .unwrap();

        if regex.captures(&username).iter().len() < 1 {
            return Err(AuthError::ValueError);
        }

        if (username.len() < 2) | (username.len() > 500) {
            return Err(AuthError::ValueError);
        }

        // ...
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "INSERT INTO \"xprofiles\" VALUES (?, ?, ?, ?, ?)"
        } else {
            "INSERT INTO \"xprofiles\" VALUES ($1, $2, $3, $4, $5)"
        };

        let user_id_unhashed: String = xsu_dataman::utility::uuid();
        let user_id_hashed: String = xsu_dataman::utility::hash(user_id_unhashed.clone());
        let timestamp = utility::unix_epoch_timestamp().to_string();

        let c = &self.base.db.client;
        match sqlquery(query)
            .bind::<&String>(&user_id_hashed)
            .bind::<&String>(&username.to_lowercase())
            .bind::<&String>(
                &serde_json::to_string::<ProfileMetadata>(&ProfileMetadata::default()).unwrap(),
            )
            .bind::<&String>(&timestamp)
            .bind::<&i32>(&0)
            .execute(c)
            .await
        {
            Ok(_) => Ok(user_id_unhashed),
            Err(_) => Err(AuthError::Other),
        }
    }

    /// Update a [`Profile`]'s metadata by its `username`
    pub async fn edit_profile_metadata_by_name(
        &self,
        name: String,
        metadata: ProfileMetadata,
    ) -> Result<()> {
        // make sure user exists
        if let Err(e) = self.get_profile_by_username(name.clone()).await {
            return Err(e);
        };

        // update user
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "UPDATE \"xprofiles\" SET \"metadata\" = ? WHERE \"username\" = ?"
        } else {
            "UPDATE \"xprofiles\" SET (\"metadata\") = ($1) WHERE \"username\" = $2"
        };

        let c = &self.base.db.client;
        let meta = &serde_json::to_string(&metadata).unwrap();
        match sqlquery(query)
            .bind::<&String>(meta)
            .bind::<&String>(&name)
            .execute(c)
            .await
        {
            Ok(_) => {
                self.base
                    .cachedb
                    .remove(format!("xsulib.authman.profile:{}", name))
                    .await;
                Ok(())
            }
            Err(_) => Err(AuthError::Other),
        }
    }

    /// Update a [`Profile`]'s `gid` by its `username`
    pub async fn edit_profile_group_by_name(&self, name: String, group: i32) -> Result<()> {
        // make sure user exists
        if let Err(e) = self.get_profile_by_username(name.clone()).await {
            return Err(e);
        };

        // update user
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "UPDATE \"xprofiles\" SET \"gid\" = ? WHERE \"username\" = ?"
        } else {
            "UPDATE \"xprofiles\" SET (\"gid\") = ($1) WHERE \"username\" = $2"
        };

        let c = &self.base.db.client;
        match sqlquery(query)
            .bind::<&i32>(&group)
            .bind::<&String>(&name)
            .execute(c)
            .await
        {
            Ok(_) => {
                self.base
                    .cachedb
                    .remove(format!("xsulib.authman.profile:{}", name))
                    .await;
                Ok(())
            }
            Err(_) => Err(AuthError::Other),
        }
    }

    // groups

    // GET
    /// Get a group by its id
    ///
    /// # Arguments:
    /// * `username` - `String` of the user's username
    pub async fn get_group_by_id(&self, id: i32) -> Result<Group> {
        // check in cache
        let cached = self
            .base
            .cachedb
            .get(format!("xsulib.authman.gid:{}", id))
            .await;

        if cached.is_some() {
            return Ok(serde_json::from_str::<Group>(cached.unwrap().as_str()).unwrap());
        }

        // ...
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xgroups\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"xgroups\" WHERE \"id\" = $1"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query).bind::<&i32>(&id).fetch_one(c).await {
            Ok(r) => self.base.textify_row(r).0,
            Err(_) => return Ok(Group::default()),
        };

        // store in cache
        let group = Group {
            name: row.get("name").unwrap().to_string(),
            id: row.get("id").unwrap().parse::<i32>().unwrap(),
            permissions: match serde_json::from_str(row.get("permissions").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(AuthError::ValueError),
            },
        };

        self.base
            .cachedb
            .set(
                format!("xsulib.authman.gid:{}", id),
                serde_json::to_string::<Group>(&group).unwrap(),
            )
            .await;

        // return
        Ok(group)
    }
}
