# soroban-cost-benchmarks

**Storage-rent forecasting, WASM metrics, and CI PR comment bot** — built on
[soroban-cost-estimator](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator).

> ⚠️ This is unaudited developer tooling. Always verify fee estimates against
> your target network before mainnet deployment.

---

## Why this exists

The Stellar/Soroban ecosystem has cost-estimation and gas-metering tools, but
two specific capabilities are genuinely missing:

### 1. Storage-rent forecasting (lead feature)

Nothing in the ecosystem projects **30-day, 180-day, and 365-day rent costs**
per storage tier (Instance / Persistent / Temporary) from live network config
rates. This tool does, using real `ConfigSetting*` rent rates from
`ConfigSettingContractLedgerCostV0` and `StateArchivalV0` — no hardcoded
rates.

### 2. Inline PR comment bot (second lead feature)

No existing tool posts a **structured cost + rent forecast summary directly into
a GitHub PR conversation**. The closest alternatives require clicking through to
a separate dashboard. This tool creates/updates a comment in-place on every
push, showing rent projections and WASM metrics right in the PR diff view.

### What's NOT differentiating

Historical cost tracking, regression detection, and CI-blocking thresholds
already exist elsewhere in the Soroban ecosystem. This project provides those
as **supporting infrastructure** for the PR comment bot, not as headline
features.

---

## Quick start

### Install

```bash
cargo install soroban-cost-benchmarks
```

### Storage-rent forecast

By default the forecast fetches the network's `ConfigSetting*` entries live, so
no setup is needed:

```bash
soroban-cost-benchmarks rent-forecast \
  --entry persistent:1024 \
  --entry persistent:2048 \
  --entry temporary:512
```

Show exactly where the rates came from (ledger, per-setting provenance, the live
state size, and the resulting effective rate):

```bash
soroban-cost-benchmarks live-config
soroban-cost-benchmarks live-config --out snapshot.json   # also write the snapshot
```

Reuse a saved snapshot instead of hitting the network:

```bash
soroban-cost-benchmarks rent-forecast \
  --config-snapshot snapshot.json \
  --entry persistent:1024
```

Placeholder rates are opt-in only, and are labelled as such in every output
format:

```bash
soroban-cost-benchmarks rent-forecast --allow-demo --entry persistent:1024
# ℹ️  --allow-demo: using placeholder rent rates. These are NOT network data ...
# JSON gets:  "config_source": "demo-config (NOT network data)", "config_ledger": 0
```

### WASM metrics

```bash
soroban-cost-benchmarks wasm-metrics --wasm path/to/contract.wasm
```

### Benchmark

```bash
soroban-cost-benchmarks benchmark \
  --config-snapshot snapshot.json \
  --json
```

### Compare snapshots (CI regression detection)

```bash
soroban-cost-benchmarks compare \
  --baseline baseline.json \
  --current current.json \
  --threshold 10.0 \
  --markdown
```

### PR comment bot

```bash
# Set your GitHub token
export GITHUB_TOKEN=ghp_...

soroban-cost-benchmarks pr-comment \
  --owner myorg \
  --repo myrepo \
  --pr-number 42 \
  --forecast forecast.json \
  --wasm-metrics wasm.json \
  --comparison comparison.json
```

---

## Storage-rent formula

Rent is a two-step computation, mirroring
[`rs-soroban-env` `fees.rs`](https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs)
(`compute_rent_write_fee_per_1kb` → `rent_fee_for_size_and_ledgers`). See
[CAP-0046-12](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0046-12.md)
for the protocol background.

**Step 1 — effective rate per 1 KB**, interpolated across the Soroban state
size curve and floored:

```
multiplier = max(high - low, 0)
if state_size < state_target_size_bytes:
    rate = ceil(multiplier × state_size / state_target_size_bytes) + low
else:
    rate = high + ceil(multiplier × (state_size - target) × growth_factor / target)
rate = max(rate, 1000)          # MINIMUM_RENT_WRITE_FEE_PER_1KB
```

**Step 2 — fee for a size and a number of ledgers**, integer `ceil` division:

```
rent_fee = ceil(size_bytes × rate × ledgers) / (1024 × rate_denominator)
```

Where:
- `low`/`high` = `rent_fee1_kb_soroban_state_size_{low,high}` from
  `ConfigSettingContractLedgerCostV0`
