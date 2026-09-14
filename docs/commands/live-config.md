# `live-config`

Fetch all `ConfigSetting*` ledger entries live and report exactly where the rent rates
came from. This command *is* the workaround for the upstream stale-ledger bug.

## Usage

```
soroban-cost-benchmarks live-config [--out <path>]
```

| Option | Meaning |
|---|---|
| `-o, --out <path>` | Also write the fetched `ConfigSnapshot` as JSON. |

Global flags apply: `--network`, `--rpc-url`, `-v`.

## What it prints

```
Snapshot written to: tests/fixtures/testnet-config-live.json
Network:            testnet
RPC endpoint:       https://soroban-testnet.stellar.org
Fetched at:         2026-09-12T08:05:32.251565729+00:00
Current ledger:     4635348   (getLatestLedger)
Upstream would say: 3470630   (max last_modified_ledger — stale by 1164718 ledgers)

ConfigSetting* entry provenance:
  CONFIG_SETTING_CONTRACT_COMPUTE_V0                   last modified @ 606666
  CONFIG_SETTING_CONTRACT_LEDGER_COST_V0               last modified @ 3470630
  CONFIG_SETTING_CONTRACT_HISTORICAL_DATA_V0           last modified @ 606666
  CONFIG_SETTING_CONTRACT_EVENTS_V0                    last modified @ 606666
  CONFIG_SETTING_CONTRACT_BANDWIDTH_V0                 last modified @ 606666
  CONFIG_SETTING_STATE_ARCHIVAL                        last modified @ 2332

LiveSorobanStateSizeWindow samples: 30
  oldest sample: 2657058556 bytes
  newest sample: 2662087063 bytes
  mean (used for rent rate): 2659201588 bytes

Effective rent rate per 1 KB: 1000 stroops  [interpolated (state 2659201588 < target 4000000000)]

Rent rates extracted from this fetch:
  persistent_rent_rate_denominator: 1215
  temp_rent_rate_denominator:       2430
  rent_fee_1kb_low:                 -17000
  rent_fee_1kb_high:                10000
  rent_fee_growth_factor:           5000

Workaround for: https://github.com/Stellar-Cost-Labs/soroban-cost-estimator/issues/267
```

Two RPC calls are made: `getLatestLedger` for the authoritative current ledger, then one
batched `getLedgerEntries` for all six settings, plus one for the state-size window.

## Why the "Upstream would say" line is there

Both ledgers are printed on purpose. `3470630` is the value the estimator's
`config snapshot` would stamp — the max `last_modified_ledger` across settings, frozen
between governance events — and `4635348` is the real current ledger. Printing both makes
the discrepancy visible instead of hidden, and lets a reader audit the workaround.

## Use it to produce a reusable snapshot

```bash
soroban-cost-benchmarks live-config --out tests/fixtures/testnet-config-live.json
soroban-cost-benchmarks rent-forecast \
  --config-snapshot tests/fixtures/testnet-config-live.json --entry persistent:1024
```

Note that a snapshot file carries no state-size window, so `rent-forecast` reading one
falls back to the protocol floor.

→ [The `live-config` workaround](../concepts/live-config-workaround.md) ·
[Upstream issue #267](../upstream-issue-config-snapshot-stale-ledger.md)
