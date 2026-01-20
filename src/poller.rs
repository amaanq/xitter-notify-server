use std::{
   sync::Arc,
   time::Duration,
};

use tokio::{
   sync::Semaphore,
   time::interval,
};

use crate::{
   config::Config,
   db::{
      Db,
      User,
   },
   http_client::HttpClient,
   twitter,
   unified_push,
};

pub async fn run_poller(db: Arc<Db>, client: Arc<HttpClient>, config: Arc<Config>) {
   let mut poll_interval = interval(Duration::from_secs(config.poll_interval_secs));

   eprintln!(
      "[poller] Starting with {}s interval, max {} concurrent",
      config.poll_interval_secs, config.max_concurrent
   );

   loop {
      poll_interval.tick().await;

      let users = match db.get_all_users() {
         Ok(users) => users,
         Err(e) => {
            eprintln!("[poller] Failed to get users: {e}");
            continue;
         },
      };

      if users.is_empty() {
         continue;
      }

      eprintln!("[poller] Polling {} users", users.len());

      let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
      let mut handles = Vec::with_capacity(users.len());

      for user in users {
         let permit = semaphore.clone().acquire_owned().await.unwrap();
         let db = db.clone();
         let client = client.clone();

         handles.push(tokio::spawn(async move {
            if let Err(e) = poll_user(&db, &client, &user).await {
               eprintln!("[poller] Error polling user {}: {e}", user.twitter_user_id);
            }
            drop(permit);
         }));
      }

      // Wait for all poll tasks to complete
      for handle in handles {
         let _ = handle.await;
      }
   }
}

async fn poll_user(
   db: &Db,
   client: &HttpClient,
   user: &User,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
   let auth = user.auth();

   // 1. Check badge count (lightweight)
   let badge = twitter::get_badge_count(client, &auth).await?;

   if badge.ntab_unread_count == 0 {
      return Ok(());
   }

   // 2. Fetch notifications timeline
   let notifs = twitter::get_notifications(client, &auth).await?;

   // 3. Filter new ones (sort_index > last_seen)
   let new_notifs: Vec<_> = notifs
      .iter()
      .filter(|n| {
         user
            .last_notif_sort_index
            .as_ref()
            .map(|last| n.sort_index.as_str() > last.as_str())
            .unwrap_or(true)
      })
      .collect();

   if new_notifs.is_empty() {
      return Ok(());
   }

   eprintln!(
      "[poller] User {} has {} new notifications",
      user.twitter_user_id,
      new_notifs.len()
   );

   // 4. Send via UnifiedPush
   for notif in &new_notifs {
      if let Err(e) = unified_push::send(client, &user.up_endpoint, notif).await {
         eprintln!(
            "[poller] Failed to send notification to {}: {e}",
            user.twitter_user_id
         );
      }
   }

   // 5. Update last seen (use the newest sort_index)
   if let Some(newest) = new_notifs
      .iter()
      .max_by(|a, b| a.sort_index.cmp(&b.sort_index))
   {
      db.update_last_notif(user.id, &newest.sort_index)?;
   }

   Ok(())
}