- `growth_factor` = `soroban_state_rent_fee_growth_factor` — a **raw multiplier**
  applied only to state beyond the target, *not* a percentage, and inert while
  the network is below target
- `state_size` = mean of the on-chain `LiveSorobanStateSizeWindow` samples
- `rate_denominator` = `persistent_rent_rate_denominator` (Persistent and
  Instance) or `temp_rent_rate_denominator` (Temporary) from `StateArchivalV0`

On testnet (2026-09-12) `low = -17000`, so early versions of this tool that used
`low` directly as the rate produced **zero** rent. The floor is what makes real
output non-zero. See [`tests/fixtures/README.md`](tests/fixtures/README.md) for
the captured evidence, including the live-vs-demo comparison.

---

## Architecture

```
soroban-cost-benchmarks
├── src/
│   ├── lib.rs              # Library root
│   ├── rent_forecast.rs    # Lead feature: 30/180/365-day rent projections
│   ├── live_config.rs      # Live ConfigSetting* fetch (upstream-bug workaround)
│   ├── wasm_metrics.rs     # WASM static analysis (code/data sizes, counts)
│   ├── benchmark.rs        # Multi-scenario benchmarking (empty vs populated)
│   ├── compare.rs          # Historical comparison & regression detection
│   ├── pr_comment.rs       # GitHub PR comment bot (update-in-place)
│   ├── error.rs            # Error types
│   └── main.rs             # CLI binary
├── tests/fixtures/         # Captured real evidence (see its README)
├── docs/                   # Prepared upstream issue report
├── Cargo.toml
├── LICENSE-MIT
├── LICENSE-APACHE
├── CONTRIBUTING.md
└── SECURITY.md
```

### Dependency: soroban-cost-estimator

This project depends on [`soroban-cost-estimator`](https://crates.io/crates/soroban-cost-estimator)
as a **library** (crates.io v0.1.0), not a CLI wrapper. It imports:
- `config_snapshot::model::ConfigSnapshot` — for rent rate config data
- `config_snapshot::model::StateArchivalV0` — rent rate denominators
- `config_snapshot::model::ContractLedgerCostV0` — rent fee rates

It does **not** re-implement RPC simulation or WASM parsing from the estimator.

### Known upstream issue, and the workaround here

`soroban-cost-estimator` v0.1.0's `config snapshot` command stamps
`ConfigSnapshot.ledger` with the **max `last_modified_ledger` of the fetched
`ConfigSetting*` entries**, not the network's current ledger, so it reports the
same long-stale ledger on every run (observed: `3470630` while the live ledger
was `4635348`).

Until that is fixed, `src/live_config.rs` fetches the same entries via
`getLedgerEntries` and stamps the snapshot with the current ledger from
`getLatestLedger`. **This is a deliberate workaround for a tracked upstream bug,
not an independent reimplementation** — it reuses the estimator's own
`RpcClient`, `fetch_all_config_settings`, and XDR helpers, and only differs in
how the ledger is stamped. It should be deleted once the upstream issue closes.

The issue report is prepared in
[`docs/upstream-issue-config-snapshot-stale-ledger.md`](docs/upstream-issue-config-snapshot-stale-ledger.md)
and is not yet filed (the available token lacks `Issues: write`); the code
references it via `live_config::UPSTREAM_ISSUE_URL`.

---

## Development

```bash
# Run all tests
cargo test

# Lint
cargo clippy --all-targets --all-features

# Format
cargo fmt

# Build release
cargo build --release
```

### Clippy policy

Clippy is configured with `all` + `pedantic` at deny level, matching
`reoban-cost-estimator`'s conventions. Allowed pedantic lints are listed in
`Cargo.toml`.

---

## Comparison with existing tools

| Capability | Soroban Rent Calculator | Forge-soroban Gas Meter | soroban-cost-estimator | **This tool** |
|---|---|---|---|---|
| Single-entry rent calc | ✅ | ❌ | ✅ (via simulation) | ✅ |
| Multi-horizon forecasting | ❌ | ❌ | ❌ | ✅ |
| Per-tier projections | ❌ | ❌ | ❌ | ✅ |
| WASM binary metrics | ❌ | ❌ | ❌ | ✅ |
| PR inline comments | ❌ | ❌ | ❌ | ✅ |
| Config drift detection | ❌ | ❌ | ✅ | ✅ (via dependency) |
| RPC simulation | ❌ | ❌ | ✅ | ✅ (via dependency) |

No organizational relationship with any tool listed for comparison.

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.
