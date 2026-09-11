//! Storage-rent forecasting for Soroban contracts.
//!
//! Projects 30-day, 180-day, and 365-day rent costs per storage tier
//! (Instance, Persistent, Temporary) using real `ConfigSetting*` rent rates
//! from the network — no hardcoded rates.
//!
//! # Rent Formula
//!
//! From CAP-0046-12 and the Stellar protocol:
//!
//! ```text
//! rent_fee = (size_bytes * ledgers_in_period * rent_rate) / (1024 * rate_denominator)
//! ```
//!
//! Where:
//! - `rent_rate` is derived from `rent_fee1_kb_soroban_state_size_{low,high}` and
//!   `soroban_state_rent_fee_growth_factor`
//! - `rate_denominator` is `persistent_rent_rate_denominator` or
//!   `temp_rent_rate_denominator` from `StateArchivalV0`

use serde::{Deserialize, Serialize};

use crate::error::{BenchError, BenchResult};

/// Ledgers per day on Stellar (approximate, from protocol spec).
/// Real value varies; 17,280 is the typical rate (~5s per ledger).
pub const LEDGERS_PER_DAY: u64 = 17_280;

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
    /// Low watermark rent fee per 1 KB.
    pub rent_fee_1kb_low: i64,
    /// High watermark rent fee per 1 KB.
    pub rent_fee_1kb_high: i64,
    /// Growth factor for rent fee calculation.
    pub rent_fee_growth_factor: u32,
}

/// Extract rent config from a `ConfigSnapshot`.
///
/// # Errors
///
/// Returns `BenchError::MissingConfig` if required config sections are absent.
pub fn extract_rent_config(
    snapshot: &soroban_cost_estimator::config_snapshot::model::ConfigSnapshot,
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
    })
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

    // Calculate the effective rent rate per 1KB using the low/high watermarks
    // and growth factor. For a first approximation, use the low watermark.
    // The actual protocol uses the median rent watermark from bucket list
    // sampling, but low watermark is a conservative default.
    let rent_rate_1kb = config.rent_fee_1kb_low;

    // Apply growth factor (compounding over time).
    // The growth factor is applied as a linear multiplier in the protocol:
    // effective_rate = rent_rate * (1 + growth_factor/10000)
    let growth = f64::from(config.rent_fee_growth_factor) / 10_000.0;
    let effective_rate = (rent_rate_1kb as f64) * (1.0 + growth);

    // rent_fee = (size_bytes * ledgers * effective_rate) / (1024 * rate_denominator)
    let numerator = (size_bytes as f64) * (ledgers as f64) * effective_rate;
    let denominator = 1024.0 * (rate_denominator as f64);

    // Round up to nearest stroop (protocol rounds up).
    (numerator / denominator).ceil() as u64
}

/// Generate a full rent forecast for a set of storage entries.
///
/// # Arguments
///
/// * `config` - Network config rates.
/// * `network` - Network name (for metadata).
/// * `config_timestamp` - When the config was snapshot.
/// * `config_ledger` - Ledger height at snapshot.
/// * `entries` - List of `(tier, size_bytes)` pairs representing the contract's
///   storage footprint.
/// * `horizons` - Time horizons to forecast (e.g., [30, 180, 365]).
pub fn generate_forecast(
    config: &RentConfig,
    network: &str,
    config_timestamp: &str,
    config_ledger: u32,
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
    fn test_config() -> RentConfig {
        RentConfig {
            persistent_rent_rate_denominator: 4096,
            temp_rent_rate_denominator: 4096,
            rent_fee_1kb_low: 1_267,
            rent_fee_1kb_high: 1_267,
            rent_fee_growth_factor: 0,
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

    #[test]
    fn test_calculate_rent_growth_factor() {
        let mut config = test_config();
        config.rent_fee_growth_factor = 1000; // 10% growth
        let rent_no_growth = {
            let mut c = test_config();
            c.rent_fee_growth_factor = 0;
            calculate_rent(&c, StorageTier::Persistent, 1024, 30)
        };
        let rent_with_growth = calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        // With 10% growth, rent should be ~10% higher
        let ratio = f64::from(u32::try_from(rent_with_growth).unwrap())
            / f64::from(u32::try_from(rent_no_growth).unwrap());
        assert!(
            (ratio - 1.1).abs() < 0.01,
            "Growth factor should increase rent by ~10%, got ratio {ratio}"
        );
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
            &entries,
            &horizons,
        );

        // 3 tiers * 3 horizons = 9 entries
        assert_eq!(forecast.entries.len(), 9);
        assert_eq!(forecast.summary.len(), 3);
        assert_eq!(forecast.network, "testnet");
        assert_eq!(forecast.config_ledger, 1000);
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
