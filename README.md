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

```bash
# Using a config snapshot from soroban-cost-estimator
soroban-cost-estimator config snapshot --network testnet
soroban-cost-benchmarks rent-forecast \
  --config-snapshot ~/.soroban-cost-estimator/snapshots/testnet-latest.json \
  --entry persistent:1024 \
  --entry persistent:2048 \
  --entry temporary:512

# Demo mode (no network required)
soroban-cost-benchmarks rent-forecast \
  --entry persistent:1024 \
  --entry temporary:512
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

From [CAP-0046-12](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0046-12.md):

```
rent_fee = (size_bytes × ledgers_in_period × rent_rate) / (1024 × rate_denominator)
```

Where:
- `rent_rate` = `rent_fee1_kb_soroban_state_size_low` (conservative default)
- `rate_denominator` = `persistent_rent_rate_denominator` or
  `temp_rent_rate_denominator` from `StateArchivalV0`
- Growth factor applied as: `effective_rate = rent_rate × (1 + growth_factor / 10000)`

---

## Architecture

```
soroban-cost-benchmarks
├── src/
│   ├── lib.rs              # Library root
│   ├── rent_forecast.rs    # Lead feature: 30/180/365-day rent projections
│   ├── wasm_metrics.rs     # WASM static analysis (code/data sizes, counts)
│   ├── benchmark.rs        # Multi-scenario benchmarking (empty vs populated)
│   ├── compare.rs          # Historical comparison & regression detection
│   ├── pr_comment.rs       # GitHub PR comment bot (update-in-place)
│   ├── error.rs            # Error types
│   └── main.rs             # CLI binary
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
