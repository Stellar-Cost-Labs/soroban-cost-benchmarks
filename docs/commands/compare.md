# `compare`

Diff two cost snapshots and flag rent increases over a threshold. Designed as a CI gate.

## Usage

```
soroban-cost-benchmarks compare --baseline <path> --current <path> [--threshold 10.0] [--json] [--markdown]
```

| Option | Default | Meaning |
|---|---|---|
| `--baseline <path>` | — | **Required.** Baseline snapshot JSON. |
| `--current <path>` | — | **Required.** Current snapshot JSON. |
| `--threshold <percent>` | `10.0` | Maximum allowed rent increase before CI fails. |
| `--json` | — | Emit the `ComparisonResult` as JSON. |
| `--markdown` | — | Accepted, but currently a **no-op** (see below). |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | No regression over threshold |
| `1` | A rent increase exceeded the threshold — `should_fail_ci` was set |

The non-zero exit is what makes it usable as a blocking CI step:

```bash
soroban-cost-benchmarks compare --baseline baseline.json --current current.json --threshold 10.0
```

## Output

Markdown is the **default** format for non-JSON output; `--json` selects JSON.

> `--markdown` is accepted for symmetry but ignored — markdown is already what you get
> without `--json`. The flag does not change behaviour today.

Secondary thresholds are fixed in code: WASM size increase is capped at 20% rather than
being configurable. Making rent thresholds per-tier instead of one global percentage is
tracked as [#4](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/4).

## Provenance in the comparison

Snapshots carry `config_source`, so a comparison between a demo-derived snapshot and a
live-derived one is visible rather than silent. Comparing across different sources
produces a meaningless delta — check `config_source` on both sides first.

## ⚠️ Not proven live

`compare` is covered by four unit tests (no regression, regression detected, decrease is
not a regression, Markdown rendering). It has never been exercised against two real
snapshots taken weeks apart from a live network. Treat the detection logic as
unit-tested, not field-tested.

→ [Limitations](../limitations.md)
