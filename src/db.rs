use std::{
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
}

impl std::fmt::Display for DbError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         DbError::Sqlite(e) => write!(f, "SQLite error: {e}"),
      }
   }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
   fn from(e: rusqlite::Error) -> Self {
      DbError::Sqlite(e)
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
   pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DbError> {
      let conn = Connection::open(path)?;
      let db = Db {
         conn: Mutex::new(conn),
      };
      db.init_schema()?;
      Ok(db)
   }

   fn init_schema(&self) -> Result<(), DbError> {
      let conn = self.conn.lock().unwrap();

      conn.execute_batch(
         r#"
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
            "#,
      )?;

      Ok(())
   }

   pub fn register_user(
      &self,
      twitter_user_id: &str,
      auth_token: &str,
      csrf_token: &str,
      up_endpoint: &str,
   ) -> Result<i64, DbError> {
      let conn = self.conn.lock().unwrap();

      // Upsert: insert or update if exists
      conn.execute(
         r#"
            INSERT INTO users (twitter_user_id, auth_token, csrf_token, up_endpoint, updated_at)
            VALUES (?1, ?2, ?3, ?4, strftime('%s', 'now'))
            ON CONFLICT(twitter_user_id) DO UPDATE SET
                auth_token = excluded.auth_token,
                csrf_token = excluded.csrf_token,
                up_endpoint = excluded.up_endpoint,
                updated_at = strftime('%s', 'now')
            "#,
         params![twitter_user_id, auth_token, csrf_token, up_endpoint],
      )?;

      // Get the user ID
      let id: i64 = conn.query_row(
         "SELECT id FROM users WHERE twitter_user_id = ?1",
         params![twitter_user_id],
         |row| row.get(0),
      )?;

      Ok(id)
   }

   pub fn unregister_user(&self, twitter_user_id: &str) -> Result<bool, DbError> {
      let conn = self.conn.lock().unwrap();

      let rows = conn.execute("DELETE FROM users WHERE twitter_user_id = ?1", params![
         twitter_user_id
      ])?;

      Ok(rows > 0)
   }

   pub fn get_all_users(&self) -> Result<Vec<User>, DbError> {
      let conn = self.conn.lock().unwrap();

      let mut stmt = conn.prepare(
         r#"
            SELECT id, twitter_user_id, auth_token, csrf_token, up_endpoint, last_notif_sort_index
            FROM users
            "#,
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

      Ok(users)
   }

   pub fn update_last_notif(&self, user_id: i64, sort_index: &str) -> Result<(), DbError> {
      let conn = self.conn.lock().unwrap();

      conn.execute(
         r#"
            UPDATE users
            SET last_notif_sort_index = ?1, updated_at = strftime('%s', 'now')
            WHERE id = ?2
            "#,
         params![sort_index, user_id],
      )?;

      Ok(())
   }

   pub fn user_count(&self) -> Result<i64, DbError> {
      let conn = self.conn.lock().unwrap();

      let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;

      Ok(count)
   }
}
