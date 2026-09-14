//! Live `ConfigSetting*` fetch straight from the network RPC.
//!
//! # This is a deliberate workaround, not a reimplementation
//!
//! `soroban-cost-estimator` v0.1.0's `config snapshot` command stamps
//! `ConfigSnapshot.ledger` with the **maximum `last_modified_ledger` across the
//! fetched `ConfigSetting*` entries**, not the network's current ledger. In its
//! `cmd_config_snapshot` (and `cmd_config_diff`) the relevant lines are:
//!
//! ```text
//! if let Some(latest) = raw_entries.iter().map(|e| e.last_modified_ledger).max() {
//!     snapshot.ledger = latest;
//! }
//! ```
//!
//! `ConfigSetting*` entries only change when the network's pricing is
//! re-parameterised, so that value is frozen between protocol-governance
//! events — every snapshot reports the same, long-stale ledger. That is the bug
//! tracked upstream at [`UPSTREAM_ISSUE_URL`].
//!
//! This module therefore fetches the *same* entries and stamps the snapshot
//! with the network's **current** ledger from `getLatestLedger`. It deliberately
//! reuses the estimator's own primitives rather than re-implementing them:
//!
//! - [`soroban_cost_estimator::rpc::client::RpcClient`] — JSON-RPC transport
//! - [`soroban_cost_estimator::rpc::config::fetch_all_config_settings`] — batched
//!   `getLedgerEntries` for all six settings
//! - [`soroban_cost_estimator::xdr_helper`] — XDR decode + snapshot population
//!
//! Only the ledger-stamping step differs. When the upstream issue is fixed,
//! this module can be deleted in favour of the estimator's `config snapshot`
//! output — see the tracking issue for the exit criteria.

use base64::Engine;
use serde::{Deserialize, Serialize};
use stellar_xdr::{
    ConfigSettingEntry, ConfigSettingId as XdrConfigSettingId, LedgerEntryData, LedgerKey,
    LedgerKeyConfigSetting, Limits, ReadXdr, WriteXdr,
};

use soroban_cost_estimator::config_snapshot::model::ConfigSnapshot;
use soroban_cost_estimator::error::AppError;
use soroban_cost_estimator::rpc::client::RpcClient;
use soroban_cost_estimator::rpc::config::fetch_all_config_settings;
use soroban_cost_estimator::xdr_helper;

use crate::error::{BenchError, BenchResult};

/// Tracking issue for the upstream `config snapshot` ledger bug.
///
/// This workaround exists only to route around that issue; delete it once the
/// issue is closed.
///
/// **Filed 2026-09-13 as issue #267** and open as of 2026-09-14. The full report
/// is kept in `docs/upstream-issue-config-snapshot-stale-ledger.md`. The issue
/// had to be opened by hand: automated filing returns
/// `GraphQL: Resource not accessible by integration (createIssue)`, because the
/// GitHub App installation behind the available token holds `Issues: write` on
/// this repository but **not** on `Stellar-Cost-Labs/soroban-cost-estimator` —
/// a per-installation grant, not a token-wide one.
pub const UPSTREAM_ISSUE_URL: &str =
    "https://github.com/Stellar-Cost-Labs/soroban-cost-estimator/issues/267";

/// Human-readable tag recorded on forecasts produced from a live fetch.
pub const LIVE_CONFIG_SOURCE_PREFIX: &str = "rpc:getLedgerEntries";

/// Human-readable tag recorded on forecasts produced from demo/placeholder config.
pub const DEMO_CONFIG_SOURCE: &str = "demo-config (NOT network data)";

/// Response shape for the RPC `getLatestLedger` method.
#[derive(Debug, Deserialize)]
struct GetLatestLedgerResult {
    /// Current ledger sequence number.
    sequence: u32,
}

/// Request payload for `getLedgerEntries`.
#[derive(Debug, Serialize)]
struct GetLedgerEntriesParams {
    keys: Vec<String>,
}

/// One entry from a `getLedgerEntries` response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LedgerEntryResult {
    xdr: String,
}

/// Response from a `getLedgerEntries` call.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetLedgerEntriesResponse {
    entries: Vec<LedgerEntryResult>,
}

