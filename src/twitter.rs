use std::{
   borrow::ToOwned,
   error::Error,
   fmt::{
      Display,
      Formatter,
      Result as FmtResult,
      Write as _,
   },
};

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

impl Display for TwitterError {
   fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
      match *self {
         Self::Http(ref error) => write!(f, "HTTP error: {error}"),
         Self::Parse(ref error) => write!(f, "Parse error: {error}"),
         Self::Api(ref error) => write!(f, "API error: {error}"),
      }
   }
}

impl Error for TwitterError {}

impl From<HttpError> for TwitterError {
   fn from(error: HttpError) -> Self {
      Self::Http(error)
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
         ("accept", "*/*".to_owned()),
         ("accept-language", "en-US,en;q=0.9".to_owned()),
         ("authorization", BEARER_TOKEN.to_owned()),
         ("cache-control", "no-cache".to_owned()),
         ("content-type", "application/json".to_owned()),
         ("pragma", "no-cache".to_owned()),
         ("priority", "u=1, i".to_owned()),
         ("referer", "https://x.com/".to_owned()),
         ("user-agent", USER_AGENT.to_owned()),
         ("x-twitter-active-user", "yes".to_owned()),
         ("x-twitter-client-language", "en".to_owned()),
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
   pub sort_index: String,
   #[serde(rename = "notification_type")]
   pub kind:       String,
   pub message:    String,
   pub icon_url:   Option<String>,
   pub url:        Option<String>,
   pub from_users: Vec<String>,
}

impl Notification {
   pub fn title(&self) -> String {
      match self.kind.as_str() {
         "like" => "New Like".to_owned(),
         "retweet" => "New Repost".to_owned(),
         "reply" => "New Reply".to_owned(),
         "mention" => "New Mention".to_owned(),
         "follow" => "New Follower".to_owned(),
         "quote" => "New Quote".to_owned(),
         _ => "New Notification".to_owned(),
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

   serde_json::from_slice(&body).map_err(|error| TwitterError::Parse(error.to_string()))
}

/// Fetch notifications timeline
pub async fn get_notifications(
   client: &HttpClient,
   auth: &TwitterAuth,
) -> Result<Vec<Notification>, TwitterError> {
   let variables = serde_json::json!({
      "count": 20_u8,
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

fn urlencoding(value: &str) -> String {
   let mut result = String::with_capacity(value.len() * 3);
   for character in value.chars() {
      match character {
         'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(character),
         _ => {
            for byte in character.to_string().as_bytes() {
               result.push('%');
               write!(result, "{byte:02X}").expect("writing to String cannot fail");
            }
         },
      }
   }
   result
}

fn parse_notifications(body: &[u8]) -> Result<Vec<Notification>, TwitterError> {
   let json = serde_json::from_slice::<serde_json::Value>(body)
      .map_err(|error| TwitterError::Parse(error.to_string()))?;

   if let Some(errors) = json.get("errors")
      && let Some(first_error) = errors.as_array().and_then(|arr| arr.first())
   {
      let message = first_error
         .get("message")
         .and_then(|message| message.as_str())
         .unwrap_or("Unknown error");
      return Err(TwitterError::Api(message.to_owned()));
   }

   let mut notifications = Vec::new();

   let timeline_instructions = json
      .pointer("/data/user/result/timeline/timeline/instructions")
      .and_then(|value| value.as_array());

   let Some(instructions) = timeline_instructions else {
      return Ok(notifications);
   };

   for instruction in instructions {
      if instruction.get("type").and_then(|kind| kind.as_str()) != Some("TimelineAddEntries") {
         continue;
      }

      let instruction_entries = instruction
         .get("entries")
         .and_then(|entries| entries.as_array());
      let Some(entries) = instruction_entries else {
         continue;
      };

      for entry in entries {
         let entry_id = entry
            .get("entryId")
            .and_then(|entry_id| entry_id.as_str())
            .unwrap_or("");

         // Skip cursors
         if entry_id.starts_with("cursor-") {
            continue;
         }

         let sort_index = entry
            .get("sortIndex")
            .and_then(|sort_index| sort_index.as_str())
            .unwrap_or("")
            .to_owned();

         if sort_index.is_empty() {
            continue;
         }

         let entry_content = entry.get("content");
         let Some(content) = entry_content else {
            continue;
         };

         if let Some(notif) = parse_notification_entry(content, &sort_index) {
            notifications.push(notif);
         }
      }
   }

   notifications.sort_by(|left, right| right.sort_index.cmp(&left.sort_index));

   Ok(notifications)
}

fn parse_notification_entry(content: &serde_json::Value, sort_index: &str) -> Option<Notification> {
   let item_content = content.get("itemContent")?;

   let notification_type = item_content
      .get("notificationType")
      .and_then(|kind| kind.as_str())
      .unwrap_or("unknown");

   let message = extract_notification_message(item_content);

   let from_users = extract_from_users(item_content);

   let notification_url = extract_notification_url(item_content);

   let icon_url = item_content
      .pointer("/icon/iconUrl")
      .and_then(|icon_url| icon_url.as_str())
      .map(ToOwned::to_owned);

   Some(Notification {
      sort_index: sort_index.to_owned(),
      kind: normalize_notification_type(notification_type),
      message,
      icon_url,
      url: notification_url,
      from_users,
   })
}

fn extract_notification_message(item_content: &serde_json::Value) -> String {
   if let Some(message) = item_content
      .pointer("/message/text")
      .and_then(|text| text.as_str())
   {
      return message.to_owned();
   }

   if let Some(header) = item_content
      .pointer("/header/text")
      .and_then(|text| text.as_str())
   {
      return header.to_owned();
   }

   if let Some(tweet) = item_content.pointer("/tweet_results/result/legacy/full_text")
      && let Some(text) = tweet.as_str()
   {
      return text.to_owned();
   }

   "New notification".to_owned()
}

fn extract_from_users(item_content: &serde_json::Value) -> Vec<String> {
   let mut users = Vec::new();

   if let Some(from_users) = item_content
      .get("fromUsers")
      .and_then(|from_users| from_users.as_array())
   {
      for user in from_users {
         if let Some(name) = user
            .pointer("/user_results/result/legacy/name")
            .and_then(|name| name.as_str())
         {
            users.push(name.to_owned());
         }
      }
   }

   users
}

fn extract_notification_url(item_content: &serde_json::Value) -> Option<String> {
   if let Some(url) = item_content
      .pointer("/url/url")
      .and_then(|url| url.as_str())
   {
      return Some(url.to_owned());
   }

   if let Some(tweet_id) = item_content
      .pointer("/tweet_results/result/rest_id")
      .and_then(|id| id.as_str())
   {
      return Some(format!("https://x.com/i/status/{tweet_id}"));
   }

   None
}

fn normalize_notification_type(notification_type: &str) -> String {
   match notification_type.to_lowercase().as_str() {
      "like" | "likes" | "liked" => "like".to_owned(),
      "retweet" | "retweets" | "retweeted" => "retweet".to_owned(),
      "reply" | "replies" | "replied" => "reply".to_owned(),
      "mention" | "mentions" | "mentioned" => "mention".to_owned(),
      "follow" | "follows" | "followed" => "follow".to_owned(),
      "quote" | "quotes" | "quoted" => "quote".to_owned(),
      other => other.to_owned(),
   }
}
