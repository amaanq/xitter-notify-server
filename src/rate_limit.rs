use std::{
   collections::HashMap,
   net::IpAddr,
   sync::RwLock,
   time::Instant,
};

pub struct RateLimiter {
   limits:       RwLock<HashMap<IpAddr, (u32, Instant)>>,
   max_requests: u32,
   window_secs:  u64,
}

impl RateLimiter {
   pub fn new(max_requests: u32, window_secs: u64) -> Self {
      Self {
         limits: RwLock::new(HashMap::new()),
         max_requests,
         window_secs,
      }
   }

   /// Check if request is allowed for the given IP
   /// Returns true if allowed, false if rate limited
   pub fn check(&self, ip: IpAddr) -> bool {
      let mut limits = self.limits.write().unwrap();
      let now = Instant::now();

      let entry = limits.entry(ip).or_insert((0, now));

      // Reset window if expired
      if entry.1.elapsed().as_secs() > self.window_secs {
         *entry = (0, now);
      }

      if entry.0 >= self.max_requests {
         return false; // Rate limited
      }

      entry.0 += 1;
      true
   }

   /// Clean up expired entries to prevent memory growth
   pub fn cleanup(&self) {
      let mut limits = self.limits.write().unwrap();
      limits.retain(|_, (_, instant)| instant.elapsed().as_secs() <= self.window_secs);
   }
}

/// Rate limiter with different limits for different operations
pub struct RateLimiters {
   pub register:   RateLimiter,
   pub unregister: RateLimiter,
}

impl RateLimiters {
   pub fn new() -> Self {
      Self {
         // 5 registrations per IP per hour
         register:   RateLimiter::new(5, 3600),
         // 10 unregistrations per IP per hour
         unregister: RateLimiter::new(10, 3600),
      }
   }

   /// Periodically clean up expired entries
   pub fn cleanup(&self) {
      self.register.cleanup();
      self.unregister.cleanup();
   }
}

impl Default for RateLimiters {
   fn default() -> Self {
      Self::new()
   }
}
