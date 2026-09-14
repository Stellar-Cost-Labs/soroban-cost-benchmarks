# Limitations

This page is intentionally blunt. The rest of these docs should not read more confidently
than this page does. If you are about to quote a number from this project, read this first.

## Only the rent-forecast path has end-to-end live proof

The **only** capability proven end-to-end against live infrastructure is the
`rent-forecast` → `pr-comment` rent path. Specifically:

- `rent-forecast` against live testnet config: **proven**.
- `live-config` provenance: **proven**.
- `pr-comment` live round-trip with update-in-place: **proven**.

Everything else:

- The `wasm_metrics` and `comparison` **sections of the PR comment have only ever
  rendered from synthetic unit-test data.** No real `.wasm` file and no real comparison
  has been fed through a live post.
- `wasm-metrics` has **never** been run against a real `.wasm` artifact compiled by the
  Soroban toolchain in any recorded session. Its four unit tests generate WASM in memory
  with `wat`.
- `compare` is covered by four unit tests and has never been exercised against two real
  snapshots taken from a live network weeks apart.
- `benchmark` reports **rent projections only**. Its `ResourceMetrics` fields
  (`cpu_instructions`, `memory_bytes`, read/write entries and bytes, tx size, min resource
  fee) are hardcoded to `0`. Do not read a benchmark report as a resource measurement.
- `export` has no dedicated live run; it is a re-serialization step.

## The Soroban state size is an approximation

The state size feeding the rent rate interpolation is the **mean** of the 30
`LiveSorobanStateSizeWindow` samples. The tool records the basis it used in `rate_basis`
rather than presenting a single number as exact.

On the captured day it did not change the result: any state size below ~2.52 GB lands on
the same protocol floor. That is luck, not a guarantee.

## This is a first-order rent projection, not a protocol-accurate simulation

**Code-entry rent discounts and per-entry TTL-vs-size top-up fees are not modelled.**
The projection implements the two-step protocol rent computation and nothing more. It is
not a substitute for `simulateTransaction`, and it should not be used to predict a final
fee.

## `Cargo.lock` is gitignored

This is a binary crate, so the usual library convention of ignoring `Cargo.lock` is the
wrong default here: builds are not pinned to the dependency versions the captured evidence
was produced with. Tracked as open decision
[#8](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/8) and **not yet
resolved**.

## `compare --markdown` is accepted but ignored

Markdown is already the default for non-JSON output, so the flag currently does nothing.
It exists for symmetry.

## The bot has only run without a ruleset in force

The live PR round-trip happened against a repository with **no ruleset active**. The
`Main_ruleset` (required status checks, PR requirement, no force-push) was created
*after* that run. Whether a required-status-check ruleset interacts with a comment-posting
job is **untested**.

## The filed upstream issue carries a stale header

Issue
[#267](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator/issues/267) was opened
by pasting this repo's report file, so its body still contains the file's own front
matter — including a `Status: prepared, NOT YET FILED` line — and a
"replace `PLACEHOLDER`" instruction block. Correcting the issue requires `Issues: write`
on the sibling repository, which this environment does not have. The corrected text lives
in [Upstream issue #267](upstream-issue-config-snapshot-stale-ledger.md).

## The `live-config` workaround still exists

The workaround is not a permanent design choice; it is scaffolding around a known
upstream bug. Until #267 closes, `live_config.rs` stays, and every forecast's provenance
points at an open issue rather than a fixed dependency.

## Repository ownership

The repository lives under a **personal account**. The transfer to the `Stellar-Cost-Labs`
org is planned but has not happened, and some metadata (notably `Cargo.toml`'s
`repository` field and generated comment footers) already points at the planned org URL.
Verify the canonical location before citing a link.

## Nothing here is audited

This is unaudited developer tooling. Always verify fee estimates against your target
network before deploying a contract to mainnet.
