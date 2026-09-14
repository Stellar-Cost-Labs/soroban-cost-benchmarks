# FAQ

## Is this published on crates.io?

No. `cargo install soroban-cost-benchmarks` fails with `crate not found`. Install from
source:

```bash
git clone https://github.com/aigbagbobila/soroban-cost-benchmarks
cd soroban-cost-benchmarks && cargo install --path .
```

## Why does the repo live under a personal account?

A transfer to the `Stellar-Cost-Labs` org is planned but has not happened. The maintainer
deferred it until the documentation was finished. `Stellar-Cost-Labs/soroban-cost-benchmarks`
returns 404 today, while some metadata (`Cargo.toml`'s `repository`, comment footers)
already points at the planned org URL. The `git remote` is the canonical location.

## Is this a fork of `soroban-cost-estimator`?

No, and it is not a CLI wrapper either. It depends on the estimator **as a library** and
imports its config model, RPC client, and XDR helpers. It does not re-implement RPC
simulation or WASM parsing.

The one exception is deliberate and disclosed: `live_config.rs` re-stamps a snapshot's
ledger, working around
[upstream issue #267](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator/issues/267).
It reuses the estimator's own transport and decoding; only the ledger-stamping step
differs. It is marked for deletion once the issue closes.

## Why is the rent rate always exactly 1000 on testnet?

Because the network is currently below its state-size target, and `low = -17000` on
testnet. Working the interpolation gives `950`, and the protocol floor
(`MINIMUM_RENT_WRITE_FEE_PER_1KB = 1000`) raises it to `1000`. The rate is pinned to the
floor right now. This is not a bug — but a revision that used `low` directly *was* a bug
that produced zero rent.

## Why did my forecast come back as zeros?

If you are on an old build, that is the negative-`low` defect. Current builds floor the
rate and are covered by a regression test. If you are on a current build, check
`config_source`: `demo-config (NOT network data)` with `config_ledger: 0` means you passed
`--allow-demo`.

## Why does `pr-comment` not fetch live config like everything else?

It genuinely does not. Every other network-touching command fetches live when
`--config-snapshot` is omitted; `pr-comment` requires an explicit `--forecast <path>` and
errors without one. Run `rent-forecast --json > forecast.json` first so the numbers are
fresh, then hand the file to the bot.

## The bot posted nothing and exited 0. Why?

With `--dry-run` it prints the banner to stderr and the body to stdout, and makes no API
call — the default stdout might be swallowed by your shell. Without `--dry-run`, check
that `GITHUB_TOKEN` is set and has `Issues: write` on the *target* repository. That grant
is per-repository on the installation, so a token that works on one repo can return
`403 Resource not accessible by integration` on another.

## Why is there only one comment after multiple pushes?

That is by design. The bot finds its previous comment by the hidden marker
`<!-- soroban-cost-benchmarks bot -->` and updates it in place. The same comment ID is
printed each run. If you see two comments, you have two markers — usually from a forked
or renamed copy of the bot.

## What does `compare --markdown` do?

Nothing. Markdown is already the default output for non-JSON. The flag is accepted for
symmetry and ignored.

## Can I trust the WASM metrics section in a PR comment?

Not yet. It has only ever rendered from synthetic unit-test data, and `wasm-metrics` has
never been run against a real compiled contract in a recorded session. See
[Limitations](limitations.md).

## Is a `--config-snapshot` file as good as a live fetch?

It depends on the state size. Snapshot files carry no `LiveSorobanStateSizeWindow`, so the
state size is unknown and the rate falls back to the protocol floor, with `rate_basis`
saying so. This is
[#3](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/3).

## Does this replace `simulateTransaction`?

No. It is a first-order rent *projection*. Code-entry rent discounts and per-entry
TTL-vs-size top-up fees are not modelled, and the resource fields in a benchmark report
are zeros. Use the estimator for real simulation numbers.

## Are the numbers in these docs made up?

No. Every figure traces to a file under `tests/fixtures/` or to a live API response
recorded in [Verification](verification.md) — see the reproduction commands there.

## How do I tell whether a forecast is real or placeholder?

Look at `config_source` and `config_ledger`:

| `config_source` | Meaning |
|---|---|
| `rpc:getLedgerEntries@<ledger> (testnet)` | Live fetch from a named ledger — real |
| `snapshot:<path>` | A saved snapshot — as fresh as the file |
| `demo-config (NOT network data)`, ledger `0` | **Placeholders.** Do not quote. |