/// Provenance of a single `ConfigSetting*` ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSettingProvenance {
    /// XDR enum name, e.g. `CONFIG_SETTING_CONTRACT_LEDGER_COST_V0`.
    pub setting: String,
    /// Ledger in which this setting was last modified.
    pub last_modified_ledger: u32,
}

/// Machine-readable evidence for one live config fetch.
///
/// Recorded alongside fixtures so a reader can tell exactly which ledger the
/// numbers came from, and can see the stale value the upstream command would
/// have reported instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveConfigEvidence {
    /// Network name (testnet, mainnet, ...).
    pub network: String,
    /// RPC endpoint the entries were read from.
    pub rpc_url: String,
    /// When this fetch completed (RFC 3339).
    pub fetched_at: String,
    /// Current network ledger per `getLatestLedger` — the authoritative value.
    pub current_ledger: u32,
    /// Max `last_modified_ledger` across the fetched entries.
    ///
    /// This is the value `soroban-cost-estimator`'s `config snapshot` uses for
    /// `ConfigSnapshot.ledger`, and the source of the upstream bug. Exposed here
    /// so the discrepancy is visible rather than hidden.
    pub max_last_modified_ledger: u32,
    /// Raw `LiveSorobanStateSizeWindow` samples (bytes), oldest first.
    pub state_size_window: Vec<u64>,
    /// Mean of `state_size_window`, used as the current Soroban state size in
    /// the protocol's rent-rate interpolation. `None` if the window is empty or
    /// could not be fetched.
    pub soroban_state_size_bytes: Option<i64>,
    /// Per-setting provenance.
    pub entries: Vec<ConfigSettingProvenance>,
}

impl LiveConfigEvidence {
    /// How far behind the current ledger the upstream-stamped value is.
    #[must_use]
    pub fn stale_lag_ledgers(&self) -> u32 {
        self.current_ledger
            .saturating_sub(self.max_last_modified_ledger)
    }

    /// The `config_source` tag recorded on forecasts built from this fetch.
    #[must_use]
    pub fn source_tag(&self) -> String {
        format!(
            "{LIVE_CONFIG_SOURCE_PREFIX}@{} ({})",
            self.current_ledger, self.network
        )
    }
}

/// A live config snapshot together with the evidence proving where it came from.
#[derive(Debug, Clone)]
pub struct LiveConfig {
    /// Snapshot populated from live `ConfigSetting*` entries, stamped with the
    /// current ledger.
    pub snapshot: ConfigSnapshot,
    /// Provenance evidence for this fetch.
    pub evidence: LiveConfigEvidence,
}

/// Mean of a `LiveSorobanStateSizeWindow`, or `None` when it is empty.
#[must_use]
pub fn mean_state_size(window: &[u64]) -> Option<i64> {
    if window.is_empty() {
        return None;
    }
    let sum: i128 = window.iter().map(|v| i128::from(*v)).sum();
    i64::try_from(sum / window.len() as i128).ok()
}

/// Fetch the `LiveSorobanStateSizeWindow` config setting.
///
/// This setting is **not** modelled by `soroban-cost-estimator` (its
/// `ConfigSettingId` enum stops at `StateArchival`), so the `LedgerKey` is built
/// here. It is still a plain `ConfigSetting` ledger entry read through the
/// estimator's RPC client — not a reimplementation of anything the estimator
/// does.
///
/// # Errors
///
/// Returns [`BenchError::LiveConfig`] if the key cannot be encoded, the RPC call
/// fails, or the entry cannot be decoded.
async fn fetch_state_size_window(client: &RpcClient) -> BenchResult<Vec<u64>> {
    let key = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
        config_setting_id: XdrConfigSettingId::LiveSorobanStateSizeWindow,
    });
    let key_xdr = key
        .to_xdr(Limits::none())
        .map_err(|e| BenchError::LiveConfig(format!("encode LedgerKey: {e}")))?;
    let key_b64 = base64::engine::general_purpose::STANDARD.encode(key_xdr);

    let params = GetLedgerEntriesParams {
        keys: vec![key_b64],
    };
    let response: GetLedgerEntriesResponse = client
        .call("getLedgerEntries", serde_json::to_value(params)?)
        .await
        .map_err(|e| estimator_err(&e))?;

    let entry = response.entries.into_iter().next().ok_or_else(|| {
        BenchError::LiveConfig("LiveSorobanStateSizeWindow not present on this network".to_string())
    })?;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&entry.xdr)
        .map_err(|e| BenchError::LiveConfig(format!("base64 decode window: {e}")))?;
    let data = LedgerEntryData::from_xdr(&bytes, Limits::none())
        .map_err(|e| BenchError::LiveConfig(format!("LedgerEntryData from_xdr: {e}")))?;

    match data {
        LedgerEntryData::ConfigSetting(ConfigSettingEntry::LiveSorobanStateSizeWindow(window)) => {
            Ok(window.to_vec())
        }
        other => Err(BenchError::LiveConfig(format!(
            "expected LiveSorobanStateSizeWindow, got {}",
            other.name()
        ))),
    }
}

