use std::{
   error::Error,
   sync::Arc,
   time::Duration,
};

use tokio::{
   select,
   signal::ctrl_c,
   sync::Semaphore,
   time::{
      MissedTickBehavior,
      interval,
   },
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

   poll_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

   while select! {
      () = async {
         poll_interval.tick().await;
      } => true,
      signal = ctrl_c() => {
         if let Err(error) = signal {
            eprintln!("[poller] Failed to listen for shutdown signal: {error}");
         }
         false
      },
   } {
      let users = match db.get_all_users() {
         Ok(users) => users,
         Err(error) => {
            eprintln!("[poller] Failed to get users: {error}");
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
         let permit = Arc::clone(&semaphore).acquire_owned().await.unwrap();
         let poll_db = Arc::clone(&db);
         let poll_client = Arc::clone(&client);

         handles.push(tokio::spawn(async move {
            if let Err(error) = poll_user(&poll_db, &poll_client, &user).await {
               eprintln!(
                  "[poller] Error polling user {}: {error}",
                  user.twitter_user_id
               );
            }
            drop(permit);
         }));
      }

      for handle in handles {
         let _ = handle.await;
      }
   }
}

async fn poll_user(
   db: &Db,
   client: &HttpClient,
   user: &User,
) -> Result<(), Box<dyn Error + Send + Sync>> {
   let auth = user.auth();

   // 1. Check badge count (lightweight)
   let badge = twitter::get_badge_count(client, &auth).await?;

   if badge.ntab_unread_count == 0_i32 {
      return Ok(());
   }

   // 2. Fetch notifications timeline
   let notifs = twitter::get_notifications(client, &auth).await?;

   // 3. Filter new ones (sort_index > last_seen)
   let new_notifs = notifs
      .iter()
      .filter(|notification| {
         user
            .last_notif_sort_index
            .as_ref()
            .is_none_or(|last| notification.sort_index.as_str() > last.as_str())
      })
      .collect::<Vec<_>>();

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
      if let Err(error) = unified_push::send(client, &user.up_endpoint, notif).await {
         eprintln!(
            "[poller] Failed to send notification to {}: {error}",
            user.twitter_user_id
         );
      }
   }

   // 5. Update last seen (use the newest sort_index)
   if let Some(newest) = new_notifs
      .iter()
      .max_by(|left, right| left.sort_index.cmp(&right.sort_index))
   {
      db.update_last_notif(user.id, &newest.sort_index)?;
   }

   Ok(())
}
