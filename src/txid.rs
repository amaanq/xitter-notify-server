use std::{
   error::Error,
   fmt::{
      Display,
      Formatter,
      Result as FmtResult,
   },
   sync::RwLock,
   time::{
      Duration,
      Instant,
   },
};

use xitter_txid::ClientTransaction;

use crate::http_client::HttpClient;

const REFRESH_INTERVAL: Duration = Duration::from_hours(12); // 12 hours

pub struct TxIdGenerator {
   client: HttpClient,
   state:  RwLock<Option<CachedState>>,
}

struct CachedState {
   transaction: ClientTransaction,
   fetched_at:  Instant,
}

impl TxIdGenerator {
   pub const fn new(client: HttpClient) -> Self {
      Self {
         client,
         state: RwLock::new(None),
      }
   }

   pub async fn generate(&self, method: &str, path: &str) -> Result<String, TxIdError> {
      {
         let state = self.state.read().unwrap();
         if let Some(ref cached) = *state
            && cached.fetched_at.elapsed() < REFRESH_INTERVAL
         {
            let txid = cached.transaction.generate_transaction_id(method, path);
            drop(state);
            return Ok(txid);
         }
         drop(state);
      }

      self.refresh().await?;

      let txid = {
         let state = self.state.read().unwrap();
         let cached = state.as_ref().ok_or(TxIdError::NotInitialized)?;
         let txid = cached.transaction.generate_transaction_id(method, path);
         drop(state);
         txid
      };

      Ok(txid)
   }

   async fn refresh(&self) -> Result<(), TxIdError> {
      let html = self
         .client
         .get_text("https://x.com")
         .await
         .map_err(|error| TxIdError::Fetch(format!("Failed to fetch homepage: {error}")))?;

      let js_url = ClientTransaction::extract_ondemand_url(&html)
         .map_err(|error| TxIdError::Parse(format!("Failed to extract JS URL: {error}")))?;

      let js = self
         .client
         .get_text(&js_url)
         .await
         .map_err(|error| TxIdError::Fetch(format!("Failed to fetch JS: {error}")))?;

      let transaction = ClientTransaction::new(&html, &js)
         .map_err(|error| TxIdError::Parse(format!("Failed to parse: {error}")))?;

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

impl Display for TxIdError {
   fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
      match *self {
         Self::Fetch(ref error) => write!(f, "Fetch error: {error}"),
         Self::Parse(ref error) => write!(f, "Parse error: {error}"),
         Self::NotInitialized => write!(f, "Not initialized"),
      }
   }
}

impl Error for TxIdError {}