/// Resolve a network name to its Soroban RPC endpoint.
///
/// Delegates to the estimator so endpoint knowledge lives in one place.
///
/// # Errors
///
/// Returns [`BenchError::Estimator`] for an unknown network name.
pub fn resolve_rpc_url(network: &str, custom_url: Option<&str>) -> BenchResult<String> {
    soroban_cost_estimator::rpc::client::resolve_endpoint(network, custom_url)
        .map_err(|e| estimator_err(&e))
}

/// Fetch all `ConfigSetting*` entries and stamp the snapshot with the network's
/// **current** ledger.
///
/// Uses two RPC calls: `getLatestLedger` for the authoritative current ledger,
/// then a single batched `getLedgerEntries` for all six settings.
///
/// # Errors
///
/// Returns [`BenchError::Estimator`] if either RPC call fails or any setting is
/// missing from the network.
pub async fn fetch_live_config(network: &str, custom_url: Option<&str>) -> BenchResult<LiveConfig> {
    let rpc_url = resolve_rpc_url(network, custom_url)?;
    let client = RpcClient::new(&rpc_url);

    let current_ledger: u32 = client
        .call::<GetLatestLedgerResult>("getLatestLedger", serde_json::json!({}))
        .await
        .map_err(|e| estimator_err(&e))?
        .sequence;

    let raw_entries = fetch_all_config_settings(&client)
        .await
        .map_err(|e| estimator_err(&e))?;

    let mut snapshot = xdr_helper::begin_snapshot(network, current_ledger);
    let mut entries = Vec::with_capacity(raw_entries.len());
    for raw in &raw_entries {
        let decoded =
            xdr_helper::decode_config_entry_xdr(&raw.config_xdr).map_err(|e| estimator_err(&e))?;
        xdr_helper::apply_config_entry(&mut snapshot, decoded);
        entries.push(ConfigSettingProvenance {
            setting: raw.id.human_name().to_string(),
            last_modified_ledger: raw.last_modified_ledger,
        });
    }

    let max_last_modified_ledger = entries
        .iter()
        .map(|e| e.last_modified_ledger)
        .max()
        .unwrap_or(0);

    // The state-size window needs its own entry (it is not part of
    // ConfigSnapshot); a failure here degrades the rate to the floor rather
    // than failing the whole fetch.
    let state_size_window = fetch_state_size_window(&client).await.unwrap_or_default();
    let soroban_state_size_bytes = mean_state_size(&state_size_window);

    // The whole point of the workaround: stamp the *current* ledger, not the
    // stale `last_modified_ledger` the upstream command uses.
    snapshot.ledger = current_ledger;

    let evidence = LiveConfigEvidence {
        network: network.to_string(),
        rpc_url,
        fetched_at: snapshot.timestamp.clone(),
        current_ledger,
        max_last_modified_ledger,
        state_size_window,
        soroban_state_size_bytes,
        entries,
    };

    Ok(LiveConfig { snapshot, evidence })
}

