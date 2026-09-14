# WASM metrics

`wasm-metrics` performs **static** analysis of a compiled `.wasm` file. It reads the
binary; it does not execute it and does not simulate a transaction.

## What it reports

| Field | Meaning |
|---|---|
| `total_size` | File size in bytes |
| `code_section_size` | Size of the `code` section |
| `data_section_size` | Size of the `data` section |
| `custom_section_size` | Total size of custom sections |
| `function_count` | Number of defined functions |
| `global_count`, `table_count`, `memory_count` | Counts from the corresponding sections |
| `import_count`, `export_count` | Import/export entries |
| `has_contract_spec` | Whether a `contractspecv0` custom section is present |
| `exported_functions` | Names of exported functions |

It is implemented with `wasmparser`. Analysis runs on either a path
(`analyze_wasm`) or an in-memory buffer (`analyze_wasm_bytes`).

```bash
soroban-cost-benchmarks wasm-metrics -w path/to/contract.wasm
soroban-cost-benchmarks wasm-metrics -w path/to/contract.wasm --json
```

## ⚠️ What is *not* proven

The module is covered by **four unit tests** that generate valid WASM binaries in memory
with the `wat` crate. Those tests prove the analyzer handles the structures they
construct — a minimal module, an invalid buffer, an empty buffer, and table formatting.

They do **not** prove:

- that it has been run against a real contract compiled by the Soroban toolchain;
- that its numbers match what a real deployment reports;
- that the `WASM Metrics` section of a PR comment has ever been rendered from live data.

No recorded session has fed a real `.wasm` artifact through either the CLI or a live
comment post. Treat `wasm-metrics` output as untested against real contracts until that
is done. This is deliberately listed in [Limitations](../limitations.md).

## Relationship to the PR comment

When `--wasm-metrics <path>` is supplied to `pr-comment`, this module's JSON output is
rendered as a `WASM Metrics` section. Because the live round-trip only ever exercised the
rent-forecast path, that section has only been rendered from synthetic unit-test data.

→ [`wasm-metrics` command reference](../commands/wasm-metrics.md)
