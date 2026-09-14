# `wasm-metrics`

Static analysis of a compiled `.wasm` file. Reads the binary; does not execute it.

## Usage

```
soroban-cost-benchmarks wasm-metrics -w <path> [--json] [-o <path>]
```

| Option | Meaning |
|---|---|
| `-w, --wasm <path>` | **Required.** Path to the `.wasm` file. |
| `--json` | Emit `WasmMetrics` as JSON. Default is a formatted table. |
| `-o, --output <path>` | Write to a file instead of stdout. |

```bash
soroban-cost-benchmarks wasm-metrics -w path/to/contract.wasm
soroban-cost-benchmarks wasm-metrics -w path/to/contract.wasm --json -o wasm.json
```

## Fields

| Field | Meaning |
|---|---|
| `path` | The file that was analyzed |
| `total_size` | File size in bytes |
| `code_section_size` | `code` section size |
| `data_section_size` | `data` section size |
| `custom_section_size` | Total custom-section size |
| `function_count` | Defined functions |
| `global_count` / `table_count` / `memory_count` | Corresponding section counts |
| `import_count` / `export_count` | Import and export entries |
| `has_contract_spec` | Whether a `contractspecv0` section exists |
| `exported_functions` | Exported function names |

## Feed it to the PR comment bot

```bash
soroban-cost-benchmarks wasm-metrics -w contract.wasm --json -o wasm.json
soroban-cost-benchmarks pr-comment --owner myorg --repo myrepo --pr-number 42 \
  --forecast forecast.json --wasm-metrics wasm.json
```

## ⚠️ Not proven against a real contract

This command has never been run against a `.wasm` artifact produced by the Soroban
toolchain in any recorded session, and the `WASM Metrics` section of a PR comment has
only ever rendered from synthetic unit-test data. The four covering unit tests generate
their WASM in memory with `wat`.

Do not treat its output as validated against real contracts.

→ [WASM metrics](../concepts/wasm-metrics.md) · [Limitations](../limitations.md)
