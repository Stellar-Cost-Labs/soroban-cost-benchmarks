# Upstream issue: `config snapshot` reports a stale ledger

**Status: prepared, NOT YET FILED.**

Filing from this environment failed:

```
$ gh issue create --repo Stellar-Cost-Labs/soroban-cost-estimator ...
GraphQL: Resource not accessible by integration (createIssue)
```

The GitHub App token in use has `Issues: write` withheld, so it cannot create
issues in the sibling repo (or in this one). Read access — including the
`permissions` block and issue/label listings — works fine.

Once the token has `Issues: write` (or a maintainer files it by hand), run the
command at the bottom of this file and replace `PLACEHOLDER` in
`src/live_config.rs::UPSTREAM_ISSUE_URL` with the returned issue number.

## Title

```
fix(config): config snapshot reports a stale ledger instead of the current one
```

## Labels

`bug`, `complexity: trivial`, `Stellar Wave`

Matches the sibling repo's existing convention (confirmed via `gh label list`:
complexity tiers are `complexity: trivial|medium|high`, and open issues carry
`Stellar Wave`).

## Body

---

## Summary

`config snapshot` stamps `ConfigSnapshot.ledger` with the **max `last_modified_ledger` across the fetched `ConfigSetting*` entries**, not the network's current ledger. Config settings only change on protocol-governance events, so that value is frozen between upgrades and every snapshot reports the same long-stale ledger.

Reported: two runs on 2026-09-11 (11:02 and 11:38 UTC) both returned `ledger: 3470630`, while Horizon and `getLatestLedger` were both in the ~4.62M range.

## Background

Root cause is in `src/main.rs`:

- `cmd_config_snapshot` populates the snapshot then does:

  ```rust
  if let Some(latest) = raw_entries.iter().map(|e| e.last_modified_ledger).max() {
      snapshot.ledger = latest;
  }
  ```

- `cmd_config_diff` uses the same pattern for `new_snapshot`.

`last_modified_ledger` answers "when did this setting last change?", not "what is the current ledger?" — and the current setting has stood at `3470630` since a governance event.

`GetLedgerEntriesResponse` already deserializes the RPC's `latestLedger` into `latest_ledger: Option<u64>` in `src/rpc/config.rs`, but nothing reads it.

Re-confirmed 2026-09-12 08:04 UTC (cross-checked from a downstream consumer):

| Source | Ledger |
|---|---|
| Horizon `/ledgers?order=desc&limit=1` | 4635340 |
| RPC `getLatestLedger` | 4635340 |
| RPC `getLedgerEntries` (same entries, current-ledger stamp) | 4635340 |
| `config snapshot` (`max last_modified_ledger`) | **3470630** (stale by 1,164,710) |

Per-setting `last_modified_ledger` values observed: ContractComputeV0 606666, ContractLedgerCostV0 3470630, ContractHistoricalDataV0 606666, ContractEventsV0 606666, ContractBandwidthV0 606666, StateArchival 2332.

Downstream impact: `soroban-cost-benchmarks` cannot use `config snapshot` output to prove a forecast is current, so it carries a workaround that reads the same entries and stamps the current ledger. Fixing this lets that workaround be deleted.

## Acceptance criteria

- [ ] After `config snapshot`, `ConfigSnapshot.ledger` equals the network's current ledger (`getLatestLedger`), not `max(last_modified_ledger)`.
- [ ] `config diff` stamps `new_snapshot.ledger` the same way.
- [ ] Per-setting `last_modified_ledger` provenance is not silently dropped.
- [ ] Two runs minutes apart return different `ledger` values, each matching Horizon within normal lag.
- [ ] A regression test asserts the snapshot ledger comes from the current ledger, not the config entries' `last_modified_ledger`.
- [ ] Lint, type-check, and tests all pass locally.

## Implementation hints

Audience: contributor.

- `src/main.rs` — `cmd_config_snapshot` and `cmd_config_diff`.
- `src/rpc/config.rs` — `GetLedgerEntriesResponse.latest_ledger` is already parsed; use it, or add one explicit `getLatestLedger` call.
- `src/xdr_helper.rs` — keep the decoding path intact; this is a stamping bug, not a decoding bug.

## Repo-specific notes

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Reproduce:
```bash
soroban-cost-estimator config snapshot --network testnet --json | jq .ledger
curl -s "https://horizon-testnet.stellar.org/ledgers?order=desc&limit=1" | jq '._embedded.records[0].sequence'
```

## Out of scope

- Changing rent or fee semantics, or any `ConfigSetting*` decoding.
- The `config diff` output format.

---

## Command to file it

```bash
gh issue create \
  --repo Stellar-Cost-Labs/soroban-cost-estimator \
  --title "fix(config): config snapshot reports a stale ledger instead of the current one" \
  --label bug --label "complexity: trivial" --label "Stellar Wave" \
  --body-file docs/upstream-issue-config-snapshot-stale-ledger.md
```

(The body above is preceded by this file's own front matter, so prefer copying
the section between the `---` markers, or let a maintainer paste it directly.)
