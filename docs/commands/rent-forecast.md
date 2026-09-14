# `rent-forecast`

Generate storage-rent projections per tier at one or more day horizons. This is the lead
feature.

## Usage

```
soroban-cost-benchmarks rent-forecast [OPTIONS]
```

## Options

| Option | Default | Meaning |
|---|---|---|
| `-e, --entry <tier:bytes>` | `persistent:1024` | Storage entry, repeatable. Tiers: `instance`, `persistent`, `temp` / `temporary`. |
| `--config-snapshot <path>` | — | Read rates from a saved `ConfigSnapshot` JSON instead of fetching live. |
| `--horizons <a,b,c>` | `30,180,365` | Comma-separated day horizons. |
| `--json` | — | Emit the full `RentForecast` as pretty JSON. |
| `--csv` | — | Emit one row per entry as CSV. |
| `-o, --output <path>` | stdout | Write output to a file. |

Global flags apply: `--network`, `--rpc-url`, `--allow-demo`, `-v`.

An invalid entry — wrong shape, unknown tier, unparsable size — is a hard error naming
the expected format.

## Rate resolution

With no `--config-snapshot`, this command **fetches live config by default**. If that
fetch fails it exits with an error listing the three escapes; it never silently
substitutes placeholders. Placeholder rates require `--allow-demo` and are stamped
`demo-config (NOT network data)`.

## Examples

Forecast a small footprint against live testnet rates:

```bash
soroban-cost-benchmarks rent-forecast \
  --entry persistent:1024 \
  --entry persistent:2048 \
  --entry temporary:512
```

Machine-readable, for the next pipeline stage:

```bash
soroban-cost-benchmarks rent-forecast --entry persistent:1024 --json > forecast.json
```

Reuse a saved snapshot without touching the network:

```bash
soroban-cost-benchmarks live-config --out snapshot.json
soroban-cost-benchmarks rent-forecast --config-snapshot snapshot.json --entry persistent:1024
```

Explicit placeholder rates, clearly labelled:

```bash
soroban-cost-benchmarks rent-forecast --allow-demo --entry persistent:1024
# ⚠️  --allow-demo: using placeholder rent rates. These are NOT network data ...
```

Custom horizons:

```bash
soroban-cost-benchmarks rent-forecast --horizons 7,90,730 --entry persistent:4096
```

## Output fields worth knowing

| Field | Why it matters |
|---|---|
| `config_source` | Where the rates came from — `rpc:getLedgerEntries@<ledger> (testnet)`, `snapshot:<path>`, or `demo-config (NOT network data)`. |
| `config_ledger` | The ledger the rates were read at. `0` for demo. |
| `effective_rent_rate_1kb` | The rate actually applied to every row. |
| `rate_basis` | How that rate was derived — interpolated, `high+growth`, or the floor. |
| `soroban_state_size_bytes` | The state size used for interpolation (mean of the window), or absent. |

A `snapshot.json` has no `LiveSorobanStateSizeWindow`, so the state size is unknown and
the rate falls back to the floor, with `rate_basis` saying so. That is
[#3](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/3).

## Caveats

- The state size is a **mean approximation**, and this is a first-order projection — it
  does not model code-entry discounts or TTL-vs-size top-up fees.
- `config_ledger: 0` with `demo-config (NOT network data)` means the numbers are
  placeholders. Do not quote them.

→ [Storage-rent forecasting](../concepts/storage-rent-forecasting.md) ·
[Limitations](../limitations.md)
