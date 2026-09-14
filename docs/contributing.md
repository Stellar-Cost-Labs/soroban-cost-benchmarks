# Contributing

Contributions are welcome. The repository-level
[CONTRIBUTING.md](https://github.com/aigbagbobila/soroban-cost-benchmarks/blob/main/CONTRIBUTING.md)
is the canonical workflow doc; this page summarises it and lists the open work.

## Workflow

1. Fork the repository.
2. Branch: `git checkout -b feat/my-feature`.
3. Make the change, with tests.
4. Run the gates:

   ```bash
   cargo fmt
   cargo clippy --all-targets --all-features
   cargo test --workspace
   ```

5. Commit with a Conventional Commit message (`feat:`, `fix:`, `docs:`, `test:`, …).
6. Open a pull request.

## Gates `main` enforces

`main` is protected by the `Main_ruleset` (ID `23152648`). A pull request needs one
approving review, and these four checks must be green:

| Check | Command |
|---|---|
| `Formatting` | `cargo fmt --check` |
| `Clippy` | `cargo clippy --all-targets --all-features` |
| `Tests` | `cargo test --workspace` |
| `Build` | `cargo build --release` |

Clippy runs with `all` + `pedantic` at **deny** level (`-D warnings` is set via
`RUSTFLAGS` in CI). The allowed pedantic lints are listed in `Cargo.toml` — add to that
list only with a reason.

## Open work

Seven scoped, claimable issues. Each follows the sibling repository's shape: `Summary` /
`Background` / `Acceptance criteria` / `Implementation hints` / `Repo-specific notes` /
`Out of scope`.

| # | Title |
|---|---|
| [#2](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/2) | `feat(rent)`: model Instance storage explicitly instead of reusing the Persistent denominator |
| [#3](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/3) | `fix(config)`: fetch `LiveSorobanStateSizeWindow` for the `--config-snapshot` path too |
| [#4](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/4) | `feat(compare)`: per-tier regression thresholds instead of one global percentage |
| [#5](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/5) | `fix(export)`: handle empty entry lists, unicode paths, and large horizon sets |
| [#6](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/6) | `feat(benchmark)`: expand the suite beyond the four standard scenarios |
| [#7](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/7) | `test(live-config)`: assert the live fetch's ledger matches Horizon |
| [#8](https://github.com/aigbagbobila/soroban-cost-benchmarks/issues/8) | `chore`: decide whether to track `Cargo.lock` |

Issue #3 also closes a documented gap: a snapshot file has no state-size window, so
`rent-forecast --config-snapshot` falls back to the protocol floor.

## Where the evidence lives

If you change the rent formula, the config fetch, or the comment rendering, update the
evidence record rather than restating it:

- [`tests/fixtures/README.md`](https://github.com/aigbagbobila/soroban-cost-benchmarks/blob/main/tests/fixtures/README.md)
  — the capture log and reproduction steps.
- [Verification](verification.md) — the same evidence, written for readers.
- [Limitations](limitations.md) — update this if you close a gap, but do not soften it.

Fixture files are **verbatim captures**. Do not back-edit an old capture to match new
behaviour; add a new one alongside it.

## Style

- Keep claims traceable to output. If something is not proven, say so.
- No invented numbers in docs. Every figure in these pages traces to a captured file or a
  live API response.
- One commit per logical unit; stage files intentionally.

## License

By contributing, you agree your contributions are licensed under the same dual license
(MIT OR Apache-2.0) as the project.
