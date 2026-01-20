use std::{
   sync::RwLock,
   time::{
      Duration,
      Instant,
   },
};

use xitter_txid::ClientTransaction;

use crate::http_client::HttpClient;

const REFRESH_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60); // 12 hours

pub struct TxIdGenerator {
   client: HttpClient,
   state:  RwLock<Option<CachedState>>,
}

struct CachedState {
   transaction: ClientTransaction,
   fetched_at:  Instant,
}

impl TxIdGenerator {
   pub fn new(client: HttpClient) -> Self {
      Self {
         client,
         state: RwLock::new(None),
      }
   }

   pub async fn generate(&self, method: &str, path: &str) -> Result<String, TxIdError> {
      // Check if we have a valid cached state
      {
         let state = self.state.read().unwrap();
         if let Some(ref cached) = *state
            && cached.fetched_at.elapsed() < REFRESH_INTERVAL
         {
            return Ok(cached.transaction.generate_transaction_id(method, path));
         }
      }

      // Need to refresh
      self.refresh().await?;

      let state = self.state.read().unwrap();
      let cached = state.as_ref().ok_or(TxIdError::NotInitialized)?;
      Ok(cached.transaction.generate_transaction_id(method, path))
   }

   async fn refresh(&self) -> Result<(), TxIdError> {
      // Fetch homepage
      let html = self
         .client
         .get_text("https://x.com")
         .await
         .map_err(|e| TxIdError::Fetch(format!("Failed to fetch homepage: {e}")))?;

      // Extract ondemand.s URL
      let js_url = ClientTransaction::extract_ondemand_url(&html)
         .map_err(|e| TxIdError::Parse(format!("Failed to extract JS URL: {e}")))?;

      // Fetch JS file
      let js = self
         .client
         .get_text(&js_url)
         .await
         .map_err(|e| TxIdError::Fetch(format!("Failed to fetch JS: {e}")))?;

      // Create ClientTransaction
      let transaction = ClientTransaction::new(&html, &js)
         .map_err(|e| TxIdError::Parse(format!("Failed to parse: {e}")))?;

      // Cache it
      {
         let mut state = self.state.write().unwrap();
         *state = Some(CachedState {
            transaction,
            fetched_at: Instant::now(),
         });
      }

      eprintln!("[txid] Refreshed transaction ID keys");
      Ok(())
   }

   /// Force refresh the cached keys (e.g., on 403/404)
   pub async fn invalidate_and_refresh(&self) -> Result<(), TxIdError> {
      {
         let mut state = self.state.write().unwrap();
         *state = None;
      }
      self.refresh().await
   }
}

#[derive(Debug)]
pub enum TxIdError {
   Fetch(String),
   Parse(String),
   NotInitialized,
}

impl std::fmt::Display for TxIdError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         TxIdError::Fetch(e) => write!(f, "Fetch error: {e}"),
         TxIdError::Parse(e) => write!(f, "Parse error: {e}"),
         TxIdError::NotInitialized => write!(f, "Not initialized"),
      }
   }
}

impl std::error::Error for TxIdError {}
