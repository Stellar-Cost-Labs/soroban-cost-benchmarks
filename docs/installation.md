# Installation

## Prerequisites

| Requirement | Why |
|---|---|
| **Rust 1.85+** | Edition 2024 (`rust-version = "1.85"` in `Cargo.toml`) |
| **Network access to a Soroban RPC endpoint** | Needed by `rent-forecast`, `live-config`, and `benchmark` unless you pass `--config-snapshot` |
| **A GitHub token with `Issues: write`** on the target repo | Only for `pr-comment` without `--dry-run` |

A token is **not** needed for `pr-comment --dry-run`, which prints the comment body and
makes no API call.

## From source

The crate is **not published on crates.io yet**. `cargo install soroban-cost-benchmarks`
will fail with `crate not found` until the first release; install from the repository
instead:

```bash
git clone https://github.com/aigbagbobila/soroban-cost-benchmarks
cd soroban-cost-benchmarks
cargo install --path .
```

To build without installing:

```bash
cargo build --release
./target/release/soroban-cost-benchmarks --help
```

## Default RPC endpoints

Endpoints are resolved by `soroban-cost-estimator`, so endpoint knowledge lives in one
place.

| Network | Endpoint |
|---|---|
| `testnet` | `https://soroban-testnet.stellar.org` |
| `mainnet` | `https://soroban.stellar.org` |
| `futurenet` | `https://rpc-futurenet.stellar.org` |

Select a network with the global `--network` flag (default `testnet`), or override the
endpoint entirely with `--rpc-url`:

```bash
soroban-cost-benchmarks rent-forecast --network mainnet --entry persistent:1024
soroban-cost-benchmarks rent-forecast --rpc-url http://localhost:8000 --entry persistent:1024
```

## Set up the PR comment bot

```bash
export GITHUB_TOKEN=ghp_...   # token with Issues: write on the target repository
```

The bot needs `Issues: write` to create and update PR comments. That permission is
**per-repository on the token's installation** — a token that can comment on one
repository cannot necessarily comment on another. Store the token as a CI secret;
never commit it. See [SECURITY.md](https://github.com/aigbagbobila/soroban-cost-benchmarks/blob/main/SECURITY.md).

## Verify the install

There is no network round-trip in this check:

```bash
soroban-cost-benchmarks pr-comment --dry-run \
  --owner example --repo example --pr-number 1 \
  --forecast forecast.json
```

Any error here is about the missing `forecast.json`, not about GitHub credentials.

Next: [Storage-rent forecasting](concepts/storage-rent-forecasting.md) ·
[Command reference](commands/rent-forecast.md)
