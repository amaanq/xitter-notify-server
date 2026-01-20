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
   net::SocketAddr,
   process::exit,
   sync::Arc,
   time::Duration,
};

use api::AppState;
use config::Config;
use db::Db;
use http_client::HttpClient;
use rate_limit::RateLimiters;
use tokio::{
   net::TcpListener,
   time::interval,
};
use txid::TxIdGenerator;

#[tokio::main]
async fn main() {
   let config = Arc::new(Config::from_env());

   eprintln!("Xitter Notification Server");
   eprintln!("  Database: {}", config.db_path.display());
   eprintln!("  Listen: {}", config.listen_addr);
   eprintln!("  Poll interval: {}s", config.poll_interval_secs);
   eprintln!("  Max concurrent: {}", config.max_concurrent);

   // Initialize database
   let db = match Db::open(&config.db_path) {
      Ok(db) => Arc::new(db),
      Err(error) => {
         eprintln!("Failed to open database: {error}");
         exit(1);
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
      db: Arc::clone(&db),
      rate_limiters: Arc::clone(&rate_limiters),
      txid_generator,
   });

   let poller_db = Arc::clone(&db);
   let poller_client = Arc::clone(&client);
   let poller_config = Arc::clone(&config);
   tokio::spawn(async move {
      poller::run_poller(poller_db, poller_client, poller_config).await;
   });

   let cleanup_limiters = Arc::clone(&rate_limiters);
   tokio::spawn(async move {
      let mut interval = interval(Duration::from_secs(300));
      loop {
         interval.tick().await;
         cleanup_limiters.cleanup();
      }
   });

   // Build the API router
   let app = api::router(app_state);

   // Start the server
   let listener = match TcpListener::bind(config.listen_addr).await {
      Ok(listener) => listener,
      Err(error) => {
         eprintln!("Failed to bind to {}: {error}", config.listen_addr);
         exit(1);
      },
   };

   eprintln!("Server listening on {}", config.listen_addr);

   if let Err(error) = axum::serve(
      listener,
      app.into_make_service_with_connect_info::<SocketAddr>(),
   )
   .await
   {
      eprintln!("Server error: {error}");
      exit(1);
   }
}