/// Format fetch evidence as a human-readable report.
#[must_use]
pub fn format_evidence(evidence: &LiveConfigEvidence) -> String {
    let mut out = String::new();
    out.push_str(&format!("Network:            {}\n", evidence.network));
    out.push_str(&format!("RPC endpoint:       {}\n", evidence.rpc_url));
    out.push_str(&format!("Fetched at:         {}\n", evidence.fetched_at));
    out.push_str(&format!(
        "Current ledger:     {}   (getLatestLedger)\n",
        evidence.current_ledger
    ));
    out.push_str(&format!(
        "Upstream would say: {}   (max last_modified_ledger — stale by {} ledgers)\n",
        evidence.max_last_modified_ledger,
        evidence.stale_lag_ledgers()
    ));
    out.push_str("\nConfigSetting* entry provenance:\n");
    for entry in &evidence.entries {
        out.push_str(&format!(
            "  {:<52} last modified @ {}\n",
            entry.setting, entry.last_modified_ledger
        ));
    }

    out.push_str(&format!(
        "\nLiveSorobanStateSizeWindow samples: {}\n",
        evidence.state_size_window.len()
    ));
    if let Some(first) = evidence.state_size_window.first() {
        out.push_str(&format!("  oldest sample: {} bytes\n", first));
    }
    if let Some(last) = evidence.state_size_window.last() {
        out.push_str(&format!("  newest sample: {} bytes\n", last));
    }
    match evidence.soroban_state_size_bytes {
        Some(mean) => out.push_str(&format!("  mean (used for rent rate): {} bytes\n", mean)),
        None => out.push_str("  mean (used for rent rate): unknown\n"),
    }
    out
}

/// Convert an estimator error into our error type.
fn estimator_err(error: &AppError) -> BenchError {
    BenchError::Estimator(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> LiveConfigEvidence {
        LiveConfigEvidence {
            network: "testnet".to_string(),
            rpc_url: "https://soroban-testnet.stellar.org".to_string(),
            fetched_at: "2026-09-12T00:00:00Z".to_string(),
            current_ledger: 4_635_235,
            max_last_modified_ledger: 3_470_630,
            state_size_window: vec![1_000, 2_000, 3_000],
            soroban_state_size_bytes: Some(2_000),
            entries: vec![ConfigSettingProvenance {
                setting: "CONFIG_SETTING_STATE_ARCHIVAL".to_string(),
                last_modified_ledger: 3_470_630,
            }],
        }
    }

    #[test]
    fn test_mean_state_size() {
        assert_eq!(mean_state_size(&[]), None);
        assert_eq!(mean_state_size(&[10]), Some(10));
        assert_eq!(mean_state_size(&[10, 20, 30]), Some(20));
        // Integer mean truncates, matching the doc comment.
        assert_eq!(mean_state_size(&[1, 2]), Some(1));
    }

    #[test]
    fn test_stale_lag_detects_the_upstream_gap() {
        assert_eq!(evidence().stale_lag_ledgers(), 1_164_605);
    }

    #[test]
    fn test_stale_lag_saturates_when_current_is_older() {
        let mut e = evidence();
        e.current_ledger = 1;
        e.max_last_modified_ledger = 3_470_630;
        assert_eq!(e.stale_lag_ledgers(), 0);
    }

    #[test]
    fn test_source_tag_records_ledger_and_network() {
        let tag = evidence().source_tag();
        assert!(tag.contains("4635235"), "got {tag}");
        assert!(tag.contains("testnet"), "got {tag}");
        assert!(tag.starts_with(LIVE_CONFIG_SOURCE_PREFIX));
    }

    #[test]
    fn test_resolve_rpc_url_known_networks() {
        assert_eq!(
            resolve_rpc_url("testnet", None).unwrap(),
            "https://soroban-testnet.stellar.org"
        );
        assert_eq!(
            resolve_rpc_url("mainnet", None).unwrap(),
            "https://soroban.stellar.org"
        );
        assert!(resolve_rpc_url("nope", None).is_err());
        assert_eq!(
            resolve_rpc_url("testnet", Some("http://localhost:8000")).unwrap(),
            "http://localhost:8000"
        );
    }

    #[test]
    fn test_format_evidence_shows_both_ledgers() {
        let text = format_evidence(&evidence());
        assert!(text.contains("4635235"));
        assert!(text.contains("3470630"));
        assert!(text.contains("stale by 1164605 ledgers"));
    }
}
