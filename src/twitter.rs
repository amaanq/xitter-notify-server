use serde::{
   Deserialize,
   Serialize,
};

use crate::http_client::{
   HttpClient,
   HttpError,
};

const BEARER_TOKEN: &str = "Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%\
                            3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like \
                          Gecko) Chrome/123.0.0.0 Mobile Safari/537.3";

// GraphQL query ID for NotificationsTimeline - this may need periodic updates
const NOTIFICATIONS_QUERY_ID: &str = "Y-4nWuqrAwaEDpHtfJmK5A";

#[derive(Debug)]
pub enum TwitterError {
   Http(HttpError),
   Parse(String),
   Api(String),
}

impl std::fmt::Display for TwitterError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         TwitterError::Http(e) => write!(f, "HTTP error: {e}"),
         TwitterError::Parse(e) => write!(f, "Parse error: {e}"),
         TwitterError::Api(e) => write!(f, "API error: {e}"),
      }
   }
}

impl std::error::Error for TwitterError {}

impl From<HttpError> for TwitterError {
   fn from(e: HttpError) -> Self {
      TwitterError::Http(e)
   }
}

#[derive(Clone)]
pub struct TwitterAuth {
   pub auth_token: String,
   pub csrf_token: String,
}

impl TwitterAuth {
   pub fn headers(&self) -> Vec<(&'static str, String)> {
      vec![
         ("accept", "*/*".to_string()),
         ("accept-language", "en-US,en;q=0.9".to_string()),
         ("authorization", BEARER_TOKEN.to_string()),
         ("cache-control", "no-cache".to_string()),
         ("content-type", "application/json".to_string()),
         ("pragma", "no-cache".to_string()),
         ("priority", "u=1, i".to_string()),
         ("referer", "https://x.com/".to_string()),
         ("user-agent", USER_AGENT.to_string()),
         ("x-twitter-active-user", "yes".to_string()),
         ("x-twitter-client-language", "en".to_string()),
         ("x-csrf-token", self.csrf_token.clone()),
         (
            "cookie",
            format!("auth_token={}; ct0={}", self.auth_token, self.csrf_token),
         ),
      ]
   }
}

#[derive(Debug, Deserialize)]
pub struct BadgeCount {
   #[serde(default)]
   pub ntab_unread_count: i32,
   #[serde(default)]
   #[expect(unused, reason = "will use later")]
   pub dm_unread_count:   i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Notification {
   pub sort_index:        String,
   pub notification_type: String,
   pub message:           String,
   pub icon_url:          Option<String>,
   pub url:               Option<String>,
   pub from_users:        Vec<String>,
}

impl Notification {
   pub fn title(&self) -> String {
      match self.notification_type.as_str() {
         "like" => "New Like".to_string(),
         "retweet" => "New Repost".to_string(),
         "reply" => "New Reply".to_string(),
         "mention" => "New Mention".to_string(),
         "follow" => "New Follower".to_string(),
         "quote" => "New Quote".to_string(),
         _ => "New Notification".to_string(),
      }
   }

