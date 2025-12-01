//! Duration utilities for time series queries

/// Duration in nanoseconds
pub struct Duration;

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
impl Duration {
    const NANOS_PER_SECOND: u128 = 1_000_000_000;
    const SECONDS_PER_MINUTE: u128 = 60;
    const SECONDS_PER_HOUR: u128 = 3600;
    const SECONDS_PER_DAY: u128 = 86400;
    const DAYS_PER_WEEK: f64 = 7.0;
    const DAYS_PER_MONTH: f64 = 30.44; // Average month length
    const DAYS_PER_YEAR: f64 = 365.25; // Average year length

    /// Returns the duration in nanoseconds for the given number of seconds.
    #[must_use]
    pub fn seconds(n: f64) -> u128 {
        (n * Self::NANOS_PER_SECOND as f64) as u128
    }

    /// Returns the duration in nanoseconds for the given number of minutes.
    #[must_use]
    pub fn minutes(n: f64) -> u128 {
        Self::seconds(n * Self::SECONDS_PER_MINUTE as f64)
    }

    /// Returns the duration in nanoseconds for the given number of hours.
    #[must_use]
    pub fn hours(n: f64) -> u128 {
        Self::seconds(n * Self::SECONDS_PER_HOUR as f64)
    }

    /// Returns the duration in nanoseconds for the given number of days.
    #[must_use]
    pub fn days(n: f64) -> u128 {
        Self::seconds(n * Self::SECONDS_PER_DAY as f64)
    }

    /// Returns the duration in nanoseconds for the given number of weeks.
    #[must_use]
    pub fn weeks(n: f64) -> u128 {
        Self::days(n * Self::DAYS_PER_WEEK)
    }

    /// Returns the duration in nanoseconds for the given number of months.
    #[must_use]
    pub fn months(n: f64) -> u128 {
        Self::days(n * Self::DAYS_PER_MONTH)
    }

    /// Returns the duration in nanoseconds for the given number of years.
    #[must_use]
    pub fn years(n: f64) -> u128 {
        Self::days(n * Self::DAYS_PER_YEAR)
    }
}
