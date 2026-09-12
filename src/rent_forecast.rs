//! Storage-rent forecasting for Soroban contracts.
//!
//! Projects 30-day, 180-day, and 365-day rent costs per storage tier
//! (Instance, Persistent, Temporary) using real `ConfigSetting*` rent rates
//! from the network — no hardcoded rates.
//!
//! # Rent Formula
//!
//! Rent is a two-step computation, mirroring the protocol implementation in
//! [`rs-soroban-env` `fees.rs`](https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs)
//! (`compute_rent_write_fee_per_1kb` → `rent_fee_for_size_and_ledgers`).
//!
//! **Step 1 — effective rate per 1 KB.** The rate is interpolated between
//! `rent_fee1_kb_soroban_state_size_low` (at zero state) and `..._high` (at
//! `soroban_state_target_size_bytes`), then floored at
//! [`MINIMUM_RENT_WRITE_FEE_PER_1KB`]:
//!
//! ```text
//! multiplier = max(high - low, 0)
//! if state_size < target:
//!     rate = ceil(multiplier * state_size / target) + low
//! else:
//!     rate = high + ceil(multiplier * (state_size - target) * growth_factor / target)
//! rate = max(rate, MINIMUM_RENT_WRITE_FEE_PER_1KB)
//! ```
//!
//! The `+ low` term can legitimately be negative when `low < 0`; the final
//! floor is what makes real output non-zero. Using `low` on its own (as an
//! earlier revision of this module did) yields zero or negative rent for any
//! network whose low rate is negative — e.g. testnet, where `low = -17000`.
//!
//! **Step 2 — fee for a size and a number of ledgers.**
//!
//! ```text
//! rent_fee = ceil(size_bytes * rate * ledgers)
//!          / (1024 * rate_denominator)
//! ```
//!
//! where `rate_denominator` is `persistent_rent_rate_denominator` (Persistent
//! and Instance) or `temp_rent_rate_denominator` (Temporary) from
//! `StateArchivalV0`. All arithmetic is integer `ceil` division, matching the
//! protocol — not floating point.

use serde::{Deserialize, Serialize};

use crate::error::{BenchError, BenchResult};

/// Ledgers per day on Stellar (approximate, from protocol spec).
/// Real value varies; 17,280 is the typical rate (~5s per ledger).
pub const LEDGERS_PER_DAY: u64 = 17_280;

/// Protocol floor on the effective rent rate per 1 KB.
///
/// Mirrors `MINIMUM_RENT_WRITE_FEE_PER_1KB` in the protocol's `fees.rs`.
pub const MINIMUM_RENT_WRITE_FEE_PER_1KB: i64 = 1000;

/// Storage tiers on Soroban.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageTier {
    /// Contract code and instance data — lives as long as the contract exists.
    Instance,
    /// Long-lived user data (maps, balances, etc.).
    Persistent,
    /// Short-lived cache data (TTL-expired entries).
    Temporary,
}

impl std::fmt::Display for StorageTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Instance => write!(f, "Instance"),
            Self::Persistent => write!(f, "Persistent"),
            Self::Temporary => write!(f, "Temporary"),
        }
    }
}

/// A single rent forecast entry for a given tier and time horizon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RentForecastEntry {
    /// Storage tier this forecast applies to.
    pub tier: StorageTier,
    /// Number of days in the forecast horizon.
    pub days: u32,
    /// Number of ledgers in this period.
    pub ledgers: u64,
    /// Data size in bytes.
    pub size_bytes: u64,
    /// Rent fee in stroops (1 XLM = 10,000,000 stroops).
    pub rent_stroops: u64,
    /// Rent fee in XLM.
    pub rent_xlm: f64,
    /// Daily rate in stroops (for comparison).
    pub daily_stroops: u64,
}

