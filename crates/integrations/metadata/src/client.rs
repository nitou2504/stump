use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{
	policies::ExponentialBackoff, RetryTransientMiddleware, Retryable, RetryableStrategy,
};

const DEFAULT_MAX_RETRIES: u32 = 3;

/// A retry strategy that retries on 5xx, timeouts, etc, and 429 unless told not to
struct RetryOn429And5xx {
	retry_rate_limited: bool,
}

impl RetryableStrategy for RetryOn429And5xx {
	fn handle(
		&self,
		res: &Result<reqwest::Response, reqwest_middleware::Error>,
	) -> Option<Retryable> {
		match res {
			Ok(response) => {
				let status = response.status();
				if (status == reqwest::StatusCode::TOO_MANY_REQUESTS
					&& self.retry_rate_limited)
					|| status.is_server_error()
				{
					Some(Retryable::Transient)
				} else if status.is_client_error() {
					Some(Retryable::Fatal)
				} else {
					None
				}
			},
			Err(error) => reqwest_retry::default_on_request_failure(error),
		}
	}
}

pub struct RetryClientConfig {
	pub max_retries: u32,
	/// Whether a 429 is retried after a backoff, or handed back at once (for a client that
	/// keeps to the provider's limits and waits as long as it is asked)
	pub retry_rate_limited: bool,
}

impl Default for RetryClientConfig {
	fn default() -> Self {
		Self {
			max_retries: DEFAULT_MAX_RETRIES,
			retry_rate_limited: true,
		}
	}
}

/// Build a [`ClientWithMiddleware`] wrapping the given [`reqwest::Client`]
/// with exponential-backoff retry logic
pub fn build_client_with_retry(
	inner: Client,
	config: RetryClientConfig,
) -> ClientWithMiddleware {
	let retry_policy =
		ExponentialBackoff::builder().build_with_max_retries(config.max_retries);
	let retry_middleware = RetryTransientMiddleware::new_with_policy_and_strategy(
		retry_policy,
		RetryOn429And5xx {
			retry_rate_limited: config.retry_rate_limited,
		},
	);

	ClientBuilder::new(inner).with(retry_middleware).build()
}
