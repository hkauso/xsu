use crate::model::{DocumentCreate, DatabaseError, Document, DocumentMetadata};

use dorsal::utility;
use dorsal::query as sqlquery;
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
    pub base: dorsal::StarterDatabase,
    pub auth: xsu_authman::Database,
    pub options: ServerOptions,
}

impl Database {
    pub async fn new(
        opts: dorsal::DatabaseOpts,
        opts1: ServerOptions,
        auth: xsu_authman::Database,
    ) -> Self {
        Self {
            base: dorsal::StarterDatabase::new(opts).await,
            auth,
            options: opts1,
        }
    }

    /// Init database
    pub async fn init(&self) {
        // create tables
        let c = &self.base.db.client;

        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"xdocuments\" (
                 path TEXT,
                 owner TEXT,
                 content TEXT,
                 date_published TEXT,
                 date_edited TEXT,
                 metadata TEXT
            )",
        )
        .execute(c)
        .await;
    }

    /// Pull [`dorsal::DatabaseOpts`] from env
    pub fn env_options() -> dorsal::DatabaseOpts {
        use std::env::var;
        dorsal::DatabaseOpts {
            _type: match var("DB_TYPE") {
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

    /// Get an existing document
    ///
    /// ## Arguments:
    /// * `path`
    /// * `owner`
    pub async fn get_document(&self, path: String, owner: String) -> Result<Document> {
        // check in cache
        match self
            .base
            .cachedb
            .get(format!("xsulib.docshare:{}:{}", owner, path))
            .await
        {
            Some(c) => return Ok(serde_json::from_str::<Document>(c.as_str()).unwrap()),
            None => (),
        };

        // pull from database
        let query: String = if (self.base.db._type == "sqlite") | (self.base.db._type == "mysql") {
            "SELECT * FROM \"xdocuments\" WHERE \"path\" = ? AND \"owner\" = ?"
        } else {
            "SELECT * FROM \"xdocuments\" WHERE \"path\" = $1 AND \"owner\" = $2"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&path.to_lowercase())
            .bind::<&String>(&owner.to_lowercase())
            .fetch_one(c)
            .await
        {
            Ok(p) => self.base.textify_row(p).data,
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        let doc = Document {
            path: res.get("path").unwrap().to_string(),
            owner: res.get("owner").unwrap().to_string(),
            content: res.get("content").unwrap().to_string(),
            date_published: res.get("date_published").unwrap().parse::<u128>().unwrap(),
            date_edited: res.get("date_edited").unwrap().parse::<u128>().unwrap(),
            metadata: match serde_json::from_str(res.get("metadata").unwrap()) {
                Ok(m) => m,
                Err(_) => return Err(DatabaseError::ValueError),
            },
        };

        // store in cache
        self.base
            .cachedb
            .set(
                format!("xsulib.docshare:{}:{}", owner, path),
                serde_json::to_string::<Document>(&doc).unwrap(),
            )
            .await;

        // return
        Ok(doc)
    }

    /// Get an existing document
    ///
    /// ## Arguments:
    /// * `owner`
    pub async fn get_documents_by_owner(&self, owner: String) -> Result<Vec<Document>> {
        // pull from database
        let query: String = if (self.base.db._type == "sqlite") | (self.base.db._type == "mysql") {
            "SELECT * FROM \"xdocuments\" WHERE \"owner\" = ?"
        } else {
            "SELECT * FROM \"xdocuments\" WHERE \"owner\" = $2"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&owner.to_lowercase())
            .fetch_all(c)
            .await
        {
            Ok(p) => {
                let mut out: Vec<Document> = Vec::new();

                for row in p {
                    let res = self.base.textify_row(row).data;
                    out.push(Document {
                        path: res.get("path").unwrap().to_string(),
                        owner: res.get("owner").unwrap().to_string(),
                        content: res.get("content").unwrap().to_string(),
                        date_published: res.get("date_published").unwrap().parse::<u128>().unwrap(),
                        date_edited: res.get("date_edited").unwrap().parse::<u128>().unwrap(),
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

    /// Create a new document
    ///
    /// ## Arguments:
    /// * `props` - [`DocumentCreate`]
    pub async fn create_document(&self, mut props: DocumentCreate, owner: String) -> Result<()> {
        // make sure document doesn't already exist
        if let Ok(_) = self.get_document(props.path.clone(), owner.clone()).await {
            return Err(DatabaseError::AlreadyExists);
        }

        // create url if not supplied
        if props.path.is_empty() {
            props.path = utility::random_id().chars().take(10).collect();
        }

        // check lengths
        if (props.path.len() > 250) | (props.path.len() < 3) {
            return Err(DatabaseError::ValueError);
        }

        if (props.content.len() > 200_000) | (props.content.len() < 1) {
            return Err(DatabaseError::ValueError);
        }

        // (characters used)
        let regex = regex::RegexBuilder::new("^[\\w\\_\\-\\.\\\\/!]+$")
            .multi_line(true)
            .build()
            .unwrap();

        if regex.captures(&props.path).iter().len() < 1 {
            return Err(DatabaseError::ValueError);
        }

        // ...
        let doc = Document {
            path: props.path,
            owner,
            content: props.content,
            date_published: utility::unix_epoch_timestamp(),
            date_edited: utility::unix_epoch_timestamp(),
            metadata: DocumentMetadata::default(),
        };

        // create document
        let query: String = if (self.base.db._type == "sqlite") | (self.base.db._type == "mysql") {
            "INSERT INTO \"xdocuments\" VALUES (?, ?, ?, ?, ?, ?)"
        } else {
            "INSERT INTO \"xdocuments\" VALEUS ($1, $2, $3, $4, $5, $6)"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&doc.path)
            .bind::<&String>(&doc.owner)
            .bind::<&String>(&doc.content)
            .bind::<&String>(&doc.date_published.to_string())
            .bind::<&String>(&doc.date_edited.to_string())
            .bind::<&String>(match serde_json::to_string(&doc.metadata) {
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

    /// Delete an existing document
    ///
    /// ## Arguments:
    /// * `path
    /// * `owner`
    /// * `user` - the user doing this
    pub async fn delete_document(&self, path: String, owner: String, user: Profile) -> Result<()> {
        // make document exists
        if let Err(e) = self.get_document(path.clone(), owner.clone()).await {
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

        // delete document
        let query: String = if (self.base.db._type == "sqlite") | (self.base.db._type == "mysql") {
            "DELETE FROM \"xdocuments\" WHERE \"path\" = ? AND \"owner\" = ?"
        } else {
            "DELETE FROM \"xdocuments\" WHERE \"path\" = $1 AND \"owner\" = $2"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&path)
            .bind::<&String>(&owner)
            .execute(c)
            .await
        {
            Ok(_) => {
                // remove from cache
                self.base
                    .cachedb
                    .remove(format!("xsulib.docshare:{}:{}", owner, path))
                    .await;

                // return
                return Ok(());
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }

    /// Edit an existing document
    ///
    /// ## Arguments:
    /// * `path`
    /// * `owner`
    /// * `new_content` - the new content of the document
    /// * `new_path` - the new path of the document
    /// * `user` - the user doing this
    pub async fn edit_document(
        &self,
        path: String,
        owner: String,
        new_content: String,
        mut new_path: String,
        user: Profile,
    ) -> Result<()> {
        // make sure document exists
        let existing = match self.get_document(path.clone(), owner.clone()).await {
            Ok(p) => p,
            Err(err) => return Err(err),
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

        // update path
        if new_path.is_empty() {
            new_path = existing.path;
        }

        // edit document
        let query: String = if (self.base.db._type == "sqlite") | (self.base.db._type == "mysql") {
            "UPDATE \"xdocuments\" SET \"content\" = ?, \"path\" = ?, \"date_edited\" = ? WHERE \"path\" = ? AND \"owner\" = ?"
        } else {
            "UPDATE \"xdocuments\" SET (\"content\" = $1, \"path\" = $2, \"date_edited\" = $3) WHERE \"path\" = $4 AND \"owner\" = $5"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&new_content)
            .bind::<&String>(&new_path)
            .bind::<&String>(&utility::unix_epoch_timestamp().to_string())
            .bind::<&String>(&path)
            .bind::<&String>(&owner)
            .execute(c)
            .await
        {
            Ok(_) => {
                // remove from cache
                self.base
                    .cachedb
                    .remove(format!("xsulib.docshare:{}:{}", owner, path))
                    .await;

                // return
                return Ok(());
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }

    /// Edit an existing document's metadata
    ///
    /// ## Arguments:
    /// * `path`
    /// * `owner`
    /// * `password` - the document's edit password
    /// * `metadata` - the new metadata of the document
    /// * `user` - the user doing this
    pub async fn edit_document_metadata(
        &self,
        path: String,
        owner: String,
        metadata: DocumentMetadata,
        user: Profile,
    ) -> Result<()> {
        // make sure document exists
        if let Err(e) = self.get_document(path.clone(), owner.clone()).await {
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

        // edit document
        let query: String = if (self.base.db._type == "sqlite") | (self.base.db._type == "mysql") {
            "UPDATE \"xdocuments\" SET \"metadata\" = ? WHERE \"path\" = ? AND \"owner\" = ?"
        } else {
            "UPDATE \"xdocuments\" SET (\"metadata\" = $1) WHERE \"path\" = $2 AND \"owner\" = $3"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(match serde_json::to_string(&metadata) {
                Ok(ref m) => m,
                Err(_) => return Err(DatabaseError::ValueError),
            })
            .bind::<&String>(&path)
            .bind::<&String>(&owner)
            .execute(c)
            .await
        {
            Ok(_) => {
                // remove from cache
                self.base
                    .cachedb
                    .remove(format!("xsulib.docshare:{}:{}", owner, path))
                    .await;

                // return
                return Ok(());
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }
}