/// Complete rent forecast for a contract's storage footprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RentForecast {
    /// Network used for the forecast (testnet, mainnet, etc.).
    pub network: String,
    /// Config snapshot timestamp.
    pub config_timestamp: String,
    /// Config ledger height at snapshot time.
    pub config_ledger: u32,
    /// Where the rent rates came from: a live RPC fetch, a `--config-snapshot`
    /// file, or explicit demo placeholders. Recorded so placeholder output can
    /// never be mistaken for network-derived numbers.
    pub config_source: String,
    /// Effective rent rate per 1 KB actually applied to every entry below.
    pub effective_rent_rate_1kb: i64,
    /// How that rate was derived — interpolated, high+growth, or floor.
    pub rate_basis: String,
    /// Live Soroban state size used for the interpolation, when known.
    pub soroban_state_size_bytes: Option<i64>,
    /// Forecasts grouped by tier.
    pub entries: Vec<RentForecastEntry>,
    /// Summary: total cost across all tiers for each horizon.
    pub summary: Vec<HorizonSummary>,
}

/// Summary across all tiers for a single time horizon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizonSummary {
    pub days: u32,
    pub total_stroops: u64,
    pub total_xlm: f64,
}

/// Raw config data needed for rent calculations.
#[derive(Debug, Clone)]
pub struct RentConfig {
    /// Persistent storage rent rate denominator.
    pub persistent_rent_rate_denominator: i64,
    /// Temporary storage rent rate denominator.
    pub temp_rent_rate_denominator: i64,
    /// Rent fee per 1 KB when the Soroban state size is 0.
    pub rent_fee_1kb_low: i64,
    /// Rent fee per 1 KB once the state reaches `state_target_size_bytes`.
    pub rent_fee_1kb_high: i64,
    /// Growth multiplier applied to state size beyond the target.
    pub rent_fee_growth_factor: u32,
    /// State size at which the high rate is reached.
    pub state_target_size_bytes: i64,
    /// Current live Soroban state size, from the on-chain
    /// `LiveSorobanStateSizeWindow` setting.
    ///
    /// `None` when the state size is unknown (e.g. a snapshot file with no
    /// window data). In that case the minimum floor is used and the forecast is
    /// marked accordingly rather than silently guessing.
    pub soroban_state_size_bytes: Option<i64>,
}

/// Extract rent config from a `ConfigSnapshot`.
///
/// `soroban_state_size_bytes` is the live Soroban state size, which lives in a
/// separate `LiveSorobanStateSizeWindow` config setting and so is not part of
/// `ConfigSnapshot`. Pass `None` when it is unavailable; the minimum floor is
/// then used.
///
/// # Errors
///
/// Returns `BenchError::MissingConfig` if required config sections are absent.
pub fn extract_rent_config(
    snapshot: &soroban_cost_estimator::config_snapshot::model::ConfigSnapshot,
    soroban_state_size_bytes: Option<i64>,
) -> BenchResult<RentConfig> {
    let archival = snapshot
        .state_archival
        .as_ref()
        .ok_or_else(|| BenchError::MissingConfig("StateArchivalV0 not available".into()))?;

    let ledger_cost = snapshot
        .contract_ledger_cost
        .as_ref()
        .ok_or_else(|| BenchError::MissingConfig("ContractLedgerCostV0 not available".into()))?;

    Ok(RentConfig {
        persistent_rent_rate_denominator: archival.persistent_rent_rate_denominator,
        temp_rent_rate_denominator: archival.temp_rent_rate_denominator,
        rent_fee_1kb_low: ledger_cost.rent_fee1_kb_soroban_state_size_low,
        rent_fee_1kb_high: ledger_cost.rent_fee1_kb_soroban_state_size_high,
        rent_fee_growth_factor: ledger_cost.soroban_state_rent_fee_growth_factor,
        state_target_size_bytes: ledger_cost.soroban_state_target_size_bytes,
        soroban_state_size_bytes,
    })
}

/// Integer ceiling division, matching `num_integer::div_ceil` on non-negative
/// numerators (which is all the protocol uses it for here).
fn div_ceil_i128(numerator: i128, denominator: i128) -> i128 {
    let denom = denominator.max(1);
    let quotient = numerator / denom;
    if numerator % denom != 0 && numerator > 0 {
        quotient + 1
    } else {
        quotient
    }
}

