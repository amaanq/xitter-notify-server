use serde::Serialize;

use crate::{
   http_client::{
      HttpClient,
      HttpError,
   },
   twitter::Notification,
};

#[derive(Debug)]
pub enum UpError {
   Http(HttpError),
   Serialize(String),
}

impl std::fmt::Display for UpError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         UpError::Http(e) => write!(f, "HTTP error: {e}"),
         UpError::Serialize(e) => write!(f, "Serialize error: {e}"),
      }
   }
}

impl std::error::Error for UpError {}

impl From<HttpError> for UpError {
   fn from(e: HttpError) -> Self {
      UpError::Http(e)
   }
}

#[derive(Serialize)]
struct UpPayload {
   title:    String,
   message:  String,
   priority: u8,
   data:     UpData,
}

#[derive(Serialize)]
struct UpData {
   url:               Option<String>,
   notification_type: String,
   sort_index:        String,
}

pub async fn send(
   client: &HttpClient,
   endpoint: &str,
   notif: &Notification,
) -> Result<(), UpError> {
   let payload = UpPayload {
      title:    notif.title(),
      message:  notif.body().to_string(),
      priority: 3,
      data:     UpData {
         url:               notif.url.clone(),
         notification_type: notif.notification_type.clone(),
         sort_index:        notif.sort_index.clone(),
      },
   };

   let body = serde_json::to_vec(&payload).map_err(|e| UpError::Serialize(e.to_string()))?;

   let headers = [("Content-Type", "application/json")];

   client.post(endpoint, &headers, &body).await?;

   Ok(())
}
