# Fixture evidence record

Everything in this directory is **real captured output**, not illustrative
text. Each file records where its numbers came from so a reader can reproduce or
challenge them. Nothing here was hand-written.

Captured: **2026-09-12**, between 08:00 and 08:05 UTC, against Stellar **testnet**.

| File | What it is |
|---|---|
| `testnet-config-live.json` | Live `ConfigSnapshot`, fetched via the `live-config` workaround |
| `live-config-evidence.txt` | `live-config` stdout — ledger provenance for the above |
| `forecast-live-testnet.json` | Rent forecast from the live config (JSON) |
| `forecast-demo.json` | Rent forecast from `--allow-demo` placeholders, for comparison |
| `forecast-live-stderr.txt` / `forecast-demo-stderr.txt` | The stderr banners each run printed |
| `pr-comment-rendered-body.md` | Comment body rendered from the live forecast — **rendered, never posted** |
| `pr-comment-dry-run-stderr.txt` | The `[dry run]` banner, proving no API call was made |

Reproduce:

```bash
soroban-cost-benchmarks live-config --out tests/fixtures/testnet-config-live.json
soroban-cost-benchmarks rent-forecast \
  --entry persistent:1024 --entry persistent:2048 --entry temporary:512 --json \
  > tests/fixtures/forecast-live-testnet.json
soroban-cost-benchmarks rent-forecast --allow-demo \
  --entry persistent:1024 --entry persistent:2048 --entry temporary:512 --json \
  > tests/fixtures/forecast-demo.json
```

---

## 1. The upstream ledger-staleness bug, reproduced

`soroban-cost-estimator` v0.1.0 stamps `ConfigSnapshot.ledger` with the **max
`last_modified_ledger` across the fetched `ConfigSetting*` entries** instead of
the network's current ledger. Reconfirmed live:

```
Current ledger:     4635348   (getLatestLedger)
Upstream would say: 3470630   (max last_modified_ledger — stale by 1164718 ledgers)
```

Per-setting provenance from the same fetch:

```
CONFIG_SETTING_CONTRACT_COMPUTE_V0                   last modified @ 606666
CONFIG_SETTING_CONTRACT_LEDGER_COST_V0               last modified @ 3470630
CONFIG_SETTING_CONTRACT_HISTORICAL_DATA_V0           last modified @ 606666
CONFIG_SETTING_CONTRACT_EVENTS_V0                    last modified @ 606666
CONFIG_SETTING_CONTRACT_BANDWIDTH_V0                 last modified @ 606666
CONFIG_SETTING_STATE_ARCHIVAL                        last modified @ 2332
```

`3470630` is exactly the value in the original report, which is the point:
config settings only change on governance events, so this number is frozen
between upgrades. The issue is prepared in
[`docs/upstream-issue-config-snapshot-stale-ledger.md`](../../docs/upstream-issue-config-snapshot-stale-ledger.md)
and is **not yet filed** — the available GitHub token lacks `Issues: write`.

## 2. Three-way independent cross-check of the workaround

Taken at **2026-09-12T08:04:46Z**, back to back:

| Source | Ledger |
|---|---|
| Horizon `GET /ledgers?order=desc&limit=1` | **4635340** |
| RPC `getLatestLedger` (raw `curl`) | **4635340** |
| `soroban-cost-benchmarks live-config` (`getLedgerEntries` + `getLatestLedger`) | **4635340** |

Exact agreement: no lag discrepancy at all. (Horizon's `closed_at` for that
ledger was `2026-09-12T08:04:47Z`.) Note that
`/history_latest_ledger` is not a valid Horizon path and returns 404; the
equivalent is `/ledgers?order=desc&limit=1`.

Against the stale value, the gap is **1,164,710 ledgers** — not normal network
lag.

## 3. Live forecast vs. demo-mode numbers

Both runs use the same entries (`persistent:1024`, `persistent:2048`,
`temporary:512`) and the same horizons. Live config: ledger 4635349,
`config_source: rpc:getLedgerEntries@4635349 (testnet)`. Demo config:
`denominator=4096`, flat rate `1267`.

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

**They differ by 1.33x to 2.66x**, so the tool is not silently falling back to
demo values. The demo rates understate real testnet rent by more than half for
persistent storage.

## 4. A formula bug this exercise exposed