/// The protocol's `clamp_fee` helper: negatives map to `i64::MAX` — the
/// protocol's deliberate "fail loudly rather than open a zero-cost path"
/// choice, reproduced faithfully here.
fn clamp_fee(value: i128) -> i64 {
    if value < 0 {
        i64::MAX
    } else {
        i64::try_from(value).unwrap_or(i64::MAX)
    }
}

/// The effective rent rate per 1 KB for the current Soroban state size.
///
/// Mirrors `compute_rent_write_fee_per_1kb` from the protocol, including the
/// placement of its clamps: the interpolated product is clamped, but adding the
/// (possibly negative) low rate is **not** — the final floor is what guarantees
/// a positive rent rate.
#[must_use]
pub fn effective_rent_rate_1kb(config: &RentConfig) -> i64 {
    let Some(state_size) = config.soroban_state_size_bytes else {
        // State size unknown: fall back to the protocol's floor applied to the
        // low end of the curve, and let the caller label the forecast.
        return config.rent_fee_1kb_low.max(MINIMUM_RENT_WRITE_FEE_PER_1KB);
    };

    // `clamp_fee` on the multiplier, per the protocol.
    let multiplier = clamp_fee(
        i128::from(config.rent_fee_1kb_high).saturating_sub(i128::from(config.rent_fee_1kb_low)),
    );
    let target = config.state_target_size_bytes.max(1);

    let rate = if state_size < config.state_target_size_bytes {
        let product = clamp_fee(div_ceil_i128(
            i128::from(multiplier) * i128::from(state_size),
            i128::from(target),
        ));
        // Deliberately unclamped: this is where a negative low rate enters, and
        // the floor below is what keeps the final rate positive.
        product.saturating_add(config.rent_fee_1kb_low)
    } else {
        let past_target = state_size.saturating_sub(config.state_target_size_bytes);
        let post_target = clamp_fee(div_ceil_i128(
            i128::from(multiplier)
                * i128::from(past_target)
                * i128::from(config.rent_fee_growth_factor),
            i128::from(target),
        ));
        config.rent_fee_1kb_high.saturating_add(post_target)
    };

    rate.max(MINIMUM_RENT_WRITE_FEE_PER_1KB)
}

/// Human-readable note on how the effective rate was derived.
#[must_use]
pub fn rate_basis(config: &RentConfig) -> String {
    match config.soroban_state_size_bytes {
        Some(state_size) if state_size >= config.state_target_size_bytes => format!(
            "high+growth (state {} >= target {})",
            state_size, config.state_target_size_bytes
        ),
        Some(state_size) => format!(
            "interpolated (state {} < target {})",
            state_size, config.state_target_size_bytes
        ),
        None => "floor (state size unknown)".to_string(),
    }
}

/// Calculate rent for a given size, duration, and tier.
///
/// Uses the protocol's rent formula with real config rates.
///
/// # Arguments
///
/// * `config` - Network config rates from `ConfigSnapshot`.
/// * `tier` - Storage tier (determines which rate denominator to use).
/// * `size_bytes` - Size of the storage entry in bytes.
/// * `days` - Number of days to forecast.
///
/// # Returns
///
/// Rent fee in stroops.
pub fn calculate_rent(config: &RentConfig, tier: StorageTier, size_bytes: u64, days: u32) -> u64 {
    let ledgers = u64::from(days) * LEDGERS_PER_DAY;

    // Determine the rate denominator based on tier.
    let rate_denominator = match tier {
        StorageTier::Persistent | StorageTier::Instance => config.persistent_rent_rate_denominator,
        StorageTier::Temporary => config.temp_rent_rate_denominator,
    };

    let rate = effective_rent_rate_1kb(config);

    // rent_fee = ceil(size_bytes * rate * ledgers / (1024 * rate_denominator))
    // i128 keeps large sizes exact; the protocol uses saturating i64.
    let numerator = i128::from(size_bytes) * i128::from(rate) * i128::from(ledgers);
    let denominator = 1024 * i128::from(rate_denominator).max(1);
    let fee = div_ceil_i128(numerator, denominator).max(0);

    u64::try_from(fee).unwrap_or(u64::MAX)
}

