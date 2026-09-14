# The `live-config` workaround

This page explains a disclosed workaround. It is deliberately the most explicit page in
these docs, because the alternative — quietly reimplementing a dependency's job — would
be dishonest.

## The upstream bug

`soroban-cost-estimator` v0.1.0's `config snapshot` command stamps
`ConfigSnapshot.ledger` with the **maximum `last_modified_ledger` across the fetched
`ConfigSetting*` entries**, instead of the network's current ledger.

`ConfigSetting*` entries only change during protocol-governance events. Between upgrades
that value is frozen, so every snapshot reports the same, long-stale ledger:

```
Current ledger:     4635348   (getLatestLedger)
Upstream would say: 3470630   (max last_modified_ledger — stale by 1,164,718 ledgers)
```

`last_modified_ledger` answers *"when did this setting last change?"* — not *"what is the
current ledger?"*. The upstream `GetLedgerEntriesResponse` already parses
`latestLedger`; nothing reads it.

## The tracking issue

**Filed: [Stellar-Cost-Labs/soroban-cost-estimator#267](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator/issues/267)**

Opened 2026-09-13 by the maintainer, labelled `bug`, `complexity: trivial`,
`Stellar Wave`, and open as of 2026-09-14.

It had to be filed by hand. Automated filing fails with:

```
GraphQL: Resource not accessible by integration (createIssue)
```

The token available in this environment holds `Issues: write` on
`aigbagbobila/soroban-cost-benchmarks` but **not** on the sibling repository. That grant
is per-installation, not token-wide, so there is no command-line workaround — the app
must be granted `Issues: write` on that repository.

> **Known defect in the filed copy:** the pasted body still carries this repo's own issue
> report front matter, including a stale `Status: prepared, NOT YET FILED` line.
> Correcting the issue needs `Issues: write` on the sibling repo. The corrected report is
> the canonical text in
> [Upstream issue #267](../upstream-issue-config-snapshot-stale-ledger.md).

## What this repo does instead

`src/live_config.rs` fetches the *same* entries and stamps the snapshot with the network's
**current** ledger from `getLatestLedger` — two RPC calls, in this order:

1. `getLatestLedger` → the authoritative current ledger.
2. one batched `getLedgerEntries` → all six `ConfigSetting*` entries.

Then it stamps `snapshot.ledger = current_ledger`.

The module deliberately **reuses the estimator's own primitives** rather than
reimplementing them:

| Reused from the estimator | Purpose |
|---|---|
| `rpc::client::RpcClient` | JSON-RPC transport |
| `rpc::config::fetch_all_config_settings` | batched `getLedgerEntries` for all six settings |
| `xdr_helper` | XDR decode + snapshot population |

**Only the ledger-stamping step differs.** This is not RPC simulation, config decoding,
or WASM parsing re-implemented — it is one field set differently.

### One entry the estimator does not model

`ConfigSettingLiveSorobanStateSizeWindow` sits outside the estimator's `ConfigSettingId`
enum, so its `LedgerKey` is built locally. It is still a plain `ConfigSetting` entry read
through the estimator's RPC client. A failure to fetch it degrades the rent rate to the
protocol floor rather than failing the whole fetch.

## Evidence it works

The workaround was cross-checked three independent ways at `2026-09-12T08:04:46Z`:

| Source | Ledger |
|---|---|
| Horizon `GET /ledgers?order=desc&limit=1` | 4635340 |
| RPC `getLatestLedger` (raw `curl`) | 4635340 |
| `soroban-cost-benchmarks live-config` | 4635340 |

Exact agreement, no lag discrepancy. Against the stale value the gap was **1,164,710
ledgers** — not normal network lag. Details in
[Verification](../verification.md#three-way-ledger-cross-check).

## Exit criteria — delete this module

The workaround is **marked for deletion**, not deprecated. `live_config.rs` should be
removed and replaced with the estimator's `config snapshot` output once issue #267 is
fixed:

- `ConfigSnapshot.ledger` equals the network's current ledger;
- `config diff` stamps `new_snapshot.ledger` the same way;
- per-setting `last_modified_ledger` provenance is still available;
- a regression test asserts the snapshot ledger comes from the current ledger.

`live_config::UPSTREAM_ISSUE_URL` already points at #267, so the link is live in
`live-config` output and in every forecast's provenance.

→ [`live-config` command reference](../commands/live-config.md) ·
[Upstream issue #267](../upstream-issue-config-snapshot-stale-ledger.md)
