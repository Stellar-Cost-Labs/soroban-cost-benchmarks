//! Multi-scenario benchmarking for Soroban contracts.
//!
//! Compares resource consumption across different deployment scenarios:
//! - Empty state (fresh deployment, no prior storage)
//! - Populated state (contract with existing data)
//!
//! Uses `soroban-cost-estimator`'s simulation engine to capture real
//! resource metrics for each scenario.

use serde::{Deserialize, Serialize};

use crate::rent_forecast::{RentConfig, StorageTier};

/// A single benchmark scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScenario {
    /// Human-readable name for this scenario.
    pub name: String,
    /// Description of what this scenario represents.
    pub description: String,
    /// Storage entries in this scenario (tier, size_bytes).
    pub storage_entries: Vec<(StorageTier, u64)>,
}

/// Resource metrics captured from a simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetrics {
    /// CPU instructions consumed.
    pub cpu_instructions: u64,
    /// Memory bytes used.
    pub memory_bytes: u64,
    /// Number of ledger read entries.
    pub read_entries: u32,
    /// Number of ledger write entries.
    pub write_entries: u32,
    /// Bytes read from ledger.
    pub read_bytes: u64,
    /// Bytes written to ledger.
    pub write_bytes: u64,
    /// Transaction size in bytes.
    pub tx_size_bytes: u32,
    /// Minimum resource fee in stroops.
    pub min_resource_fee: u64,
}

/// Benchmark result for a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// The scenario that was benchmarked.
    pub scenario: BenchmarkScenario,
    /// Captured resource metrics.
    pub metrics: ResourceMetrics,
    /// Rent forecast for this scenario's storage.
    pub rent_forecast: Vec<crate::rent_forecast::RentForecastEntry>,
}

/// Complete benchmark report across all scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    /// Network name.
    pub network: String,
    /// Config snapshot timestamp.
    pub config_timestamp: String,
    /// Where the rent rates came from (live RPC fetch, snapshot file, or demo).
    pub config_source: String,
    /// All scenario results.
    pub results: Vec<BenchmarkResult>,
    /// Delta between first and last scenario (if multiple).
    pub delta: Option<ScenarioDelta>,
}

/// Delta between two scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDelta {
    /// CPU instructions difference.
    pub cpu_delta: i64,
    /// Read entries difference.
    pub read_entries_delta: i32,
    /// Write entries difference.
    pub write_entries_delta: i32,
    /// Fee difference in stroops.
    pub fee_delta: i64,
}

/// Define standard benchmark scenarios for a Soroban contract.
///
/// Returns a set of scenarios covering empty, minimal, and populated states.
#[must_use]
pub fn standard_scenarios() -> Vec<BenchmarkScenario> {
    vec![
        BenchmarkScenario {
            name: "empty".to_string(),
            description: "Fresh deployment, no prior storage entries".to_string(),
            storage_entries: vec![],
        },
        BenchmarkScenario {
            name: "minimal".to_string(),
            description: "Single 1 KB persistent storage entry".to_string(),
            storage_entries: vec![(StorageTier::Persistent, 1024)],
        },
        BenchmarkScenario {
            name: "populated".to_string(),
            description: "Contract with 10 persistent entries of 1 KB each".to_string(),
            storage_entries: (0..10).map(|_| (StorageTier::Persistent, 1024)).collect(),
        },
        BenchmarkScenario {
            name: "heavy".to_string(),
            description: "Contract with 100 persistent entries of 1 KB + 50 temporary entries"
                .to_string(),
            storage_entries: (0..100)
                .map(|_| (StorageTier::Persistent, 1024))
                .chain((0..50).map(|_| (StorageTier::Temporary, 512)))
                .collect(),
        },
    ]
}

/// Calculate the rent cost for a scenario across given time horizons.
#[must_use]
pub fn scenario_rent(
    config: &RentConfig,
    scenario: &BenchmarkScenario,
    horizons: &[u32],
) -> Vec<crate::rent_forecast::RentForecastEntry> {
    let mut entries = Vec::new();
    for &days in horizons {
        let mut total_stroops: u64 = 0;
        for &(tier, size_bytes) in &scenario.storage_entries {
            total_stroops += crate::rent_forecast::calculate_rent(config, tier, size_bytes, days);
        }
        let rent_xlm = crate::rent_forecast::stroops_to_xlm(total_stroops);
        let daily_stroops = if days > 0 {
            total_stroops / u64::from(days)
        } else {
            0
        };
        entries.push(crate::rent_forecast::RentForecastEntry {
            tier: StorageTier::Persistent, // Composite tier for multi-entry scenarios
            days,
            ledgers: u64::from(days) * crate::rent_forecast::LEDGERS_PER_DAY,
            size_bytes: scenario
                .storage_entries
                .iter()
                .map(|&(_, s)| s)
                .sum::<u64>(),
            rent_stroops: total_stroops,
            rent_xlm,
            daily_stroops,
        });
    }
    entries
}

