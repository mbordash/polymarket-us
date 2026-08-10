use std::time::Duration;

/// Configuration for automatic request retries with exponential backoff and jitter.
///
/// Only **idempotent** HTTP methods (`GET`, `DELETE`) are retried automatically.
/// `POST` requests are never retried by default to prevent duplicate order submissions.
///
/// # Example
/// ```rust
/// use polymarket_us::{PolymarketUsClient, RetryConfig};
/// use std::time::Duration;
///
/// let client = PolymarketUsClient::builder()
///     .retry(RetryConfig {
///         max_retries: 5,
///         initial_backoff: Duration::from_millis(100),
///         max_backoff: Duration::from_secs(30),
///         jitter_factor: 0.3,
///     })
///     .build()
///     .unwrap();
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 = no retries). Default: `3`.
    pub max_retries: u32,

    /// Initial backoff before the first retry. Default: `200ms`.
    pub initial_backoff: Duration,

    /// Upper bound on backoff after exponential growth. Default: `10s`.
    pub max_backoff: Duration,

    /// Fraction of the computed backoff added as random jitter (0.0–1.0).
    /// Prevents thundering-herd retry storms. Default: `0.25`.
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(200),
            max_backoff: Duration::from_secs(10),
            jitter_factor: 0.25,
        }
    }
}

impl RetryConfig {
    /// Disable retries entirely.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    /// Aggressive retry settings for resilient workflows.
    pub fn aggressive() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            jitter_factor: 0.3,
        }
    }

    /// Compute the backoff duration for the given 1-indexed attempt number.
    ///
    /// Uses `initial_backoff × 2^(attempt−1)` capped at `max_backoff`,
    /// plus jitter derived from the subsecond system clock.
    pub(crate) fn backoff_for(&self, attempt: u32) -> Duration {
        // Jitter seed from the subsecond clock — avoids pulling in `rand`.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        self.backoff_with_seed(attempt, seed)
    }

    /// Backoff computation with an explicit jitter seed, so the jitter range can
    /// be tested without depending on the wall clock.
    ///
    /// `seed_nanos` is expected in `0..1_000_000_000` and is normalised against
    /// that range. Normalising against `u32::MAX` instead would cap jitter at
    /// roughly 23% of `jitter_factor` rather than spanning it.
    fn backoff_with_seed(&self, attempt: u32, seed_nanos: u32) -> Duration {
        const NANOS_PER_SEC: f64 = 1_000_000_000.0;

        let base_ms = self.initial_backoff.as_millis() as f64;
        let exp = 2_f64.powi(attempt.saturating_sub(1) as i32);
        let backoff_ms = (base_ms * exp).min(self.max_backoff.as_millis() as f64);

        let jitter_range_ms = backoff_ms * self.jitter_factor.clamp(0.0, 1.0);
        let fraction = (seed_nanos as f64 / NANOS_PER_SEC).clamp(0.0, 1.0);

        Duration::from_millis((backoff_ms + fraction * jitter_range_ms) as u64)
    }
}

/// Returns `true` for HTTP status codes that are safe to retry.
pub(crate) fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_reasonable_values() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.initial_backoff, Duration::from_millis(200));
        assert_eq!(cfg.max_backoff, Duration::from_secs(10));
        assert!((cfg.jitter_factor - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn none_config_disables_retries() {
        assert_eq!(RetryConfig::none().max_retries, 0);
    }

    #[test]
    fn backoff_grows_exponentially() {
        let cfg = RetryConfig {
            max_retries: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(60),
            jitter_factor: 0.0,
        };
        assert_eq!(cfg.backoff_for(1), Duration::from_millis(100));
        assert_eq!(cfg.backoff_for(2), Duration::from_millis(200));
        assert_eq!(cfg.backoff_for(3), Duration::from_millis(400));
        assert_eq!(cfg.backoff_for(4), Duration::from_millis(800));
    }

    #[test]
    fn backoff_caps_at_max() {
        let cfg = RetryConfig {
            max_retries: 10,
            initial_backoff: Duration::from_millis(1000),
            max_backoff: Duration::from_secs(5),
            jitter_factor: 0.0,
        };
        assert_eq!(cfg.backoff_for(10), Duration::from_secs(5));
    }

    #[test]
    fn backoff_with_jitter_is_within_expected_range() {
        let cfg = RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(60),
            jitter_factor: 0.25,
        };
        let b = cfg.backoff_for(1);
        // 100ms base + up to 25ms jitter
        assert!(b >= Duration::from_millis(100));
        assert!(b <= Duration::from_millis(125));
    }

    #[test]
    fn jitter_spans_the_full_configured_range() {
        let cfg = RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(1000),
            max_backoff: Duration::from_secs(60),
            jitter_factor: 0.5,
        };

        // A zero seed adds no jitter; a near-maximum seed adds nearly all of it.
        // Before the fix the top of the range reached only ~1116ms, because the
        // seed was normalised against u32::MAX rather than one second of nanos.
        assert_eq!(cfg.backoff_with_seed(1, 0), Duration::from_millis(1000));
        assert_eq!(
            cfg.backoff_with_seed(1, 999_999_999),
            Duration::from_millis(1499)
        );
        assert_eq!(
            cfg.backoff_with_seed(1, 500_000_000),
            Duration::from_millis(1250)
        );
    }

    #[test]
    fn jitter_never_exceeds_the_configured_factor() {
        let cfg = RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(200),
            max_backoff: Duration::from_secs(60),
            jitter_factor: 0.25,
        };
        for seed in [0, 1, 250_000_000, 999_999_999, u32::MAX] {
            let b = cfg.backoff_with_seed(1, seed);
            assert!(b >= Duration::from_millis(200), "seed {seed} underflowed");
            assert!(b <= Duration::from_millis(250), "seed {seed} overflowed");
        }
    }

    #[test]
    fn retryable_status_codes() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(504));
    }

    #[test]
    fn non_retryable_status_codes() {
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(403));
        assert!(!is_retryable_status(404));
    }
}
