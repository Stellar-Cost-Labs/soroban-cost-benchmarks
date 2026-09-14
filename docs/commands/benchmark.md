# `benchmark`

Compute rent across a set of standard storage-footprint scenarios, or one custom
footprint.

## Usage

```
soroban-cost-benchmarks benchmark [--config-snapshot <path>] [-e <tier:bytes>]... [--json] [-o <path>]
```

| Option | Meaning |
|---|---|
| `--config-snapshot <path>` | Read rates from a saved snapshot instead of fetching live. |
| `-e, --entry <tier:bytes>` | A single custom scenario, repeatable. Omit to use the four standard scenarios. |
| `--json` | Emit the full `BenchmarkReport` as JSON. |
| `-o, --output <path>` | Write to a file instead of stdout. |

Global flags apply: `--network`, `--rpc-url`, `--allow-demo`, `-v`.

## Standard scenarios

| Name | Footprint |
|---|---|
| `empty` | No storage entries — fresh deployment |
| `minimal` | One 1 KB persistent entry |
| `populated` | Ten 1 KB persistent entries |
| `heavy` | 100 × 1 KB persistent **plus** 50 × 512 B temporary |

Each scenario is evaluated at horizons `30, 180, 365` days.

```bash
soroban-cost-benchmarks benchmark --json
```

Custom footprint instead:

```bash
soroban-cost-benchmarks benchmark -e persistent:4096 -e instance:256
```

## ⚠️ The resource metrics are placeholders

A `BenchmarkReport` contains a `ResourceMetrics` block with `cpu_instructions`,
`memory_bytes`, `read_entries`, `write_entries`, `read_bytes`, `write_bytes`,
`tx_size_bytes`, and `min_resource_fee`.

**Every one of those fields is currently `0`.** This command computes *rent projections*
per scenario; it does not measure resource consumption. The report also carries
`config_source`, so the provenance of the rates is recorded.

If you need real resource numbers, use
[`soroban-cost-estimator`](https://github.com/Stellar-Cost-Labs/soroban-cost-estimator),
which wraps `simulateTransaction`.

Widening the scenario suite is tracked as
[#6](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/6).

→ [Limitations](../limitations.md)