Running the forecast against real config the first time produced **all zeros**,
while demo mode produced plausible non-zero numbers — the opposite of the usual
failure mode, and the more dangerous one.

Cause: the original `calculate_rent` used `rent_fee1_kb_soroban_state_size_low`
directly as the rate. On testnet that field is **`-17000`**, so the fee came out
negative and saturated to 0.

The protocol's real rate is a two-step computation (`compute_rent_write_fee_per_1kb`
in [`rs-soroban-env`
`fees.rs`](https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs)),
not the raw low value:

```
multiplier = max(high - low, 0)
if state_size < target:
    rate = ceil(multiplier * state_size / target) + low
else:
    rate = high + ceil(multiplier * (state_size - target) * growth_factor / target)
rate = max(rate, 1000)        # MINIMUM_RENT_WRITE_FEE_PER_1KB
```

For this capture:

```
state size (mean of LiveSorobanStateSizeWindow, 30 samples) = 2,659,201,588 bytes
target                                                      = 4,000,000,000 bytes
low / high / growth                                         = -17000 / 10000 / 5000
interpolated                                                = ceil(27000 * 2659201588 / 4e9) = 17950
17950 + (-17000)                                            = 950
max(950, 1000)                                              = 1000   <-- effective rate
```

So the rate is currently pinned to the protocol floor. Two consequences worth
flagging:

- The old `README` claim `effective_rate = rent_rate * (1 + growth_factor / 10000)`
  was **wrong**. `soroban_state_rent_fee_growth_factor` is a raw multiplier
  applied only to state size *beyond* the target — it has no effect at all while
  the network is below target, which testnet is.
- The state size is not part of `ConfigSnapshot`. It lives in the separate
  `LiveSorobanStateSizeWindow` `ConfigSetting` entry, which
  `soroban-cost-estimator` does not model, so it is fetched directly. The **mean**
  of the 30 window samples is used; the raw samples are in
  `live-config-evidence.txt`. This choice does not affect the current result
  (any state size below ~2.52 GB lands on the same floor), but it is an
  approximation and is called out as such rather than presented as exact.

## 5. Demo mode is now opt-in

Previously, omitting `--config-snapshot` silently produced plausible-looking
placeholder numbers with only a warning. That is the exact foot-gun that made
this evidence record necessary, and it is now closed:

- Default behaviour with no `--config-snapshot` is a **live fetch**.
- If the live fetch fails, the command **errors out** and refuses to fall back,
  printing the three options (`--config-snapshot`, `--allow-demo`, `--rpc-url`).
- Placeholder numbers require `--allow-demo`, print a `⚠️` banner, and are stamped
  `config_source: demo-config (NOT network data)` with `config_ledger: 0` in both
  the table output and the JSON.

See `forecast-demo-stderr.txt` for the banner.

## 6. What the PR comment would look like — rendered, not posted

`pr-comment-rendered-body.md` is the exact body produced from the live forecast
in §3, via the new `--dry-run` flag:

```bash
soroban-cost-benchmarks pr-comment \
  --owner Stellar-Cost-Labs --repo soroban-cost-benchmarks --pr-number 999 \
  --forecast tests/fixtures/forecast-live-testnet.json --dry-run
```

`--dry-run` prints the body and makes **no GitHub API call**, so it needs no
token. This is a preview only. It is **not** evidence that the bot works, and it
must not be reported as such.

It did surface a real defect while being written: the table originally omitted
entry size, so the two `Persistent` rows (1024 B and 2048 B) were
indistinguishable. A `Size (B)` column was added and is now asserted in the unit
test.

## 7. Not yet verified

- **The PR comment bot has still never posted a real comment.** The Markdown
  renderer and the update-in-place logic are unit-tested, and the body is
  rendered above — but no GitHub API call has executed. This needs a token with
  **`Issues: Write`** (PR conversation comments are issue comments) and
  **`Pull requests: Write`**. The token available here has neither, which is the
  same limitation that blocked issue creation.
- **Update-in-place is unproven.** The marker-based find/update path
  (`COMMENT_MARKER` → `list_comments` → `update_comment`) has never run against
  the API. Until a second push is observed editing the existing comment rather
  than adding a duplicate, treat the bot as unverified.
- The `wasm_metrics` and `comparison` sections of the comment are exercised only
  by unit tests with synthetic data — no real WASM file or comparison has been
  fed through.
- The upstream issue is prepared but unfiled (see §1).
