# Storage-rent forecasting

This is the lead feature. It answers a question the rest of the ecosystem's tooling does
not: *what will this storage footprint cost over the next 30, 180, and 365 days, per
storage tier, at the network's **current** rent rates?*

## Where the rates come from

Two `ConfigSetting*` ledger entries, read live from RPC:

| Setting | Fields used |
|---|---|
| `ConfigSettingContractLedgerCostV0` | `rent_fee1_kb_soroban_state_size_low`, `..._high`, `soroban_state_rent_fee_growth_factor`, `soroban_state_target_size_bytes` |
| `ConfigSettingStateArchival` | `persistent_rent_rate_denominator`, `temp_rent_rate_denominator` |

A third setting supplies the input the estimator does not model:
`ConfigSettingLiveSorobanStateSizeWindow`, a rolling window of the Soroban state size.

Nothing is hardcoded, and the rates are recorded on every output as
`config_source` + `config_ledger` so a forecast can be traced back to the ledger it came
from.

## The formula

Rent is a two-step computation mirroring
[`rs-soroban-env` `fees.rs`](https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs)
(`compute_rent_write_fee_per_1kb` → `rent_fee_for_size_and_ledgers`). All arithmetic is
integer `ceil` division — no floating point in the fee path.

**Step 1 — effective rate per 1 KB**, interpolated across the state-size curve and
floored:

```
multiplier = max(high - low, 0)
if state_size < state_target_size_bytes:
    rate = ceil(multiplier × state_size / state_target_size_bytes) + low
else:
    rate = high + ceil(multiplier × (state_size - target) × growth_factor / target)
rate = max(rate, 1000)          # MINIMUM_RENT_WRITE_FEE_PER_1KB
```

**Step 2 — fee for a size and a number of ledgers:**

```
rent_fee = ceil(size_bytes × rate × ledgers) / (1024 × rate_denominator)
```

`rate_denominator` is `persistent_rent_rate_denominator` for Persistent **and Instance**,
and `temp_rent_rate_denominator` for Temporary. Days are converted to ledgers at
`17,280` ledgers/day (approximate, ~5 s per ledger).

## Three details that are easy to get wrong

**`growth_factor` is a raw multiplier, not a percentage.** It multiplies only the state
size *beyond* the target. While the network is below target it is completely inert — and
testnet is below target. An earlier revision of this tool applied it as
`rate × (1 + growth_factor / 10000)`, which was simply wrong; there is now a test
asserting the factor has no effect below target.

**`low` can be negative, and using it alone yields zero rent.** On testnet `low =
-17000`. The `+ low` term in step 1 is intentionally *not* clamped; the final floor
(`max(rate, 1000)`) is what keeps the result positive. An earlier revision used `low`
directly as the rate, so live forecasts produced **all zeros** while demo mode produced
plausible numbers — the dangerous direction to fail in.

**State size is an approximation.** It is the **mean** of the
`LiveSorobanStateSizeWindow` samples. The tool records the basis it used in `rate_basis`
(for example `interpolated (state 2659201588 < target 4000000000)`) rather than
presenting it as exact. On the captured day this did not change the answer: any state
size below ~2.52 GB lands on the same protocol floor.

## Live vs. demo, and why it matters

The tool **never** silently substitutes placeholder rates:

- With no `--config-snapshot`, the default is a **live fetch**.
- If the live fetch fails, the command **errors out** and names the three options
  (`--config-snapshot`, `--allow-demo`, `--rpc-url`).
- Placeholder numbers require `--allow-demo`, print a `⚠️` banner, and are stamped
  `config_source: demo-config (NOT network data)` with `config_ledger: 0` in every output
  format.

Running the same entries both ways proves the tool is serving real data. On testnet,
2026-09-12, live and demo differed by **1.33× to 2.66×** — demo understated persistent
storage rent by more than half.

The full table is in [Verification](../verification.md#rent-forecast).

## Try it

```bash
soroban-cost-benchmarks rent-forecast \
  --entry persistent:1024 --entry persistent:2048 --entry temporary:512
```

→ [`rent-forecast` command reference](../commands/rent-forecast.md)