/// Generate a full rent forecast for a set of storage entries.
///
/// # Arguments
///
/// * `config` - Network config rates.
/// * `network` - Network name (for metadata).
/// * `config_timestamp` - When the config was snapshot.
/// * `config_ledger` - Ledger height at snapshot.
/// * `config_source` - Provenance of the rates (see [`RentForecast::config_source`]).
/// * `entries` - List of `(tier, size_bytes)` pairs representing the contract's
///   storage footprint.
/// * `horizons` - Time horizons to forecast (e.g., [30, 180, 365]).
#[allow(clippy::too_many_arguments)]
pub fn generate_forecast(
    config: &RentConfig,
    network: &str,
    config_timestamp: &str,
    config_ledger: u32,
    config_source: &str,
    entries: &[(StorageTier, u64)],
    horizons: &[u32],
) -> RentForecast {
    let mut forecast_entries = Vec::new();
    let mut horizon_totals: std::collections::HashMap<u32, (u64, f64)> =
        std::collections::HashMap::new();

    for &(tier, size_bytes) in entries {
        for &days in horizons {
            let rent_stroops = calculate_rent(config, tier, size_bytes, days);
            let ledgers = u64::from(days) * LEDGERS_PER_DAY;
            let rent_xlm = stroops_to_xlm(rent_stroops);
            let daily_stroops = rent_stroops / u64::from(days);

            forecast_entries.push(RentForecastEntry {
                tier,
                days,
                ledgers,
                size_bytes,
                rent_stroops,
                rent_xlm,
                daily_stroops,
            });

            let entry = horizon_totals.entry(days).or_insert((0, 0.0));
            entry.0 += rent_stroops;
            entry.1 += rent_xlm;
        }
    }

    let summary: Vec<HorizonSummary> = horizons
        .iter()
        .map(|&days| {
            let (total_stroops, total_xlm) = horizon_totals.get(&days).copied().unwrap_or((0, 0.0));
            HorizonSummary {
                days,
                total_stroops,
                total_xlm,
            }
        })
        .collect();

    RentForecast {
        network: network.to_string(),
        config_timestamp: config_timestamp.to_string(),
        config_ledger,
        config_source: config_source.to_string(),
        effective_rent_rate_1kb: effective_rent_rate_1kb(config),
        rate_basis: rate_basis(config),
        soroban_state_size_bytes: config.soroban_state_size_bytes,
        entries: forecast_entries,
        summary,
    }
}

/// Convert stroops to XLM.
#[must_use]
pub fn stroops_to_xlm(stroops: u64) -> f64 {
    f64::from(stroops as f32) / 10_000_000.0
}

/// Format a rent forecast as a human-readable table.
#[must_use]
pub fn format_forecast_table(forecast: &RentForecast) -> String {
    use comfy_table::{Cell, Color, Table};

    let mut table = Table::new();
    table.set_header(vec![
        "Tier",
        "Days",
        "Size (B)",
        "Rent (stroops)",
        "Rent (XLM)",
        "Daily (stroops)",
    ]);

    for entry in &forecast.entries {
        table.add_row(vec![
            Cell::new(entry.tier),
            Cell::new(entry.days),
            Cell::new(entry.size_bytes),
            Cell::new(format_stroops(entry.rent_stroops)),
            Cell::new(format_xlm(entry.rent_xlm)),
            Cell::new(format_stroops(entry.daily_stroops)),
        ]);
    }

    // Add separator + summary rows
    table.add_row(vec!["---"; 6]);
    for s in &forecast.summary {
        table.add_row(vec![
            Cell::new("TOTAL").fg(Color::Cyan),
            Cell::new(s.days).fg(Color::Cyan),
            Cell::new("").fg(Color::Cyan),
            Cell::new(format_stroops(s.total_stroops)).fg(Color::Cyan),
            Cell::new(format_xlm(s.total_xlm)).fg(Color::Cyan),
            Cell::new("").fg(Color::Cyan),
        ]);
    }

    table.to_string()
}