   pub fn body(&self) -> &str {
      &self.message
   }
}

/// Check the badge count for unread notifications
pub async fn get_badge_count(
   client: &HttpClient,
   auth: &TwitterAuth,
) -> Result<BadgeCount, TwitterError> {
   let url = "https://x.com/i/api/2/badge_count/badge_count.json?supports_ntab_urt=1";

   let headers = auth.headers();

   let body = client.get(url, &headers).await?;

   serde_json::from_slice(&body).map_err(|e| TwitterError::Parse(e.to_string()))
}

/// Fetch notifications timeline
pub async fn get_notifications(
   client: &HttpClient,
   auth: &TwitterAuth,
) -> Result<Vec<Notification>, TwitterError> {
   let variables = serde_json::json!({
       "count": 20,
       "includePromotedContent": false,
       "withCommunity": true,
       "withQuickPromoteEligibilityTweetFields": true,
       "withBirdwatchNotes": true,
       "withVoice": true,
       "withV2Timeline": true
   });

   let features = serde_json::json!({
       "rweb_tipjar_consumption_enabled": true,
       "responsive_web_graphql_exclude_directive_enabled": true,
       "verified_phone_label_enabled": false,
       "creator_subscriptions_tweet_preview_api_enabled": true,
       "responsive_web_graphql_timeline_navigation_enabled": true,
       "responsive_web_graphql_skip_user_profile_image_extensions_enabled": false,
       "communities_web_enable_tweet_community_results_fetch": true,
       "c9s_tweet_anatomy_moderator_badge_enabled": true,
       "articles_preview_enabled": true,
       "responsive_web_edit_tweet_api_enabled": true,
       "graphql_is_translatable_rweb_tweet_is_translatable_enabled": true,
       "view_counts_everywhere_api_enabled": true,
       "longform_notetweets_consumption_enabled": true,
       "responsive_web_twitter_article_tweet_consumption_enabled": true,
       "tweet_awards_web_tipping_enabled": false,
       "creator_subscriptions_quote_tweet_preview_enabled": false,
       "freedom_of_speech_not_reach_fetch_enabled": true,
       "standardized_nudges_misinfo": true,
       "tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled": true,
       "rweb_video_timestamps_enabled": true,
       "longform_notetweets_rich_text_read_enabled": true,
       "longform_notetweets_inline_media_enabled": true,
       "responsive_web_enhance_cards_enabled": false
   });

   let url = format!(
      "https://x.com/i/api/graphql/{NOTIFICATIONS_QUERY_ID}/NotificationsTimeline?variables={}&features={}",
      urlencoding(&variables.to_string()),
      urlencoding(&features.to_string())
   );

   let headers = auth.headers();

   let body = client.get(&url, &headers).await?;

   parse_notifications(&body)
}

fn urlencoding(s: &str) -> String {
   let mut result = String::with_capacity(s.len() * 3);
   for c in s.chars() {
      match c {
         'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
         _ => {
            for byte in c.to_string().as_bytes() {
               result.push('%');
               result.push_str(&format!("{:02X}", byte));
            }
         },
      }
   }
   result
}

fn parse_notifications(body: &[u8]) -> Result<Vec<Notification>, TwitterError> {
   let json: serde_json::Value =
      serde_json::from_slice(body).map_err(|e| TwitterError::Parse(e.to_string()))?;

   // Check for errors
   if let Some(errors) = json.get("errors")
      && let Some(first_error) = errors.as_array().and_then(|arr| arr.first())
   {
      let message = first_error
         .get("message")
         .and_then(|m| m.as_str())
         .unwrap_or("Unknown error");
      return Err(TwitterError::Api(message.to_string()));
   }

   let mut notifications = Vec::new();

   // Navigate to the instructions
   let instructions = json
      .pointer("/data/user/result/timeline/timeline/instructions")
      .and_then(|v| v.as_array());

   let Some(instructions) = instructions else {
      return Ok(notifications);
   };

   for instruction in instructions {
      if instruction.get("type").and_then(|t| t.as_str()) != Some("TimelineAddEntries") {
         continue;
      }

      let entries = instruction.get("entries").and_then(|e| e.as_array());
      let Some(entries) = entries else {
         continue;
      };

      for entry in entries {
         let entry_id = entry.get("entryId").and_then(|e| e.as_str()).unwrap_or("");

         // Skip cursors
         if entry_id.starts_with("cursor-") {
            continue;
         }

         let sort_index = entry
            .get("sortIndex")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();

         if sort_index.is_empty() {
            continue;
         }

         // Extract notification content
         let content = entry.get("content");
         let Some(content) = content else {
            continue;
         };

         if let Some(notif) = parse_notification_entry(content, &sort_index) {
            notifications.push(notif);
         }
      }
   }

   // Sort by sort_index descending (newest first)
   notifications.sort_by(|a, b| b.sort_index.cmp(&a.sort_index));

   Ok(notifications)
}

fn parse_notification_entry(content: &serde_json::Value, sort_index: &str) -> Option<Notification> {
   // Try to get the notification from itemContent
   let item_content = content.get("itemContent")?;

   let notification_type = item_content
      .get("notificationType")
      .and_then(|t| t.as_str())
      .unwrap_or("unknown");

   // Try to extract the message from the notification
   let message = extract_notification_message(item_content);

   // Extract user info
   let from_users = extract_from_users(item_content);

   // Extract URL if available
   let url = extract_notification_url(item_content);

   // Extract icon URL
   let icon_url = item_content
      .pointer("/icon/iconUrl")
      .and_then(|u| u.as_str())
      .map(|s| s.to_string());

   Some(Notification {
      sort_index: sort_index.to_string(),
      notification_type: normalize_notification_type(notification_type),
      message,
      icon_url,
      url,
      from_users,
   })
}

fn extract_notification_message(item_content: &serde_json::Value) -> String {
   // Try multiple paths to get the message
   if let Some(message) = item_content
      .pointer("/message/text")
      .and_then(|t| t.as_str())
   {
      return message.to_string();
   }

   if let Some(header) = item_content
      .pointer("/header/text")
      .and_then(|t| t.as_str())
   {
      return header.to_string();
   }

   // For tweet-based notifications, try to get the tweet text
   if let Some(tweet) = item_content.pointer("/tweet_results/result/legacy/full_text")
      && let Some(text) = tweet.as_str()
   {
      return text.to_string();
   }

   "New notification".to_string()
}

fn extract_from_users(item_content: &serde_json::Value) -> Vec<String> {
   let mut users = Vec::new();

   // Try to get users from various paths
   if let Some(from_users) = item_content.get("fromUsers").and_then(|u| u.as_array()) {
      for user in from_users {
         if let Some(name) = user
            .pointer("/user_results/result/legacy/name")
            .and_then(|n| n.as_str())
         {
            users.push(name.to_string());
         }
      }
   }

   users
}

fn extract_notification_url(item_content: &serde_json::Value) -> Option<String> {
   // Try to get URL from notification
   if let Some(url) = item_content.pointer("/url/url").and_then(|u| u.as_str()) {
      return Some(url.to_string());
   }

   // Try to get tweet ID for tweet-based notifications
   if let Some(tweet_id) = item_content
      .pointer("/tweet_results/result/rest_id")
      .and_then(|id| id.as_str())
   {
      return Some(format!("https://x.com/i/status/{}", tweet_id));
   }

   None
}

fn normalize_notification_type(notification_type: &str) -> String {
   match notification_type.to_lowercase().as_str() {
      "like" | "likes" | "liked" => "like".to_string(),
      "retweet" | "retweets" | "retweeted" => "retweet".to_string(),
      "reply" | "replies" | "replied" => "reply".to_string(),
      "mention" | "mentions" | "mentioned" => "mention".to_string(),
      "follow" | "follows" | "followed" => "follow".to_string(),
      "quote" | "quotes" | "quoted" => "quote".to_string(),
      other => other.to_string(),
   }
}
