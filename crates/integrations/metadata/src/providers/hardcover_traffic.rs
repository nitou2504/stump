//! Hardcover's limits are per token: 60 requests a minute (in bursts of 10) and 5,000 a day,
//! see its `RateLimit-Policy` header. Every Hardcover request this process sends goes through
//! [`TRAFFIC`]: one limiter for all of them, and a slower one that background work (the
//! metadata fetch job) waits for as well, so a person's lookup always finds room; a daily
//! budget for background work; and, after a refusal (429), a hold for as long as Hardcover
//! asks, with background work stopped for the rest of the UTC day.

use std::{
	sync::{LazyLock, Mutex},
	time::{Duration, Instant},
};

use chrono::{DateTime, NaiveDate, Utc};

use crate::RateLimiter;

/// Who a Hardcover request is for
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestPriority {
	/// Someone is waiting for it: a lookup (Add books, Suggest genres) or a fetch they
	/// started for a book or series
	#[default]
	Interactive,
	/// The metadata fetch job (after a scan, a library-wide fetch, the scheduled retries)
	Background,
}

/// Background requests a UTC day, leaving most of Hardcover's 5,000 to people
pub const BACKGROUND_DAILY_BUDGET: u32 = 1000;
/// Every request: a burst of 5, then one every 1.1 s (under 60 a minute)
const SHARED_BURST: u32 = 5;
const SHARED_PERIOD: Duration = Duration::from_millis(1100);
/// Background requests also wait for one every 4 s (15 a minute)
const BACKGROUND_PERIOD: Duration = Duration::from_secs(4);
/// After a 429 that doesn't say how long to wait
const DEFAULT_WAIT: Duration = Duration::from_secs(60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// The background requests sent on a UTC day
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DailyBudget {
	day: Option<NaiveDate>,
	used: u32,
}

impl DailyBudget {
	/// Takes one request from the day's budget, or says it is used up
	pub fn take(&mut self, today: NaiveDate, limit: u32) -> bool {
		if self.day != Some(today) {
			self.day = Some(today);
			self.used = 0;
		}
		if self.used >= limit {
			return false;
		}
		self.used += 1;
		true
	}
}

/// How long background work stops after a refusal: as long as Hardcover asks, and at least
/// until the next UTC day, when the daily quota starts again
pub fn background_hold(now: DateTime<Utc>, retry_after: Duration) -> Duration {
	let midnight = (now.date_naive() + chrono::Days::new(1))
		.and_hms_opt(0, 0, 0)
		.map(|t| t.and_utc());
	let to_midnight = midnight
		.and_then(|m| (m - now).to_std().ok())
		.unwrap_or_default();
	retry_after.max(to_midnight).min(DAY)
}

/// Why a request may not be sent now
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Held {
	/// Hardcover asked to wait this much longer
	Wait(Duration),
	/// Background work stopped for the day (a refusal, or its budget is used up)
	Background,
}

pub(crate) struct Traffic {
	shared: RateLimiter,
	background: RateLimiter,
	budget: Mutex<DailyBudget>,
	/// Until when Hardcover asked not to be called (its `Retry-After`)
	hold: Mutex<Option<Instant>>,
	/// Until when background work stays stopped
	background_hold: Mutex<Option<Instant>>,
}

pub(crate) static TRAFFIC: LazyLock<Traffic> = LazyLock::new(|| Traffic {
	shared: RateLimiter::with_period(SHARED_PERIOD, SHARED_BURST),
	background: RateLimiter::with_period(BACKGROUND_PERIOD, 1),
	budget: Mutex::new(DailyBudget::default()),
	hold: Mutex::new(None),
	background_hold: Mutex::new(None),
});

fn remaining(slot: &Mutex<Option<Instant>>) -> Option<Duration> {
	let until = (*slot.lock().ok()?)?;
	until
		.checked_duration_since(Instant::now())
		.filter(|wait| !wait.is_zero())
}

impl Traffic {
	/// Waits for a request's turn, or says why it may not be sent: Hardcover asked to wait,
	/// or background work stopped for the day or used up its budget
	pub(crate) async fn admit(&self, priority: RequestPriority) -> Result<(), Held> {
		if let Some(wait) = remaining(&self.hold) {
			return Err(Held::Wait(wait));
		}
		if priority == RequestPriority::Background {
			if remaining(&self.background_hold).is_some() {
				return Err(Held::Background);
			}
			let allowed = self
				.budget
				.lock()
				.map(|mut budget| {
					budget.take(Utc::now().date_naive(), BACKGROUND_DAILY_BUDGET)
				})
				.unwrap_or(false);
			if !allowed {
				let hold = background_hold(Utc::now(), Duration::ZERO);
				if let Ok(mut until) = self.background_hold.lock() {
					*until = Some(Instant::now() + hold);
				}
				tracing::warn!(
					budget = BACKGROUND_DAILY_BUDGET,
					"Hardcover's daily budget for background work is used up: the rest waits for tomorrow"
				);
				return Err(Held::Background);
			}
			self.background.until_ready().await;
		}
		self.shared.until_ready().await;
		Ok(())
	}

	/// Hardcover refused a request (429): nothing is sent until `retry_after` has passed,
	/// and background work stops for the rest of the day
	pub(crate) fn refused(&self, retry_after: Option<Duration>) {
		let wait = retry_after.unwrap_or(DEFAULT_WAIT).min(DAY);
		let now = Instant::now();
		if let Ok(mut until) = self.hold.lock() {
			*until = Some(now + wait);
		}
		if let Ok(mut until) = self.background_hold.lock() {
			*until = Some(now + background_hold(Utc::now(), wait));
		}
		tracing::warn!(
			seconds = wait.as_secs(),
			"Hardcover refused a request (rate limit): waiting as asked, background work stops for the day"
		);
	}

	/// Whether background work is stopped for now (a refusal, or the day's budget is used
	/// up)
	pub(crate) fn background_paused(&self) -> bool {
		remaining(&self.hold).is_some() || remaining(&self.background_hold).is_some()
	}
}

/// Whether Hardcover background work (the metadata fetch job) is stopped for now: Hardcover
/// refused a request today, or the day's budget for background work is used up
pub fn hardcover_background_paused() -> bool {
	TRAFFIC.background_paused()
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::TimeZone;

	#[test]
	fn the_background_budget_is_per_utc_day() {
		let mut budget = DailyBudget::default();
		let monday = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
		let tuesday = NaiveDate::from_ymd_opt(2026, 11, 3).unwrap();
		assert!((0..3).all(|_| budget.take(monday, 3)));
		assert!(
			!budget.take(monday, 3),
			"the fourth request of the day waits"
		);
		assert!(budget.take(tuesday, 3), "a new day, a new budget");
	}

	#[test]
	fn background_work_stops_until_the_next_utc_day_or_longer() {
		let evening = Utc.with_ymd_and_hms(2026, 11, 3, 22, 0, 0).unwrap();
		assert_eq!(
			background_hold(evening, Duration::from_secs(60)),
			Duration::from_secs(2 * 60 * 60)
		);
		assert_eq!(
			background_hold(evening, Duration::from_secs(3 * 60 * 60)),
			Duration::from_secs(3 * 60 * 60)
		);
		let last_second = Utc.with_ymd_and_hms(2026, 11, 3, 23, 59, 59).unwrap();
		assert_eq!(
			background_hold(last_second, Duration::ZERO),
			Duration::from_secs(1)
		);
	}
}
