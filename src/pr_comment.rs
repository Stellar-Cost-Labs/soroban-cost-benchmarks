//! GitHub PR comment bot for Soroban cost summaries.
//!
//! Posts a structured cost + rent forecast summary directly inline in a
//! GitHub PR conversation. Updates the existing comment on subsequent pushes
//! rather than duplicating.

use serde::{Deserialize, Serialize};

use crate::error::BenchResult;
use crate::rent_forecast::RentForecast;

/// Marker text used to identify our bot's comment for update-in-place behavior.
const COMMENT_MARKER: &str = "<!-- soroban-cost-benchmarks bot -->";

/// Configuration for the PR comment bot.
#[derive(Debug, Clone)]
pub struct PrCommentConfig {
    /// GitHub owner (org or user).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// PR number to comment on.
    pub pr_number: u64,
    /// GitHub token for API access.
    pub token: String,
}

/// Content of a cost summary PR comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummaryComment {
    /// Title shown in the comment.
    pub title: String,
    /// The rent forecast data.
    pub rent_forecast: RentForecast,
    /// WASM metrics (optional).
    pub wasm_metrics: Option<crate::wasm_metrics::WasmMetrics>,
    /// Comparison result (optional — for differential reporting).
    pub comparison: Option<crate::compare::ComparisonResult>,
}

/// Post or update a cost summary comment on a GitHub PR.
///
/// On first invocation, creates a new comment. On subsequent pushes to the
/// same PR, finds the existing comment (by marker) and updates it in place.
///
/// # Errors
///
/// Returns `BenchError::GitHub` if the API call fails.
pub async fn post_or_update_comment(
    config: &PrCommentConfig,
    summary: &CostSummaryComment,
) -> BenchResult<u64> {
    let body = render_comment_markdown(summary);

    let octocrab = octocrab::OctocrabBuilder::default()
        .personal_token(config.token.clone())
        .build()?;

    let existing = find_marker_comment(&octocrab, config).await?;

    if let Some(comment_id) = existing {
        octocrab
            .issues(&config.owner, &config.repo)
            .update_comment(comment_id.into(), &body)
            .await?;
        Ok(comment_id)
    } else {
        let comment = octocrab
            .issues(&config.owner, &config.repo)
            .create_comment(config.pr_number, &body)
            .await?;
        Ok(*comment.id)
    }
}

