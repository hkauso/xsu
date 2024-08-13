use crate::model::{Group, UserFollow};
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
                password TEXT,
                tokens   TEXT,
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

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"xfollows\" (
                user      TEXT,
                following TEXT
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
            "SELECT * FROM \"xprofiles\" WHERE \"tokens\" LIKE ?"
        } else {
            "SELECT * FROM \"xprofiles\" WHERE \"tokens\" LIKE $1"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query)
            .bind::<&String>(&format!("%\"{hashed}\"%"))
            .fetch_one(c)
            .await
        {
            Ok(u) => self.base.textify_row(u, Vec::new()).0,
            Err(_) => return Err(AuthError::Other),
        };

        // return
        Ok(Profile {
            id: row.get("id").unwrap().to_string(),
            username: row.get("username").unwrap().to_string(),
            password: row.get("password").unwrap().to_string(),
            tokens: match serde_json::from_str(row.get("tokens").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(AuthError::ValueError),
            },
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
        self.get_profile_by_hashed(utility::hash(unhashed.clone()))
            .await
    }

    /// Get a user by their unhashed secondary token
    ///
    /// # Arguments:
    /// * `unhashed` - `String` of the user's unhashed secondary token
    pub async fn get_profile_by_username_password(
        &self,
        username: String,
        mut password: String,
    ) -> Result<Profile> {
        password = xsu_dataman::utility::hash(password);

        // fetch from database
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xprofiles\" WHERE \"username\" = ? AND \"password\" = ?"
        } else {
            "SELECT * FROM \"xprofiles\" WHERE \"username\" = $1 AND \"password\" = $2"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query)
            .bind::<&String>(&username)
            .bind::<&String>(&password)
            .fetch_one(c)
            .await
        {
            Ok(r) => self.base.textify_row(r, Vec::new()).0,
            Err(_) => return Err(AuthError::Other),
        };

        // return
        Ok(Profile {
            id: row.get("id").unwrap().to_string(),
            username: row.get("username").unwrap().to_string(),
            password: row.get("password").unwrap().to_string(),
            tokens: match serde_json::from_str(row.get("tokens").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(AuthError::ValueError),
            },
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
            Ok(r) => self.base.textify_row(r, Vec::new()).0,
            Err(_) => return Err(AuthError::NotFound),
        };

        // store in cache
        let user = Profile {
            id: row.get("id").unwrap().to_string(),
            username: row.get("username").unwrap().to_string(),
            password: row.get("password").unwrap().to_string(),
            tokens: match serde_json::from_str(row.get("tokens").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(AuthError::ValueError),
            },
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
    /// Create a new user given their username. Returns their unhashed token
    ///
    /// # Arguments:
    /// * `username` - `String` of the user's `username`
    pub async fn create_profile(&self, username: String, password: String) -> Result<String> {
        // make sure user doesn't already exists
        if let Ok(_) = &self.get_profile_by_username(username.clone()).await {
            return Err(AuthError::MustBeUnique);
        };

        // check username
        let banned_usernames = &["admin", "account", "anonymous", "login", "sign_up"];

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

        if banned_usernames.contains(&username.as_str()) {
            return Err(AuthError::ValueError);
        }

        // ...
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "INSERT INTO \"xprofiles\" VALUES (?, ?, ?, ?, ?, ?, ?)"
        } else {
            "INSERT INTO \"xprofiles\" VALUES ($1, $2, $3, $4, $5, $6, $7)"
        };

        let user_token_unhashed: String = xsu_dataman::utility::uuid();
        let user_token_hashed: String = xsu_dataman::utility::hash(user_token_unhashed.clone());
        let timestamp = utility::unix_epoch_timestamp().to_string();

        let c = &self.base.db.client;
        match sqlquery(query)
            .bind::<&String>(&xsu_dataman::utility::uuid())
            .bind::<&String>(&username.to_lowercase())
            .bind::<&String>(&xsu_dataman::utility::hash(password))
            .bind::<&String>(
                &serde_json::to_string::<Vec<String>>(&vec![user_token_hashed]).unwrap(),
            )
            .bind::<&String>(
                &serde_json::to_string::<ProfileMetadata>(&ProfileMetadata::default()).unwrap(),
            )
            .bind::<&String>(&timestamp)
            .bind::<&i32>(&0)
            .execute(c)
            .await
        {
            Ok(_) => Ok(user_token_unhashed),
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

    /// Update a [`Profile`]'s tokens by its `username`
    pub async fn edit_profile_tokens_by_name(
        &self,
        name: String,
        tokens: Vec<String>,
    ) -> Result<()> {
        // make sure user exists
        if let Err(e) = self.get_profile_by_username(name.clone()).await {
            return Err(e);
        };

        // update user
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "UPDATE \"xprofiles\" SET \"tokens\" = ? WHERE \"username\" = ?"
        } else {
            "UPDATE \"xprofiles\" SET (\"tokens\") = ($1) WHERE \"username\" = $2"
        };

        let c = &self.base.db.client;
        let tokens = &serde_json::to_string(&tokens).unwrap();
        match sqlquery(query)
            .bind::<&String>(tokens)
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
            Ok(r) => self.base.textify_row(r, Vec::new()).0,
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

    // profiles

    // GET
    /// Get an existing [`UserFollow`]
    ///
    /// # Arguments:
    /// * `user`
    /// * `following`
    pub async fn get_follow(&self, user: String, following: String) -> Result<UserFollow> {
        // fetch from database
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xfollows\" WHERE \"user\" = ? AND \"following\" = ?"
        } else {
            "SELECT * FROM \"xfollows\" WHERE \"user\" = $1 AND \"following\" = $2"
        };

        let c = &self.base.db.client;
        let row = match sqlquery(query)
            .bind::<&String>(&user)
            .bind::<&String>(&following)
            .fetch_one(c)
            .await
        {
            Ok(u) => self.base.textify_row(u, Vec::new()).0,
            Err(_) => return Err(AuthError::Other),
        };

        // return
        Ok(UserFollow {
            user: row.get("user").unwrap().to_string(),
            following: row.get("following").unwrap().to_string(),
        })
    }

    /// Get all existing [`UserFollow`]s where `following` is the value of `user`
    ///
    /// # Arguments:
    /// * `user`
    pub async fn get_followers(&self, user: String) -> Result<Vec<UserFollow>> {
        // fetch from database
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xfollows\" WHERE \"following\" = ?"
        } else {
            "SELECT * FROM \"xfollows\" WHERE \"following\" = $1"
        };

        let c = &self.base.db.client;
        let res = match sqlquery(query).bind::<&String>(&user).fetch_all(c).await {
            Ok(u) => {
                let mut out = Vec::new();

                for row in u {
                    let row = self.base.textify_row(row, Vec::new()).0;
                    out.push(UserFollow {
                        user: row.get("user").unwrap().to_string(),
                        following: row.get("following").unwrap().to_string(),
                    })
                }

                out
            }
            Err(_) => return Err(AuthError::Other),
        };

        // return
        Ok(res)
    }

    /// Get all existing [`UserFollow`]s where `user` is the value of `user`
    ///
    /// # Arguments:
    /// * `user`
    pub async fn get_following(&self, user: String) -> Result<Vec<UserFollow>> {
        // fetch from database
        let query: &str = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
            "SELECT * FROM \"xfollows\" WHERE \"user\" = ?"
        } else {
            "SELECT * FROM \"xfollows\" WHERE \"user\" = $1"
        };

        let c = &self.base.db.client;
        let res = match sqlquery(query).bind::<&String>(&user).fetch_all(c).await {
            Ok(u) => {
                let mut out = Vec::new();

                for row in u {
                    let row = self.base.textify_row(row, Vec::new()).0;
                    out.push(UserFollow {
                        user: row.get("user").unwrap().to_string(),
                        following: row.get("following").unwrap().to_string(),
                    })
                }

                out
            }
            Err(_) => return Err(AuthError::Other),
        };

        // return
        Ok(res)
    }

    // SET
    /// Toggle the following status of `user` on `following` ([`UserFollow`])
    ///
    /// # Arguments:
    /// * `props` - [`UserFollow`]
    pub async fn toggle_user_follow(&self, props: &mut UserFollow) -> Result<()> {
        // users cannot be the same
        if props.user == props.following {
            return Err(AuthError::Other);
        }

        // make sure both users exist
        if let Err(e) = self.get_profile_by_username(props.user.to_owned()).await {
            return Err(e);
        };

        // make sure both users exist
        if let Err(e) = self
            .get_profile_by_username(props.following.to_owned())
            .await
        {
            return Err(e);
        };

        // check if follow exists
        if let Ok(_) = self
            .get_follow(props.user.to_owned(), props.following.to_owned())
            .await
        {
            // delete
            let query: String =
                if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
                    "DELETE FROM \"xfollows\" WHERE \"user\" = ? AND \"following\" = ?"
                } else {
                    "DELETE FROM \"xfollows\" WHERE \"user\" = $1 AND \"following\" = $2"
                }
                .to_string();

            let c = &self.base.db.client;
            match sqlquery(&query)
                .bind::<&String>(&props.user)
                .bind::<&String>(&props.following)
                .execute(c)
                .await
            {
                Ok(_) => {
                    return Ok(());
                }
                Err(_) => return Err(AuthError::Other),
            };
        }

        // return
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "INSERT INTO \"xfollows\" VALUES (?, ?)"
        } else {
            "INSERT INTO \"xfollows\" VALEUS ($1, $2)"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&props.user)
            .bind::<&String>(&props.following)
            .execute(c)
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(AuthError::Other),
        }
    }
}
