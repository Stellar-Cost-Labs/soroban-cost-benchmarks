# Introduction

**`soroban-cost-benchmarks` forecasts Soroban storage rent, captures WASM binary
metrics, and posts a cost summary comment directly into a GitHub pull request.**

It is built on
[`soroban-cost-estimator`](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator),
which it uses as a library for RPC transport, config snapshots, and XDR decoding.

> ⚠️ **Unaudited developer tooling.** Always verify fee estimates against your target
> network before deploying a contract to mainnet.

## Where it lives

The project currently lives under a **personal account**:
[`aigbagbobila/soroban-cost-benchmarks`](https://github.com/aigbagbobila/soroban-cost-benchmarks).
A transfer to the [`Stellar-Cost-Labs`](https://github.com/Stellar-Cost-Labs) org is
planned but **has not happened** — `Stellar-Cost-Labs/soroban-cost-benchmarks` does not
exist yet. `Cargo.toml`'s `repository` field and the footer of generated PR comments
point at the *planned* org URL; the working `git remote` points at the personal account,
which is the canonical location today.

## The two lead features

### 1. Storage-rent forecasting

Rent is projected at 30, 180, and 365 days for each storage tier — Instance, Persistent,
Temporary — from the network's **real** rent rates, fetched from live RPC
(`ConfigSettingContractLedgerCostV0` and `ConfigSettingStateArchival`). Nothing is
hardcoded, and the tool will not silently fall back to placeholder numbers.

→ [Storage-rent forecasting](concepts/storage-rent-forecasting.md)

### 2. Inline PR cost comment

The bot posts a structured cost + rent forecast summary into a GitHub PR conversation,
and **edits the same comment in place** on later pushes instead of posting duplicates.
Update-in-place is proven against a real PR, not inferred.

→ [The PR comment bot](concepts/pr-comment-bot.md)

## What is *not* claimed

Historical cost tracking, regression detection, and CI-blocking thresholds already exist
elsewhere in the Soroban ecosystem. This tool ships them as supporting infrastructure for
the PR comment bot, not as headline features.

## Read this before quoting a number

Two things to know up front:

1. **Only the rent-forecast path has end-to-end live proof.** The `wasm_metrics` and
   `comparison` sections of the comment have only ever rendered from synthetic unit-test
   data.
2. **This is a first-order rent projection.** It does not model code-entry rent discounts
   or per-entry TTL-vs-size top-up fees, and it is not a substitute for
   `simulateTransaction`.

The full, itemised list is in [Limitations](limitations.md), and the captured evidence is
in [Verification](verification.md).

## Upstream dependency and a disclosed workaround

This tool works around a known bug in `soroban-cost-estimator`'s `config snapshot`
command, which reports a stale ledger. The workaround is tracked publicly as
[upstream issue #267](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator/issues/267)
and is **marked for deletion** once that issue closes.

→ [The `live-config` workaround](concepts/live-config-workaround.md)