/// Find an existing comment containing our marker.
async fn find_marker_comment(
    octocrab: &octocrab::Octocrab,
    config: &PrCommentConfig,
) -> BenchResult<Option<u64>> {
    let comments: Vec<octocrab::models::issues::Comment> = octocrab
        .issues(&config.owner, &config.repo)
        .list_comments(config.pr_number)
        .send()
        .await?
        .items;

    for comment in comments {
        if comment
            .body
            .as_ref()
            .is_some_and(|b| b.contains(COMMENT_MARKER))
        {
            let id: u64 = *comment.id;
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Render a cost summary as GitHub-flavored Markdown.
///
/// Includes the hidden marker comment for update-in-place behavior.
#[must_use]
pub fn render_comment_markdown(summary: &CostSummaryComment) -> String {
    let mut md = String::new();

    // Hidden marker for update-in-place
    md.push_str(COMMENT_MARKER);
    md.push('\n');

    md.push_str(&format!("## 📊 {}\n\n", summary.title));

    // Network + config info
    md.push_str(&format!(
        "**Network:** `{}` | **Config ledger:** `{}` | **Snapshot:** `{}`\n\n",
        summary.rent_forecast.network,
        summary.rent_forecast.config_ledger,
        summary.rent_forecast.config_timestamp,
    ));

    // Rent forecast table
    md.push_str("### Storage Rent Forecast\n\n");
    md.push_str("| Tier | Days | Rent (stroops) | Rent (XLM) | Daily (stroops) |\n");
    md.push_str("|------|------|----------------|------------|------------------|\n");

    let tiers = [
        crate::rent_forecast::StorageTier::Instance,
        crate::rent_forecast::StorageTier::Persistent,
        crate::rent_forecast::StorageTier::Temporary,
    ];

    for tier in &tiers {
        let entries: Vec<_> = summary
            .rent_forecast
            .entries
            .iter()
            .filter(|e| e.tier == *tier)
            .collect();
        for entry in entries {
            md.push_str(&format!(
                "| {} | {} | {} | {:.8} | {} |\n",
                entry.tier, entry.days, entry.rent_stroops, entry.rent_xlm, entry.daily_stroops,
            ));
        }
    }

    // Summary row
    md.push_str("\n**Total by horizon:**\n\n");
    for s in &summary.rent_forecast.summary {
        md.push_str(&format!(
            "- **{} days:** {} stroops ({:.8} XLM)\n",
            s.days, s.total_stroops, s.total_xlm,
        ));
    }
    md.push('\n');

    // WASM metrics (if present)
    if let Some(ref wasm) = summary.wasm_metrics {
        md.push_str("### WASM Metrics\n\n");
        md.push_str(&format!(
            "- **Total size:** {} bytes ({:.1} KB)\n",
            wasm.total_size,
            wasm.total_size as f64 / 1024.0,
        ));
        md.push_str(&format!(
            "- **Code section:** {} bytes ({:.1} KB)\n",
            wasm.code_section_size,
            wasm.code_section_size as f64 / 1024.0,
        ));
        md.push_str(&format!(
            "- **Data section:** {} bytes ({:.1} KB)\n",
            wasm.data_section_size,
            wasm.data_section_size as f64 / 1024.0,
        ));
        md.push_str(&format!("- **Functions:** {}\n", wasm.function_count));
        md.push_str(&format!(
            "- **Has contract spec:** {}\n",
            wasm.has_contract_spec,
        ));
        md.push('\n');
    }

    // Comparison (if present)
    if let Some(ref comparison) = summary.comparison {
        md.push_str(&crate::compare::format_comparison_markdown(comparison));
    }

    // Footer
    md.push_str("---\n");
    md.push_str("*Generated by [soroban-cost-benchmarks](https://github.com/Stellar-Cost-Labs/soroban-cost-benchmarks) — built on [soroban-cost-estimator](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator)*\n");

    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rent_forecast::{HorizonSummary, RentForecast, RentForecastEntry, StorageTier};

    fn test_forecast() -> RentForecast {
        RentForecast {
            network: "testnet".to_string(),
            config_timestamp: "2026-09-10T00:00:00Z".to_string(),
            config_ledger: 1000,
            entries: vec![
                RentForecastEntry {
                    tier: StorageTier::Persistent,
                    days: 30,
                    ledgers: 30 * 17_280,
                    size_bytes: 1024,
                    rent_stroops: 15_938,
                    rent_xlm: 0.001_593_8,
                    daily_stroops: 531,
                },
                RentForecastEntry {
                    tier: StorageTier::Persistent,
                    days: 180,
                    ledgers: 180 * 17_280,
                    size_bytes: 1024,
                    rent_stroops: 95_628,
                    rent_xlm: 0.009_562_8,
                    daily_stroops: 531,
                },
                RentForecastEntry {
                    tier: StorageTier::Persistent,
                    days: 365,
                    ledgers: 365 * 17_280,
                    size_bytes: 1024,
                    rent_stroops: 194_017,
                    rent_xlm: 0.019_401_7,
                    daily_stroops: 531,
                },
            ],
            summary: vec![
                HorizonSummary {
                    days: 30,
                    total_stroops: 15_938,
                    total_xlm: 0.001_593_8,
                },
                HorizonSummary {
                    days: 180,
                    total_stroops: 95_628,
                    total_xlm: 0.009_562_8,
                },
                HorizonSummary {
                    days: 365,
                    total_stroops: 194_017,
                    total_xlm: 0.019_401_7,
                },
            ],
        }
    }

    #[test]
    fn test_render_comment_markdown() {
        let summary = CostSummaryComment {
            title: "Cost Summary".to_string(),
            rent_forecast: test_forecast(),
            wasm_metrics: None,
            comparison: None,
        };

        let md = render_comment_markdown(&summary);
        assert!(md.contains(COMMENT_MARKER));
        assert!(md.contains("Storage Rent Forecast"));
        assert!(md.contains("Persistent"));
        assert!(md.contains("testnet"));
        assert!(md.contains("soroban-cost-benchmarks"));
    }

    #[test]
    fn test_render_comment_with_wasm() {
        let wasm = crate::wasm_metrics::WasmMetrics {
            path: "contract.wasm".to_string(),
            total_size: 50_000,
            code_section_size: 30_000,
            data_section_size: 10_000,
            custom_section_size: 5_000,
            function_count: 3,
            global_count: 0,
            table_count: 0,
            memory_count: 1,
            import_count: 10,
            export_count: 3,
            has_contract_spec: true,
            exported_functions: vec!["initialize".to_string(), "transfer".to_string()],
        };

        let summary = CostSummaryComment {
            title: "Cost Summary".to_string(),
            rent_forecast: test_forecast(),
            wasm_metrics: Some(wasm),
            comparison: None,
        };

        let md = render_comment_markdown(&summary);
        assert!(md.contains("WASM Metrics"));
        assert!(md.contains("50000 bytes"));
        assert!(md.contains("Has contract spec:** true"));
    }
}
