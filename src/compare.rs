//! Historical comparison engine for Soroban contract costs.
//!
//! Compares a current cost snapshot against a baseline (previous run or
//! saved snapshot) to detect regressions. Supports configurable
//! percentage-threshold CI failure.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::BenchResult;
use crate::rent_forecast::RentForecast;

/// A saved cost snapshot for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSnapshot {
    /// Snapshot label (e.g., branch name, commit hash).
    pub label: String,
    /// Timestamp when the snapshot was taken.
    pub timestamp: String,
    /// Network name.
    pub network: String,
    /// Rent forecast data.
    pub rent_forecast: RentForecast,
    /// WASM metrics (if captured).
    pub wasm_metrics: Option<crate::wasm_metrics::WasmMetrics>,
}

/// Comparison result between baseline and current.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// Baseline label.
    pub baseline_label: String,
    /// Current label.
    pub current_label: String,
    /// Per-tier rent changes.
    pub rent_changes: Vec<RentChange>,
    /// Overall regressions detected.
    pub regressions: Vec<Regression>,
    /// Whether CI should fail based on thresholds.
    pub should_fail_ci: bool,
}

/// Change in rent cost for a specific tier and horizon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RentChange {
    /// Storage tier.
    pub tier: crate::rent_forecast::StorageTier,
    /// Time horizon in days.
    pub days: u32,
    /// Baseline rent in stroops.
    pub baseline_stroops: u64,
    /// Current rent in stroops.
    pub current_stroops: u64,
    /// Absolute change in stroops.
    pub delta_stroops: i64,
    /// Percentage change.
    pub delta_percent: f64,
}

/// A detected regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    /// Description of the regression.
    pub description: String,
    /// Severity (percentage increase).
    pub severity_percent: f64,
}

/// Threshold configuration for CI failure.
#[derive(Debug, Clone)]
pub struct ThresholdConfig {
    /// Maximum allowed rent increase percentage before CI fails.
    pub max_rent_increase_percent: f64,
    /// Maximum allowed WASM size increase percentage.
    pub max_wasm_size_increase_percent: f64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            max_rent_increase_percent: 10.0,
            max_wasm_size_increase_percent: 20.0,
        }
    }
}

/// Load a cost snapshot from a JSON file.
pub fn load_snapshot(path: &Path) -> BenchResult<CostSnapshot> {
    let data = std::fs::read_to_string(path)?;
    let snapshot: CostSnapshot = serde_json::from_str(&data)?;
    Ok(snapshot)
}

/// Save a cost snapshot to a JSON file.
pub fn save_snapshot(snapshot: &CostSnapshot, path: &Path) -> BenchResult<()> {
    let data = serde_json::to_string_pretty(snapshot)?;
    std::fs::write(path, data)?;
    Ok(())
}

/// Compare two cost snapshots and detect regressions.
#[must_use]
pub fn compare(
    baseline: &CostSnapshot,
    current: &CostSnapshot,
    thresholds: &ThresholdConfig,
) -> ComparisonResult {
    let mut rent_changes = Vec::new();
    let mut regressions = Vec::new();

    // Compare rent forecasts by matching tier + days
    let baseline_entries = &baseline.rent_forecast.entries;
    let current_entries = &current.rent_forecast.entries;

    for current_entry in current_entries {
        if let Some(baseline_entry) = baseline_entries
            .iter()
            .find(|b| b.tier == current_entry.tier && b.days == current_entry.days)
        {
            let delta = current_entry.rent_stroops as i64 - baseline_entry.rent_stroops as i64;
            let delta_percent = if baseline_entry.rent_stroops > 0 {
                (delta as f64 / baseline_entry.rent_stroops as f64) * 100.0
            } else if delta > 0 {
                100.0
            } else {
                0.0
            };

            rent_changes.push(RentChange {
                tier: current_entry.tier,
                days: current_entry.days,
                baseline_stroops: baseline_entry.rent_stroops,
                current_stroops: current_entry.rent_stroops,
                delta_stroops: delta,
                delta_percent,
            });

            // Check threshold
            if delta_percent > thresholds.max_rent_increase_percent {
                regressions.push(Regression {
                    description: format!(
                        "Rent for {} tier ({}d) increased by {:.1}% (threshold: {:.1}%)",
                        current_entry.tier,
                        current_entry.days,
                        delta_percent,
                        thresholds.max_rent_increase_percent,
                    ),
                    severity_percent: delta_percent,
                });
            }
        }
    }

    // Compare WASM size if both have it
    if let (Some(b_wasm), Some(c_wasm)) = (&baseline.wasm_metrics, &current.wasm_metrics) {
        if b_wasm.total_size > 0 {
            let size_delta = c_wasm.total_size as i64 - b_wasm.total_size as i64;
            let size_percent = (size_delta as f64 / b_wasm.total_size as f64) * 100.0;
            if size_percent > thresholds.max_wasm_size_increase_percent {
                regressions.push(Regression {
                    description: format!(
                        "WASM size increased by {:.1}% ({:.1} KB -> {:.1} KB, threshold: {:.1}%)",
                        size_percent,
                        b_wasm.total_size as f64 / 1024.0,
                        c_wasm.total_size as f64 / 1024.0,
                        thresholds.max_wasm_size_increase_percent,
                    ),
                    severity_percent: size_percent,
                });
            }
        }
    }

    ComparisonResult {
        baseline_label: baseline.label.clone(),
        current_label: current.label.clone(),
        rent_changes,
        regressions: regressions.clone(),
        should_fail_ci: !regressions.is_empty(),
    }
}

