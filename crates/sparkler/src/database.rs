use crate::config::Config;
use crate::model::{QuestionCreate, QuestionResponse, ResponseCreate};
use crate::model::{DatabaseError, Question};

use xsu_dataman::utility;
use xsu_dataman::query as sqlquery;
use xsu_authman::model::{Permission, Profile};

pub type Result<T> = std::result::Result<T, DatabaseError>;

/// Database connector
#[derive(Clone)]
pub struct Database {
    pub base: xsu_dataman::StarterDatabase,
    pub auth: xsu_authman::Database,
    pub server_options: Config,
}

impl Database {
    pub async fn new(
        opts: xsu_dataman::DatabaseOpts,
        auth: xsu_authman::Database,
        server_options: Config,
    ) -> Self {
        Self {
            base: xsu_dataman::StarterDatabase::new(opts).await,
            auth,
            server_options,
        }
    }

    /// Init database
    pub async fn init(&self) {
        // create tables
        let c = &self.base.db.client;

        // create questions table
        // we're only going to store unanswered questions here
        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"xquestions\" (
                author    TEXT,
                recipient TEXT,
                content   TEXT,
                id        TEXT,
                timestamp TEXT
            )",
        )
        .execute(c)
        .await;

        // create responses table
        let _ = sqlquery(
            "CREATE TABLE IF NOT EXISTS \"xresponses\" (
                author    TEXT,
                question  TEXT,
                content   TEXT,
                id        TEXT,
                timestamp TEXT
            )",
        )
        .execute(c)
        .await;
    }

    // ...

    // questions

    /// Get an existing question
    ///
    /// ## Arguments:
    /// * `id`
    pub async fn get_question(&self, id: String) -> Result<Question> {
        // check in cache
        match self
            .base
            .cachedb
            .get(format!("xsulib.sparkler.question:{}", id))
            .await
        {
            Some(c) => return Ok(serde_json::from_str::<Question>(c.as_str()).unwrap()),
            None => (),
        };

        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"xquestions\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"xquestions\" WHERE \"id\" = $1"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query).bind::<&String>(&id).fetch_one(c).await {
            Ok(p) => self.base.textify_row(p, Vec::new()).0,
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        let question = Question {
            author: res.get("author").unwrap().to_string(),
            recipient: res.get("recipient").unwrap().to_string(),
            content: res.get("content").unwrap().to_string(),
            id: res.get("id").unwrap().to_string(),
            timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
        };

        // store in cache
        self.base
            .cachedb
            .set(
                format!("xsulib.sparkler.question:{}", id),
                serde_json::to_string::<Question>(&question).unwrap(),
            )
            .await;

        // return
        Ok(question)
    }

    /// Get all questions by their recipient
    ///
    /// ## Arguments:
    /// * `recipient`
    pub async fn get_questions_by_recipient(&self, recipient: String) -> Result<Vec<Question>> {
        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"xquestions\" WHERE \"recipient\" = ? ORDER BY \"timestamp\" DESC"
        } else {
            "SELECT * FROM \"xquestions\" WHERE \"recipient\" = $1 ORDER BY \"timestamp\" DESC"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&recipient.to_lowercase())
            .fetch_all(c)
            .await
        {
            Ok(p) => {
                let mut out: Vec<Question> = Vec::new();

                for row in p {
                    let res = self.base.textify_row(row, Vec::new()).0;
                    out.push(Question {
                        author: res.get("author").unwrap().to_string(),
                        recipient: res.get("recipient").unwrap().to_string(),
                        content: res.get("content").unwrap().to_string(),
                        id: res.get("id").unwrap().to_string(),
                        timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
                    });
                }

                out
            }
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        Ok(res)
    }

    /// Create a new question
    ///
    /// ## Arguments:
    /// * `props` - [`QuestionCreate`]
    /// * `author` - the username of the user creating the question
    pub async fn create_question(&self, props: QuestionCreate, author: String) -> Result<()> {
        // check content length
        if props.content.len() > 250 {
            return Err(DatabaseError::ValueError);
        }

        // check recipient
        let recipient = match self
            .auth
            .get_profile_by_username(props.recipient.clone())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Err(DatabaseError::NotFound),
        };

        let profile_locked = recipient
            .metadata
            .kv
            .get("sparkler:lock_profile")
            .unwrap_or(&"false".to_string())
            == "true";

        let block_anonymous = recipient
            .metadata
            .kv
            .get("sparkler:disallow_anonymous")
            .unwrap_or(&"false".to_string())
            == "true";

        if profile_locked {
            return Err(DatabaseError::NotAllowed);
        }

        if (block_anonymous == true) && author == "anonymous" {
            return Err(DatabaseError::NotAllowed);
        }

        // ...
        let question = Question {
            author,
            recipient: props.recipient,
            content: props.content,
            id: utility::random_id(),
            timestamp: utility::unix_epoch_timestamp(),
        };

        // create document
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "INSERT INTO \"xquestions\" VALUES (?, ?, ?, ?, ?)"
        } else {
            "INSERT INTO \"xquestions\" VALEUS ($1, $2, $3, $4, $5)"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&question.author)
            .bind::<&String>(&question.recipient)
            .bind::<&String>(&question.content)
            .bind::<&String>(&question.id)
            .bind::<&String>(&question.timestamp.to_string())
            .execute(c)
            .await
        {
            Ok(_) => return Ok(()),
            Err(_) => return Err(DatabaseError::Other),
        };
    }

    /// Delete an existing question
    ///
    /// Questions can only be deleted by their recipient.
    ///
    /// ## Arguments:
    /// * `id` - the ID of the question
    /// * `user` - the user doing this
    pub async fn delete_question(&self, id: String, user: Profile) -> Result<()> {
        // make sure question exists
        let question = match self.get_question(id.clone()).await {
            Ok(q) => q,
            Err(e) => return Err(e),
        };

        // check username
        if user.username != question.recipient {
            // check permission
            let group = match self.auth.get_group_by_id(user.group).await {
                Ok(g) => g,
                Err(_) => return Err(DatabaseError::Other),
            };

            if !group.permissions.contains(&Permission::Manager) {
                return Err(DatabaseError::NotAllowed);
            }
        }

        // delete question
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "DELETE FROM \"xquestions\" WHERE \"id\" = ?"
        } else {
            "DELETE FROM \"xquestions\" WHERE \"id\" = $1"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query).bind::<&String>(&id).execute(c).await {
            Ok(_) => {
                // remove from cache
                self.base
                    .cachedb
                    .remove(format!("xsulib.sparkler.question:{}", id))
                    .await;

                // return
                return Ok(());
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }

    // responses

    /// Get an existing response
    ///
    /// ## Arguments:
    /// * `id`
    pub async fn get_response(&self, id: String) -> Result<QuestionResponse> {
        // check in cache
        match self
            .base
            .cachedb
            .get(format!("xsulib.sparkler.response:{}", id))
            .await
        {
            Some(c) => return Ok(serde_json::from_str::<QuestionResponse>(c.as_str()).unwrap()),
            None => (),
        };

        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"xresponses\" WHERE \"id\" = ?"
        } else {
            "SELECT * FROM \"xresponses\" WHERE \"id\" = $1"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query).bind::<&String>(&id).fetch_one(c).await {
            Ok(p) => self.base.textify_row(p, Vec::new()).0,
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        let response = QuestionResponse {
            author: res.get("author").unwrap().to_string(),
            question: match serde_json::from_str(res.get("question").unwrap()) {
                Ok(q) => q,
                Err(_) => return Err(DatabaseError::ValueError),
            },
            content: res.get("content").unwrap().to_string(),
            id: res.get("id").unwrap().to_string(),
            timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
        };

        // store in cache
        self.base
            .cachedb
            .set(
                format!("xsulib.sparkler.response:{}", id),
                serde_json::to_string::<QuestionResponse>(&response).unwrap(),
            )
            .await;

        // return
        Ok(response)
    }

    /// Get 25 responses by their author
    ///
    /// ## Arguments:
    /// * `author`
    pub async fn get_responses_by_author(&self, author: String) -> Result<Vec<QuestionResponse>> {
        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "SELECT * FROM \"xresponses\" WHERE \"author\" = ? ORDER BY \"timestamp\" DESC"
        } else {
            "SELECT * FROM \"xresponses\" WHERE \"author\" = $1 ORDER BY \"timestamp\" DESC"
        }
        .to_string();

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&author.to_lowercase())
            .fetch_all(c)
            .await
        {
            Ok(p) => {
                let mut out: Vec<QuestionResponse> = Vec::new();

                for row in p {
                    let res = self.base.textify_row(row, Vec::new()).0;
                    out.push(QuestionResponse {
                        author: res.get("author").unwrap().to_string(),
                        question: match serde_json::from_str(res.get("question").unwrap()) {
                            Ok(q) => q,
                            Err(_) => return Err(DatabaseError::ValueError),
                        },
                        content: res.get("content").unwrap().to_string(),
                        id: res.get("id").unwrap().to_string(),
                        timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
                    });
                }

                out
            }
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        Ok(res)
    }

    /// Get 25 responses from people `user` is following
    ///
    /// ## Arguments:
    /// * `user`
    pub async fn get_responses_by_following(&self, user: String) -> Result<Vec<QuestionResponse>> {
        // get following
        let following = match self.auth.get_following(user.clone()).await {
            Ok(f) => f,
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // build string
        let mut query_string = String::new();

        for follow in following {
            query_string.push_str(&format!(" OR \"author\" = '{}'", follow.following));
        }

        // pull from database
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            // we're also going to include our own responses so we don't have to do any complicated stuff to detect if we should start with "OR" (previous)
            format!("SELECT * FROM \"xresponses\" WHERE \"author\" = ?{query_string} ORDER BY \"timestamp\" DESC")
        } else {
            format!( "SELECT * FROM \"xresponses\" WHERE \"author\" = $1{query_string} ORDER BY \"timestamp\" DESC")
        };

        let c = &self.base.db.client;
        let res = match sqlquery(&query)
            .bind::<&String>(&user.to_lowercase())
            .fetch_all(c)
            .await
        {
            Ok(p) => {
                let mut out: Vec<QuestionResponse> = Vec::new();

                for row in p {
                    let res = self.base.textify_row(row, Vec::new()).0;
                    out.push(QuestionResponse {
                        author: res.get("author").unwrap().to_string(),
                        question: match serde_json::from_str(res.get("question").unwrap()) {
                            Ok(q) => q,
                            Err(_) => return Err(DatabaseError::ValueError),
                        },
                        content: res.get("content").unwrap().to_string(),
                        id: res.get("id").unwrap().to_string(),
                        timestamp: res.get("timestamp").unwrap().parse::<u128>().unwrap(),
                    });
                }

                out
            }
            Err(_) => return Err(DatabaseError::NotFound),
        };

        // return
        Ok(res)
    }

    /// Create a new response
    ///
    /// Responses can only be created for questions where `recipient` matches the given `author`
    ///
    /// ## Arguments:
    /// * `props` - [`ResponseCreate`]
    /// * `author` - the username of the user creating the response
    pub async fn create_response(&self, props: ResponseCreate, author: String) -> Result<()> {
        // make sure the question exists
        let question = match self.get_question(props.question.clone()).await {
            Ok(q) => q,
            Err(e) => return Err(e),
        };

        if question.recipient != author {
            // cannot respond to a question not asked to us
            return Err(DatabaseError::NotAllowed);
        }

        // check content length
        if props.content.len() > 500 {
            return Err(DatabaseError::ValueError);
        }

        // ...
        let response = QuestionResponse {
            author,
            question: question.clone(),
            content: props.content,
            id: utility::random_id(),
            timestamp: utility::unix_epoch_timestamp(),
        };

        // create response
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "INSERT INTO \"xresponses\" VALUES (?, ?, ?, ?, ?)"
        } else {
            "INSERT INTO \"xresponses\" VALEUS ($1, $2, $3, $4, $5)"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query)
            .bind::<&String>(&response.author)
            .bind::<&String>(&match serde_json::to_string(&response.question) {
                Ok(s) => s,
                Err(_) => return Err(DatabaseError::ValueError),
            })
            .bind::<&String>(&response.content)
            .bind::<&String>(&response.id)
            .bind::<&String>(&response.timestamp.to_string())
            .execute(c)
            .await
        {
            Ok(_) => {
                // delete question
                let query: String =
                    if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql") {
                        "DELETE FROM \"xquestions\" WHERE \"id\" = ?"
                    } else {
                        "DELETE FROM \"xquestions\" WHERE \"id\" = $1"
                    }
                    .to_string();

                let c = &self.base.db.client;
                match sqlquery(&query)
                    .bind::<&String>(&props.question)
                    .execute(c)
                    .await
                {
                    Ok(_) => {
                        // remove from cache
                        self.base
                            .cachedb
                            .remove(format!("xsulib.sparkler.question:{}", props.question))
                            .await;

                        // return
                        return Ok(());
                    }
                    Err(_) => return Err(DatabaseError::Other),
                };
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }

    /// Delete an existing question
    ///
    /// Responses can only be deleted by their author.
    ///
    /// ## Arguments:
    /// * `id` - the ID of the response
    /// * `user` - the user doing this
    pub async fn delete_response(&self, id: String, user: Profile) -> Result<()> {
        // make sure response exists
        let response = match self.get_response(id.clone()).await {
            Ok(q) => q,
            Err(e) => return Err(e),
        };

        // check username
        if user.username != response.author {
            // check permission
            let group = match self.auth.get_group_by_id(user.group).await {
                Ok(g) => g,
                Err(_) => return Err(DatabaseError::Other),
            };

            if !group.permissions.contains(&Permission::Manager) {
                return Err(DatabaseError::NotAllowed);
            }
        }

        // delete question
        let query: String = if (self.base.db.r#type == "sqlite") | (self.base.db.r#type == "mysql")
        {
            "DELETE FROM \"xresponses\" WHERE \"id\" = ?"
        } else {
            "DELETE FROM \"xresponses\" WHERE \"id\" = $1"
        }
        .to_string();

        let c = &self.base.db.client;
        match sqlquery(&query).bind::<&String>(&id).execute(c).await {
            Ok(_) => {
                // remove from cache
                self.base
                    .cachedb
                    .remove(format!("xsulib.sparkler.response:{}", id))
                    .await;

                // return
                return Ok(());
            }
            Err(_) => return Err(DatabaseError::Other),
        };
    }
}
