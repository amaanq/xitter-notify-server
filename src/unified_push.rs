use std::{
   error::Error,
   fmt::{
      Display,
      Formatter,
      Result as FmtResult,
   },
};

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

impl Display for UpError {
   fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
      match *self {
         Self::Http(ref error) => write!(f, "HTTP error: {error}"),
         Self::Serialize(ref error) => write!(f, "Serialize error: {error}"),
      }
   }
}

impl Error for UpError {}

impl From<HttpError> for UpError {
   fn from(error: HttpError) -> Self {
      Self::Http(error)
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
      message:  notif.body().to_owned(),
      priority: 3,
      data:     UpData {
         url:               notif.url.clone(),
         notification_type: notif.kind.clone(),
         sort_index:        notif.sort_index.clone(),
      },
   };

   let body =
      serde_json::to_vec(&payload).map_err(|error| UpError::Serialize(error.to_string()))?;

   let headers = [("Content-Type", "application/json")];

   client.post(endpoint, &headers, &body).await?;

   Ok(())
}