/// Format stroops with thousand separators.
pub fn format_stroops(stroops: u64) -> String {
    let s = stroops.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format XLM to 8 decimal places.
fn format_xlm(xlm: f64) -> String {
    format!("{xlm:.8}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a minimal RentConfig for testing.
    ///
    /// Flat rate of 1267 stroops/1 KB: `low == high`, so the interpolation is
    /// a no-op and the effective rate is exactly 1267 everywhere.
    fn test_config() -> RentConfig {
        RentConfig {
            persistent_rent_rate_denominator: 4096,
            temp_rent_rate_denominator: 4096,
            rent_fee_1kb_low: 1_267,
            rent_fee_1kb_high: 1_267,
            rent_fee_growth_factor: 0,
            state_target_size_bytes: 1,
            soroban_state_size_bytes: Some(0),
        }
    }

    /// The real testnet ledger-cost curve, as observed 2026-09-12.
    fn testnet_config() -> RentConfig {
        RentConfig {
            persistent_rent_rate_denominator: 1215,
            temp_rent_rate_denominator: 2430,
            rent_fee_1kb_low: -17_000,
            rent_fee_1kb_high: 10_000,
            rent_fee_growth_factor: 5_000,
            state_target_size_bytes: 4_000_000_000,
            soroban_state_size_bytes: Some(0),
        }
    }

    #[test]
    fn test_calculate_rent_persistent_30_day() {
        let config = test_config();
        let rent = calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        let expected = (30.0 * (LEDGERS_PER_DAY as f64) * 1267.0 / 4096.0).ceil() as u64;
        assert_eq!(rent, expected, "Persistent 30-day rent mismatch");
    }

    #[test]
    fn test_calculate_rent_temporary_30_day() {
        let config = test_config();
        let rent = calculate_rent(&config, StorageTier::Temporary, 1024, 30);
        let expected = (30.0 * (LEDGERS_PER_DAY as f64) * 1267.0 / 4096.0).ceil() as u64;
        assert_eq!(rent, expected, "Temporary 30-day rent mismatch");
    }

    #[test]
    fn test_calculate_rent_zero_size() {
        let config = test_config();
        let rent = calculate_rent(&config, StorageTier::Persistent, 0, 30);
        assert_eq!(rent, 0, "Zero size should yield zero rent");
    }

    #[test]
    fn test_calculate_rent_zero_days() {
        let config = test_config();
        let rent = calculate_rent(&config, StorageTier::Persistent, 1024, 0);
        assert_eq!(rent, 0, "Zero days should yield zero rent");
    }

    #[test]
    fn test_calculate_rent_scales_with_size() {
        let config = test_config();
        let rent_1k = calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        let rent_2k = calculate_rent(&config, StorageTier::Persistent, 2048, 30);
        assert_eq!(rent_2k, rent_1k * 2, "Rent should scale linearly with size");
    }

    #[test]
    fn test_calculate_rent_scales_with_time() {
        let config = test_config();
        let rent_30 = calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        let rent_60 = calculate_rent(&config, StorageTier::Persistent, 1024, 60);
        assert_eq!(rent_60, rent_30 * 2, "Rent should scale linearly with time");
    }

    /// Below the state target the growth factor must have **no** effect — it
    /// only multiplies state size beyond `state_target_size_bytes`.
    #[test]
    fn test_growth_factor_is_inert_below_target() {
        let mut config = testnet_config();
        config.rent_fee_growth_factor = 5_000;
        let with_growth = calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        config.rent_fee_growth_factor = 0;
        let without_growth = calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        assert_eq!(with_growth, without_growth);
    }

    /// Past the target, the growth factor raises the rate above `high`.
    ///
    /// Note the growth factor is a **raw multiplier**, not a percentage —
    /// `5000` here means 5000x per target-multiple of state, not 50%.
    #[test]
    fn test_growth_factor_applies_past_target() {
        let mut config = testnet_config();
        config.soroban_state_size_bytes = Some(8_000_000_000); // 2x target
        config.rent_fee_growth_factor = 5_000;
        // past_target = 4e9 (== target), so the /target cancels:
        // post = ceil((high-low) * past_target * growth / target) = 27000 * 5000
        // rate = high + post = 10000 + 135_000_000
        assert_eq!(effective_rent_rate_1kb(&config), 135_010_000);
    }

    /// At exactly the target the growth term is zero and the rate is `high`.
    #[test]
    fn test_rate_at_target_is_high() {
        let mut config = testnet_config();
        config.soroban_state_size_bytes = Some(4_000_000_000);
        assert_eq!(effective_rent_rate_1kb(&config), 10_000);
    }

    /// A negative low rate must not produce zero rent: the protocol's 1 KB
    /// rate floor (1000 stroops) applies.
    #[test]
    fn test_negative_low_rate_is_floored() {
        let config = testnet_config();
        assert_eq!(
            effective_rent_rate_1kb(&config),
            MINIMUM_RENT_WRITE_FEE_PER_1KB
        );
        let rent = calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        assert!(rent > 0, "real testnet config must not yield zero rent");
    }

    /// The rate rises monotonically toward `high` as state approaches target.
    #[test]
    fn test_rate_interpolates_toward_high() {
        let mut config = testnet_config();
        config.soroban_state_size_bytes = Some(0);
        let at_zero = effective_rent_rate_1kb(&config);
        config.soroban_state_size_bytes = Some(4_000_000_000);
        let at_target = effective_rent_rate_1kb(&config);
        config.soroban_state_size_bytes = Some(5_400_000_000);
        let past_target = effective_rent_rate_1kb(&config);
        assert!(at_zero < at_target, "{at_zero} !< {at_target}");
        assert!(at_target < past_target, "{at_target} !< {past_target}");
    }

    /// With the state size unknown we use the floor and say so, rather than
    /// silently picking a state size.
    #[test]
    fn test_unknown_state_size_uses_floor_and_is_labelled() {
        let mut config = testnet_config();
        config.soroban_state_size_bytes = None;
        assert_eq!(
            effective_rent_rate_1kb(&config),
            MINIMUM_RENT_WRITE_FEE_PER_1KB
        );
        assert!(rate_basis(&config).contains("floor"));
    }

    #[test]
    fn test_rate_basis_labels_interpolation() {
        let mut config = testnet_config();
        assert!(rate_basis(&config).contains("interpolated"));
        config.soroban_state_size_bytes = Some(9_000_000_000);
        assert!(rate_basis(&config).contains("high+growth"));
    }

    /// Integer ceil division must match the protocol, including exact divides.
    #[test]
    fn test_div_ceil_i128() {
        assert_eq!(div_ceil_i128(10, 3), 4);
        assert_eq!(div_ceil_i128(9, 3), 3);
        assert_eq!(div_ceil_i128(0, 3), 0);
        assert_eq!(div_ceil_i128(1, 0), 1); // denominator guarded to >= 1
    }

    #[test]
    fn test_generate_forecast_structure() {
        let config = test_config();
        let entries = vec![
            (StorageTier::Instance, 512),
            (StorageTier::Persistent, 2048),
            (StorageTier::Temporary, 1024),
        ];
        let horizons = vec![30, 180, 365];

        let forecast = generate_forecast(
            &config,
            "testnet",
            "2026-09-10T00:00:00Z",
            1000,
            "unit-test",
            &entries,
            &horizons,
        );

        // 3 tiers * 3 horizons = 9 entries
        assert_eq!(forecast.entries.len(), 9);
        assert_eq!(forecast.summary.len(), 3);
        assert_eq!(forecast.network, "testnet");
        assert_eq!(forecast.config_ledger, 1000);
        assert_eq!(forecast.config_source, "unit-test");
    }

    #[test]
    fn test_format_stroops() {
        assert_eq!(format_stroops(0), "0");
        assert_eq!(format_stroops(1000), "1,000");
        assert_eq!(format_stroops(1_000_000), "1,000,000");
        assert_eq!(format_stroops(1_234_567_890), "1,234,567,890");
    }

    #[test]
    fn test_stroops_to_xlm() {
        assert!((stroops_to_xlm(10_000_000) - 1.0).abs() < 0.0001);
        assert!((stroops_to_xlm(0)).abs() < 0.0001);
    }
}