/// Format a comparison result as a Markdown table (for PR comments).
#[must_use]
pub fn format_comparison_markdown(result: &ComparisonResult) -> String {
    let mut md = String::new();

    // Status line
    if result.should_fail_ci {
        md.push_str("## ❌ Cost Regression Detected\n\n");
    } else {
        md.push_str("## ✅ No Cost Regressions\n\n");
    }

    md.push_str(&format!(
        "**Baseline:** `{}` → **Current:** `{}`\n\n",
        result.baseline_label, result.current_label
    ));

    // Rent changes table
    if !result.rent_changes.is_empty() {
        md.push_str("### Rent Changes\n\n");
        md.push_str("| Tier | Days | Baseline | Current | Delta | Change |\n");
        md.push_str("|------|------|----------|---------|-------|--------|\n");
        for change in &result.rent_changes {
            let sign = if change.delta_stroops >= 0 { "+" } else { "" };
            let emoji = if change.delta_percent > 10.0 {
                "🔴"
            } else if change.delta_percent > 0.0 {
                "🟡"
            } else {
                "🟢"
            };
            md.push_str(&format!(
                "| {} | {} | {} stroops | {} stroops | {sign}{} | {emoji} {sign}{:.1}% |\n",
                change.tier,
                change.days,
                change.baseline_stroops,
                change.current_stroops,
                change.delta_stroops,
                change.delta_percent,
            ));
        }
        md.push('\n');
    }

    // Regressions
    if !result.regressions.is_empty() {
        md.push_str("### Regressions\n\n");
        for reg in &result.regressions {
            md.push_str(&format!(
                "- ❌ {} ({:.1}%)\n",
                reg.description, reg.severity_percent
            ));
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rent_forecast::{HorizonSummary, RentForecast, RentForecastEntry, StorageTier};

    fn make_snapshot(label: &str, persistent_30d_stroops: u64) -> CostSnapshot {
        CostSnapshot {
            label: label.to_string(),
            timestamp: "2026-09-10T00:00:00Z".to_string(),
            network: "testnet".to_string(),
            rent_forecast: RentForecast {
                network: "testnet".to_string(),
                config_timestamp: "2026-09-10T00:00:00Z".to_string(),
                config_ledger: 1000,
                config_source: "unit-test".to_string(),
                effective_rent_rate_1kb: 1_267,
                rate_basis: "unit-test".to_string(),
                soroban_state_size_bytes: None,
                entries: vec![RentForecastEntry {
                    tier: StorageTier::Persistent,
                    days: 30,
                    ledgers: 30 * 17_280,
                    size_bytes: 1024,
                    rent_stroops: persistent_30d_stroops,
                    rent_xlm: persistent_30d_stroops as f64 / 10_000_000.0,
                    daily_stroops: persistent_30d_stroops / 30,
                }],
                summary: vec![HorizonSummary {
                    days: 30,
                    total_stroops: persistent_30d_stroops,
                    total_xlm: persistent_30d_stroops as f64 / 10_000_000.0,
                }],
            },
            wasm_metrics: None,
        }
    }

    #[test]
    fn test_no_regression() {
        let baseline = make_snapshot("main", 10_000);
        let current = make_snapshot("feature", 10_000);
        let result = compare(&baseline, &current, &ThresholdConfig::default());
        assert!(!result.should_fail_ci);
        assert!(result.regressions.is_empty());
    }

    #[test]
    fn test_regression_detected() {
        let baseline = make_snapshot("main", 10_000);
        let current = make_snapshot("feature", 12_000); // 20% increase
        let result = compare(&baseline, &current, &ThresholdConfig::default());
        assert!(result.should_fail_ci);
        assert_eq!(result.regressions.len(), 1);
    }

    #[test]
    fn test_decrease_no_regression() {
        let baseline = make_snapshot("main", 10_000);
        let current = make_snapshot("feature", 8_000); // 20% decrease
        let result = compare(&baseline, &current, &ThresholdConfig::default());
        assert!(!result.should_fail_ci);
    }

    #[test]
    fn test_format_comparison_markdown() {
        let baseline = make_snapshot("main", 10_000);
        let current = make_snapshot("feature", 12_000);
        let result = compare(&baseline, &current, &ThresholdConfig::default());
        let md = format_comparison_markdown(&result);
        assert!(md.contains("Cost Regression Detected"));
        assert!(md.contains("Persistent"));
    }
}
