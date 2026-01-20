use std::{
   error::Error,
   fmt::{
      Display,
      Formatter,
      Result as FmtResult,
   },
   path::Path,
   sync::Mutex,
};

use rusqlite::{
   Connection,
   params,
};

use crate::twitter::TwitterAuth;

#[derive(Debug)]
pub enum DbError {
   Sqlite(rusqlite::Error),
   Lock(String),
}

impl Display for DbError {
   fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
      match *self {
         Self::Sqlite(ref error) => write!(f, "SQLite error: {error}"),
         Self::Lock(ref error) => write!(f, "database lock error: {error}"),
      }
   }
}

impl Error for DbError {}

impl From<rusqlite::Error> for DbError {
   fn from(error: rusqlite::Error) -> Self {
      Self::Sqlite(error)
   }
}

#[derive(Debug, Clone)]
pub struct User {
   pub id:                    i64,
   pub twitter_user_id:       String,
   pub auth_token:            String,
   pub csrf_token:            String,
   pub up_endpoint:           String,
   pub last_notif_sort_index: Option<String>,
}

impl User {
   pub fn auth(&self) -> TwitterAuth {
      TwitterAuth {
         auth_token: self.auth_token.clone(),
         csrf_token: self.csrf_token.clone(),
      }
   }
}

pub struct Db {
   conn: Mutex<Connection>,
}

impl Db {
   pub fn open<P>(path: P) -> Result<Self, DbError>
   where
      P: AsRef<Path>,
   {
      let conn = Connection::open(path)?;
      let db = Self {
         conn: Mutex::new(conn),
      };
      db.init_schema()?;
      Ok(db)
   }

   fn init_schema(&self) -> Result<(), DbError> {
      let conn = self
         .conn
         .lock()
         .map_err(|error| DbError::Lock(error.to_string()))?;

      let result = conn.execute_batch(
         "
            -- Users registered for notifications
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                twitter_user_id TEXT UNIQUE NOT NULL,
                auth_token TEXT NOT NULL,
                csrf_token TEXT NOT NULL,
                up_endpoint TEXT NOT NULL,
                last_notif_sort_index TEXT,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_users_twitter_id ON users(twitter_user_id);
            ",
      );
      drop(conn);
      result?;

      Ok(())
   }

   pub fn register_user(
      &self,
      twitter_user_id: &str,
      auth_token: &str,
      csrf_token: &str,
      up_endpoint: &str,
   ) -> Result<i64, DbError> {
      let conn = self
         .conn
         .lock()
         .map_err(|error| DbError::Lock(error.to_string()))?;

      let result = conn.execute(
         "
            INSERT INTO users (twitter_user_id, auth_token, csrf_token, up_endpoint, updated_at)
            VALUES (?1, ?2, ?3, ?4, strftime('%s', 'now'))
            ON CONFLICT(twitter_user_id) DO UPDATE SET
                auth_token = excluded.auth_token,
                csrf_token = excluded.csrf_token,
                up_endpoint = excluded.up_endpoint,
                updated_at = strftime('%s', 'now')
            ",
         params![twitter_user_id, auth_token, csrf_token, up_endpoint],
      );
      result?;

      let id = conn.query_row(
         "SELECT id FROM users WHERE twitter_user_id = ?1",
         params![twitter_user_id],
         |row| row.get(0),
      )?;
      drop(conn);

      Ok(id)
   }

   pub fn unregister_user(&self, twitter_user_id: &str) -> Result<bool, DbError> {
      let conn = self
         .conn
         .lock()
         .map_err(|error| DbError::Lock(error.to_string()))?;

      let rows = conn.execute("DELETE FROM users WHERE twitter_user_id = ?1", params![
         twitter_user_id
      ])?;
      drop(conn);

      Ok(rows > 0)
   }

   pub fn get_all_users(&self) -> Result<Vec<User>, DbError> {
      let conn = self
         .conn
         .lock()
         .map_err(|error| DbError::Lock(error.to_string()))?;

      let mut stmt = conn.prepare(
         "
            SELECT id, twitter_user_id, auth_token, csrf_token, up_endpoint, last_notif_sort_index
            FROM users
            ",
      )?;

      let users = stmt
         .query_map([], |row| {
            Ok(User {
               id:                    row.get(0)?,
               twitter_user_id:       row.get(1)?,
               auth_token:            row.get(2)?,
               csrf_token:            row.get(3)?,
               up_endpoint:           row.get(4)?,
               last_notif_sort_index: row.get(5)?,
            })
         })?
         .collect::<Result<Vec<_>, _>>()?;
      drop(stmt);
      drop(conn);

      Ok(users)
   }

   pub fn update_last_notif(&self, user_id: i64, sort_index: &str) -> Result<(), DbError> {
      let conn = self
         .conn
         .lock()
         .map_err(|error| DbError::Lock(error.to_string()))?;

      let result = conn.execute(
         "
            UPDATE users
            SET last_notif_sort_index = ?1, updated_at = strftime('%s', 'now')
            WHERE id = ?2
            ",
         params![sort_index, user_id],
      );
      drop(conn);
      result?;

      Ok(())
   }

   pub fn user_count(&self) -> Result<i64, DbError> {
      let conn = self
         .conn
         .lock()
         .map_err(|error| DbError::Lock(error.to_string()))?;

      let count = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
      drop(conn);

      Ok(count)
   }
}