/// Compute delta between two benchmark results.
#[must_use]
pub fn compute_delta(a: &ResourceMetrics, b: &ResourceMetrics) -> ScenarioDelta {
    ScenarioDelta {
        cpu_delta: b.cpu_instructions as i64 - a.cpu_instructions as i64,
        read_entries_delta: b.read_entries as i32 - a.read_entries as i32,
        write_entries_delta: b.write_entries as i32 - a.write_entries as i32,
        fee_delta: b.min_resource_fee as i64 - a.min_resource_fee as i64,
    }
}

/// Format a benchmark report as a human-readable table.
#[must_use]
pub fn format_report_table(report: &BenchmarkReport) -> String {
    use comfy_table::{Cell, Table};

    let mut table = Table::new();
    table.set_header(vec![
        "Scenario",
        "CPU instr",
        "Reads",
        "Writes",
        "Fee (stroops)",
        "Fee (XLM)",
        "30d rent",
        "180d rent",
        "365d rent",
    ]);

    for result in &report.results {
        let rent_30 = result
            .rent_forecast
            .iter()
            .find(|e| e.days == 30)
            .map_or("—".to_string(), |e| format!("{:.8}", e.rent_xlm));
        let rent_180 = result
            .rent_forecast
            .iter()
            .find(|e| e.days == 180)
            .map_or("—".to_string(), |e| format!("{:.8}", e.rent_xlm));
        let rent_365 = result
            .rent_forecast
            .iter()
            .find(|e| e.days == 365)
            .map_or("—".to_string(), |e| format!("{:.8}", e.rent_xlm));

        table.add_row(vec![
            Cell::new(&result.scenario.name),
            Cell::new(result.metrics.cpu_instructions),
            Cell::new(result.metrics.read_entries),
            Cell::new(result.metrics.write_entries),
            Cell::new(crate::rent_forecast::format_stroops(
                result.metrics.min_resource_fee,
            )),
            Cell::new(format!(
                "{:.8}",
                crate::rent_forecast::stroops_to_xlm(result.metrics.min_resource_fee)
            )),
            Cell::new(rent_30),
            Cell::new(rent_180),
            Cell::new(rent_365),
        ]);
    }

    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper — minimal RentConfig for unit tests.
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

    #[test]
    fn test_standard_scenarios_count() {
        let scenarios = standard_scenarios();
        assert_eq!(scenarios.len(), 4);
        assert_eq!(scenarios[0].name, "empty");
        assert_eq!(scenarios[3].name, "heavy");
    }

    #[test]
    fn test_empty_scenario_has_no_entries() {
        let scenarios = standard_scenarios();
        assert!(scenarios[0].storage_entries.is_empty());
    }

    #[test]
    fn test_heavy_scenario_entry_count() {
        let scenarios = standard_scenarios();
        let heavy = &scenarios[3];
        assert_eq!(heavy.storage_entries.len(), 150); // 100 + 50
    }

    #[test]
    fn test_compute_delta() {
        let a = ResourceMetrics {
            cpu_instructions: 1000,
            memory_bytes: 1024,
            read_entries: 2,
            write_entries: 1,
            read_bytes: 512,
            write_bytes: 256,
            tx_size_bytes: 128,
            min_resource_fee: 5000,
        };
        let b = ResourceMetrics {
            cpu_instructions: 1500,
            memory_bytes: 2048,
            read_entries: 3,
            write_entries: 2,
            read_bytes: 1024,
            write_bytes: 512,
            tx_size_bytes: 256,
            min_resource_fee: 7500,
        };

        let delta = compute_delta(&a, &b);
        assert_eq!(delta.cpu_delta, 500);
        assert_eq!(delta.read_entries_delta, 1);
        assert_eq!(delta.write_entries_delta, 1);
        assert_eq!(delta.fee_delta, 2500);
    }

    #[test]
    fn test_scenario_rent_empty() {
        let config = test_config();
        let scenario = &standard_scenarios()[0]; // empty
        let rent = scenario_rent(&config, scenario, &[30, 180, 365]);
        assert_eq!(rent.len(), 3);
        assert!(rent.iter().all(|e| e.rent_stroops == 0));
    }

    #[test]
    fn test_scenario_rent_populated() {
        let config = test_config();
        let scenario = &standard_scenarios()[2]; // populated: 10 x 1KB persistent
        let rent = scenario_rent(&config, scenario, &[30]);
        assert_eq!(rent.len(), 1);
        assert!(rent[0].rent_stroops > 0);
        // 10 entries should cost 10x a single entry
        let single =
            crate::rent_forecast::calculate_rent(&config, StorageTier::Persistent, 1024, 30);
        assert_eq!(rent[0].rent_stroops, single * 10);
    }
}
