# `export`

Re-serialize a rent forecast produced by `rent-forecast --json` into JSON or CSV.

## Usage

```
soroban-cost-benchmarks export --forecast <path> [--format json|csv] [-o <path>]
```

| Option | Default | Meaning |
|---|---|---|
| `--forecast <path>` | — | **Required.** A rent-forecast JSON file. |
| `--format <fmt>` | `json` | `json` or `csv`. Any other value falls through to JSON. |
| `-o, --output <path>` | stdout | Write to a file instead of stdout. |

## Examples

```bash
soroban-cost-benchmarks rent-forecast --entry persistent:1024 --json > forecast.json

soroban-cost-benchmarks export --forecast forecast.json --format csv
soroban-cost-benchmarks export --forecast forecast.json --format csv -o forecast.csv
```

## Relationship to `rent-forecast --csv`

`rent-forecast` can emit CSV directly. `export` exists to convert a forecast you already
saved — useful when the JSON is the artifact you archive and CSV is what a spreadsheet
needs later.

Both serialize the same `RentForecastEntry` rows: tier, days, ledgers, size, stroops,
XLM, daily stroops. Summary rows are not included in CSV output.

## Caveats

- Round-tripping an empty entry list, unicode paths, and very large horizon sets is not
  handled explicitly — tracked as
  [#5](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/5).
- No dedicated live run of `export` is recorded; it is a re-serialization step, verified
  only by the round-trip it performs on a forecast file.

→ [Limitations](../limitations.md)
