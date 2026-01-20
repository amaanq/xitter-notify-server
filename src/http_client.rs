use std::net::{
   IpAddr,
   Ipv4Addr,
};

use bytes::Bytes;
use http_body_util::{
   BodyExt,
   Full,
};
use hyper::{
   Method,
   Request,
   Response,
   StatusCode,
   body::Incoming,
};
use hyper_rustls::HttpsConnector;
use hyper_util::{
   client::legacy::{
      Client,
      connect::HttpConnector,
   },
   rt::TokioExecutor,
};

pub type HttpsClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

#[derive(Debug)]
pub enum HttpError {
   Request(String),
   Status(StatusCode, String),
   Body(String),
}

impl std::fmt::Display for HttpError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         HttpError::Request(e) => write!(f, "request error: {e}"),
         HttpError::Status(code, body) => write!(f, "HTTP {code}: {body}"),
         HttpError::Body(e) => write!(f, "body error: {e}"),
      }
   }
}

impl std::error::Error for HttpError {}

pub struct HttpClient {
   client: HttpsClient,
}

impl HttpClient {
   pub fn new() -> Self {
      let mut http = HttpConnector::new();
      http.enforce_http(false); // Allow HTTPS
      http.set_local_address(Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));

      let https = hyper_rustls::HttpsConnectorBuilder::new()
         .with_webpki_roots()
         .https_only()
         .enable_http1()
         .wrap_connector(http);

      let client: HttpsClient = Client::builder(TokioExecutor::new()).build(https);

      Self { client }
   }

   pub async fn get<H: AsRef<str>>(
      &self,
      url: &str,
      headers: &[(&str, H)],
   ) -> Result<Vec<u8>, HttpError> {
      let mut builder = Request::builder().method(Method::GET).uri(url);

      for (key, value) in headers {
         builder = builder.header(*key, value.as_ref());
      }

      let request = builder
         .body(Full::new(Bytes::new()))
         .map_err(|e| HttpError::Request(e.to_string()))?;

      let response = self
         .client
         .request(request)
         .await
         .map_err(|e| HttpError::Request(e.to_string()))?;

      self.handle_response(response).await
   }

   /// Simple GET without custom headers, returns text
   pub async fn get_text(&self, url: &str) -> Result<String, HttpError> {
      let headers: [(&str, &str); 1] = [(
         "user-agent",
         "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 \
          Safari/537.36",
      )];
      let body = self.get(url, &headers).await?;
      String::from_utf8(body).map_err(|e| HttpError::Body(e.to_string()))
   }

   pub async fn post<H: AsRef<str>>(
      &self,
      url: &str,
      headers: &[(&str, H)],
      body: &[u8],
   ) -> Result<Vec<u8>, HttpError> {
      let mut builder = Request::builder().method(Method::POST).uri(url);

      for (key, value) in headers {
         builder = builder.header(*key, value.as_ref());
      }

      let request = builder
         .body(Full::new(Bytes::from(body.to_vec())))
         .map_err(|e| HttpError::Request(e.to_string()))?;

      let response = self
         .client
         .request(request)
         .await
         .map_err(|e| HttpError::Request(e.to_string()))?;

      self.handle_response(response).await
   }

   async fn handle_response(&self, response: Response<Incoming>) -> Result<Vec<u8>, HttpError> {
      let status = response.status();
      let body = response
         .into_body()
         .collect()
         .await
         .map_err(|e| HttpError::Body(e.to_string()))?
         .to_bytes()
         .to_vec();

      if !status.is_success() {
         let body_str = String::from_utf8_lossy(&body).to_string();
         return Err(HttpError::Status(status, body_str));
      }

      Ok(body)
   }
}

impl Default for HttpClient {
   fn default() -> Self {
      Self::new()
   }
}
