use std::{
   net::SocketAddr,
   path::PathBuf,
};

pub struct Config {
   pub db_path:            PathBuf,
   pub listen_addr:        SocketAddr,
   pub poll_interval_secs: u64,
   pub max_concurrent:     usize,
}

impl Config {
   pub fn from_env() -> Self {
      let db_path = std::env::var("XITTER_NOTIFY_DB_PATH")
         .map(PathBuf::from)
         .unwrap_or_else(|_| PathBuf::from("./xitter-notify-server.db"));

      let listen_addr = std::env::var("XITTER_NOTIFY_LISTEN_ADDR")
         .ok()
         .and_then(|s| s.parse().ok())
         .unwrap_or_else(|| "127.0.0.1:3000".parse().unwrap());

      let poll_interval_secs = std::env::var("XITTER_NOTIFY_POLL_INTERVAL")
         .ok()
         .and_then(|s| s.parse().ok())
         .unwrap_or(15);

      let max_concurrent = std::env::var("XITTER_NOTIFY_MAX_CONCURRENT")
         .ok()
         .and_then(|s| s.parse().ok())
         .unwrap_or(50);

      Self {
         db_path,
         listen_addr,
         poll_interval_secs,
         max_concurrent,
      }
   }
}

impl Default for Config {
   fn default() -> Self {
      Self::from_env()
   }
}
