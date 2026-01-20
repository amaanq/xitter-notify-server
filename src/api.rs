use std::sync::Arc;

use axum::{
   Json,
   Router,
   extract::{
      ConnectInfo,
      Query,
      State,
   },
   http::StatusCode,
   response::IntoResponse,
   routing::{
      delete,
      get,
      post,
   },
};
use serde::{
   Deserialize,
   Serialize,
};

use crate::{
   db::Db,
   rate_limit::RateLimiters,
   txid::TxIdGenerator,
};

pub struct AppState {
   pub db:             Arc<Db>,
   pub rate_limiters:  Arc<RateLimiters>,
   pub txid_generator: Arc<TxIdGenerator>,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
   twitter_user_id: String,
   auth_token:      String,
   csrf_token:      String,
   up_endpoint:     String,
}

#[derive(Deserialize)]
pub struct UnregisterRequest {
   twitter_user_id: String,
}

#[derive(Deserialize)]
pub struct TxIdQuery {
   path:  String,
   #[serde(default)]
   force: bool,
}

#[derive(Serialize)]
pub struct TxIdResponse {
   #[serde(rename = "x-client-transaction-id")]
   txid: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
   status: &'static str,
   #[serde(skip_serializing_if = "Option::is_none")]
   users:  Option<i64>,
   #[serde(skip_serializing_if = "Option::is_none")]
   error:  Option<String>,
}

impl StatusResponse {
   fn ok() -> Self {
      Self {
         status: "ok",
         users:  None,
         error:  None,
      }
   }

   fn ok_with_users(users: i64) -> Self {
      Self {
         status: "ok",
         users:  Some(users),
         error:  None,
      }
   }

   fn error(msg: impl Into<String>) -> Self {
      Self {
         status: "error",
         users:  None,
         error:  Some(msg.into()),
      }
   }
}

pub fn router(state: Arc<AppState>) -> Router {
   Router::new()
      .route("/register", post(register))
      .route("/unregister", delete(unregister))
      .route("/health", get(health))
      .route("/txid", get(generate_txid))
      .with_state(state)
}

async fn register(
   State(state): State<Arc<AppState>>,
   ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
   Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
   let ip = addr.ip();

   // Check rate limit
   if !state.rate_limiters.register.check(ip) {
      return (
         StatusCode::TOO_MANY_REQUESTS,
         Json(StatusResponse::error("Rate limit exceeded")),
      );
   }

   // Validate inputs
   if req.twitter_user_id.is_empty() {
      return (
         StatusCode::BAD_REQUEST,
         Json(StatusResponse::error("twitter_user_id is required")),
      );
   }
   if req.auth_token.is_empty() {
      return (
         StatusCode::BAD_REQUEST,
         Json(StatusResponse::error("auth_token is required")),
      );
   }
   if req.csrf_token.is_empty() {
      return (
         StatusCode::BAD_REQUEST,
         Json(StatusResponse::error("csrf_token is required")),
      );
   }
   if req.up_endpoint.is_empty() {
      return (
         StatusCode::BAD_REQUEST,
         Json(StatusResponse::error("up_endpoint is required")),
      );
   }

   // Validate UP endpoint URL
   if !req.up_endpoint.starts_with("https://") && !req.up_endpoint.starts_with("http://") {
      return (
         StatusCode::BAD_REQUEST,
         Json(StatusResponse::error("up_endpoint must be a valid URL")),
      );
   }

   match state.db.register_user(
      &req.twitter_user_id,
      &req.auth_token,
      &req.csrf_token,
      &req.up_endpoint,
   ) {
      Ok(_) => {
         eprintln!("[api] Registered user {}", req.twitter_user_id);
         (StatusCode::OK, Json(StatusResponse::ok()))
      },
      Err(e) => {
         eprintln!("[api] Failed to register user: {e}");
         (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error("Failed to register")),
         )
      },
   }
}

async fn unregister(
   State(state): State<Arc<AppState>>,
   ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
   Json(req): Json<UnregisterRequest>,
) -> impl IntoResponse {
   let ip = addr.ip();

   // Check rate limit
   if !state.rate_limiters.unregister.check(ip) {
      return (
         StatusCode::TOO_MANY_REQUESTS,
         Json(StatusResponse::error("Rate limit exceeded")),
      );
   }

   if req.twitter_user_id.is_empty() {
      return (
         StatusCode::BAD_REQUEST,
         Json(StatusResponse::error("twitter_user_id is required")),
      );
   }

   match state.db.unregister_user(&req.twitter_user_id) {
      Ok(deleted) => {
         if deleted {
            eprintln!("[api] Unregistered user {}", req.twitter_user_id);
            (StatusCode::OK, Json(StatusResponse::ok()))
         } else {
            (
               StatusCode::NOT_FOUND,
               Json(StatusResponse::error("User not found")),
            )
         }
      },
      Err(e) => {
         eprintln!("[api] Failed to unregister user: {e}");
         (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error("Failed to unregister")),
         )
      },
   }
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
   match state.db.user_count() {
      Ok(count) => (StatusCode::OK, Json(StatusResponse::ok_with_users(count))),
      Err(e) => {
         eprintln!("[api] Health check failed: {e}");
         (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error("Database error")),
         )
      },
   }
}

async fn generate_txid(
   State(state): State<Arc<AppState>>,
   Query(query): Query<TxIdQuery>,
) -> impl IntoResponse {
   if query.path.is_empty() {
      return (
         StatusCode::BAD_REQUEST,
         Json(StatusResponse::error("path is required")),
      )
         .into_response();
   }

   // Force refresh if requested (e.g., after 403/404 from Twitter)
   if query.force
      && let Err(e) = state.txid_generator.invalidate_and_refresh().await
   {
      eprintln!("[api] Failed to force refresh: {e}");
   }

   match state.txid_generator.generate("GET", &query.path).await {
      Ok(txid) => (StatusCode::OK, Json(TxIdResponse { txid })).into_response(),
      Err(e) => {
         eprintln!("[api] Failed to generate transaction ID: {e}");
         (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error(format!("Failed to generate: {e}"))),
         )
            .into_response()
      },
   }
}
