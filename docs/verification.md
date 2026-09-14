# Verification

Everything on this page is **captured real output**, not illustration. Raw files are
archived in
[`tests/fixtures/`](https://github.com/aigbagbobila/soroban-cost-benchmarks/tree/main/tests/fixtures),
with the full reproduction record in
[`tests/fixtures/README.md`](https://github.com/aigbagbobila/soroban-cost-benchmarks/blob/main/tests/fixtures/README.md).
Nothing here was hand-written.

Unless stated otherwise, captures are from **2026-09-12**, against Stellar **testnet**.

## Rent forecast

### Provenance

```
config_ledger:  4635349
config_source:  rpc:getLedgerEntries@4635349 (testnet)
rate_basis:     interpolated (state 2659201588 < target 4000000000)
```

The `config_source` string is the point: the rates came over RPC from a named ledger, not
from a placeholder. The `live-config` capture immediately before it recorded current
ledger `4635348`.

### Live vs. demo — no silent fallback

The same entries and horizons were run twice, once live and once with `--allow-demo`, and
compared:

| Tier | Days | Size (B) | Live (stroops) | Demo (stroops) | Live/demo |
|---|---|---:|---:|---:|---:|
| Persistent | 30 | 1024 | 426,667 | 160,355 | 2.66 |
| Persistent | 180 | 1024 | 2,560,000 | 962,129 | 2.66 |
| Persistent | 365 | 1024 | 5,191,112 | 1,950,983 | 2.66 |
| Persistent | 30 | 2048 | 853,334 | 320,710 | 2.66 |
| Persistent | 180 | 2048 | 5,120,000 | 1,924,257 | 2.66 |
| Persistent | 365 | 2048 | 10,382,223 | 3,901,965 | 2.66 |
| Temporary | 30 | 512 | 106,667 | 80,178 | 1.33 |
| Temporary | 180 | 512 | 640,000 | 481,065 | 1.33 |
| Temporary | 365 | 512 | 1,297,778 | 975,492 | 1.33 |

Totals:

| Horizon | Live (stroops) | Demo (stroops) |
|---|---:|---:|
| 30 days | 1,386,668 | 561,243 |
| 180 days | 8,320,000 | 3,367,451 |
| 365 days | 16,871,113 | 6,828,440 |

The two differ by **1.33× to 2.66×**, which is the evidence that live mode is not
secretly serving demo values. Demo rates understate real testnet persistent-storage rent
by more than half.

### Reproduce

```bash
soroban-cost-benchmarks live-config --out tests/fixtures/testnet-config-live.json

soroban-cost-benchmarks rent-forecast \
  --entry persistent:1024 --entry persistent:2048 --entry temporary:512 --json \
  > tests/fixtures/forecast-live-testnet.json

soroban-cost-benchmarks rent-forecast --allow-demo \
  --entry persistent:1024 --entry persistent:2048 --entry temporary:512 --json \
  > tests/fixtures/forecast-demo.json
```

The stderr banners of each run are archived as `forecast-live-stderr.txt` and
`forecast-demo-stderr.txt`.

## Three-way ledger cross-check

Taken at `2026-09-12T08:04:46Z`, back to back:

| Source | Ledger |
|---|---|
| Horizon `GET /ledgers?order=desc&limit=1` | **4635340** |
| RPC `getLatestLedger` (raw `curl`) | **4635340** |
| `soroban-cost-benchmarks live-config` | **4635340** |

Exact agreement — no lag discrepancy at all. Horizon's `closed_at` for that ledger was
`2026-09-12T08:04:47Z`.

Against the stale value (`3470630`) the gap is **1,164,710 ledgers**, which is not normal
network lag.

> Note: `/history_latest_ledger` is not a valid Horizon path and returns 404. The
> equivalent is `/ledgers?order=desc&limit=1`.

## The formula defect this exercise exposed

The first live run produced **all zeros**, while demo mode produced plausible non-zero
numbers — the dangerous direction to fail in.

Cause: the original `calculate_rent` used `rent_fee1_kb_soroban_state_size_low` directly
as the rate. On testnet that field is **`-17000`**, so the fee came out negative and
saturated to zero. The protocol's real rate is a two-step computation, not the raw low
value:

```
state size (mean of LiveSorobanStateSizeWindow, 30 samples) = 2,659,201,588 bytes
target                                                      = 4,000,000,000 bytes
low / high / growth                                         = -17000 / 10000 / 5000
interpolated                                                = ceil(27000 × 2659201588 / 4e9) = 17950
17950 + (-17000)                                            = 950
max(950, 1000)                                              = 1000   ← effective rate
```

The rate is currently pinned to the protocol floor. Two consequences were recorded:

- A previous README claim, `effective_rate = rent_rate × (1 + growth_factor / 10000)`, was
  **wrong**. `soroban_state_rent_fee_growth_factor` is a raw multiplier applied only to
  state *beyond* the target, and has no effect at all while the network is below target —
  which testnet is.
- The state size is not part of `ConfigSnapshot`. It lives in a separate
  `LiveSorobanStateSizeWindow` entry the estimator does not model, so it is fetched
  directly. The **mean** of the 30 samples is used: an approximation, and labelled as one
  in `rate_basis` rather than presented as exact.

## Demo mode is opt-in

Previously, omitting `--config-snapshot` silently produced plausible-looking placeholder
numbers with only a warning. That foot-gun is closed:

- Default behaviour with no `--config-snapshot` is a **live fetch**.
- A failed live fetch **errors out** and refuses to fall back, printing the three options
  (`--config-snapshot`, `--allow-demo`, `--rpc-url`).
- Placeholder numbers require `--allow-demo`, print a `⚠️` banner, and are stamped
  `config_source: demo-config (NOT network data)` with `config_ledger: 0` in both table
  and JSON output.

## PR comment bot

Captured **2026-09-13** against a throwaway PR (`#1`) on
`aigbagbobila/soroban-cost-benchmarks`, driven by a fresh live forecast (testnet ledger
`4652629`, not demo data).

### Round-trip

```
RUN 1   Comment posted/updated: ID 5652127269
        created_at 2026-09-13T08:08:27Z   updated_at 2026-09-13T08:08:27Z

RUN 2   Comment posted/updated: ID 5652127269
        created_at 2026-09-13T08:08:27Z   updated_at 2026-09-13T08:08:37Z
        comment count on PR #1: 1
```

Same comment ID across both runs, `created_at` unchanged, `updated_at` advanced, and the
count stayed at **1**. The second run edited the existing comment rather than posting a
duplicate — update-in-place is **proven, not inferred**.

The exact posted body is preserved verbatim in
[`tests/fixtures/pr-comment-posted-body.md`](https://github.com/aigbagbobila/soroban-cost-benchmarks/blob/main/tests/fixtures/pr-comment-posted-body.md),
and can be re-checked against the live API:

```bash
gh api repos/aigbagbobila/soroban-cost-benchmarks/issues/1/comments \
  --jq '.[] | {id, created_at, updated_at, user: .user.login}'
```

### Permission result — measured, not inferred

- `gh pr create` succeeded → `Pull requests: write` works.
- Comment create **and** update succeeded → `Issues: write` works on this repository.

Both are **per-repository on the installation**, not token-wide. The same token returns
`HTTP 403: Resource not accessible by integration` for issue and label writes on
`Stellar-Cost-Labs/soroban-cost-estimator`.

### Cleanup

PR #1 is closed, the throwaway branch was deleted, and the throwaway file was removed.

### The rustls defect

The first live call panicked **before any HTTP request was sent**:

```
thread 'main' panicked at .../rustls-0.23.44/src/crypto/mod.rs:249:14:
Could not automatically determine the process-level CryptoProvider from Rustls
crate features.
```

Cause: `octocrab`'s default features enable `rustls-ring`, while
`soroban-cost-estimator`'s `reqwest` enables `rustls` with `aws-lc-rs`. With **two**
providers in the dependency graph, rustls 0.23 refuses to pick a process-level default.
`reqwest` was unaffected because it builds its TLS config with an explicit provider;
`octocrab` relies on the process default.

Fixed in `Cargo.toml` by selecting `rustls-aws-lc-rs` for `octocrab` (restating its other
defaults), leaving exactly one provider compiled in. Verified with
`cargo tree -e features -i rustls`: `aws-lc-rs` only, no `ring` feature.

This is precisely why the unit tests could not catch it: **they never open a socket**.

## CI

The latest run on `main` (run `34748180579`, 2026-09-13) passed all four jobs:

| Job | Result |
|---|---|
| `Formatting` | success |
| `Clippy` | success |
| `Tests` | success |
| `Build` | success |

```bash
gh run list --repo aigbagbobila/soroban-cost-benchmarks --limit 5
```

## What is still not verified

See [Limitations](limitations.md) for the itemised list. In short: the `wasm_metrics` and
`comparison` comment sections, and `compare` itself, have only ever been exercised by
unit tests with synthetic data.
