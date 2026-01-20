mod api;
mod config;
mod db;
mod http_client;
mod poller;
mod rate_limit;
mod twitter;
mod txid;
mod unified_push;

use std::{
   sync::Arc,
   time::Duration,
};

use api::AppState;
use config::Config;
use db::Db;
use http_client::HttpClient;
use rate_limit::RateLimiters;
use tokio::net::TcpListener;
use txid::TxIdGenerator;

#[tokio::main]
async fn main() {
   let config = Arc::new(Config::from_env());

   eprintln!("Xitter Notification Server");
   eprintln!("  Database: {:?}", config.db_path);
   eprintln!("  Listen: {}", config.listen_addr);
   eprintln!("  Poll interval: {}s", config.poll_interval_secs);
   eprintln!("  Max concurrent: {}", config.max_concurrent);

   // Initialize database
   let db = match Db::open(&config.db_path) {
      Ok(db) => Arc::new(db),
      Err(e) => {
         eprintln!("Failed to open database: {e}");
         std::process::exit(1);
      },
   };

   // Initialize HTTP client
   let client = Arc::new(HttpClient::new());

   // Initialize rate limiters
   let rate_limiters = Arc::new(RateLimiters::new());

   // Initialize transaction ID generator
   let txid_generator = Arc::new(TxIdGenerator::new(HttpClient::new()));

   // Create app state for API
   let app_state = Arc::new(AppState {
      db: db.clone(),
      rate_limiters: rate_limiters.clone(),
      txid_generator,
   });

   // Start the poller in a background task
   let poller_db = db.clone();
   let poller_client = client.clone();
   let poller_config = config.clone();
   tokio::spawn(async move {
      poller::run_poller(poller_db, poller_client, poller_config).await;
   });

   // Start rate limiter cleanup task
   let cleanup_limiters = rate_limiters.clone();
   tokio::spawn(async move {
      let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
      loop {
         interval.tick().await;
         cleanup_limiters.cleanup();
      }
   });

   // Build the API router
   let app = api::router(app_state);

   // Start the server
   let listener = match TcpListener::bind(config.listen_addr).await {
      Ok(l) => l,
      Err(e) => {
         eprintln!("Failed to bind to {}: {e}", config.listen_addr);
         std::process::exit(1);
      },
   };

   eprintln!("Server listening on {}", config.listen_addr);

   if let Err(e) = axum::serve(
      listener,
      app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
   )
   .await
   {
      eprintln!("Server error: {e}");
      std::process::exit(1);
   }
}
