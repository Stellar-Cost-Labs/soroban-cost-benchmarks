# Contributing to soroban-cost-benchmarks

Thanks for your interest in contributing! This document outlines the process
for contributing to this project.

## Getting started

1. Fork the repository
2. Clone your fork
3. Create a feature branch: `git checkout -b feat/my-feature`
4. Make your changes
5. Run the checks:
   ```bash
   cargo fmt
   cargo clippy --all-targets --all-features
   cargo test
   ```
6. Commit with a conventional commit message (see below)
7. Push and open a pull request

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add storage-rent forecast for custom horizons
fix: correct rent formula growth factor calculation
docs: update README with PR comment bot examples
```

## Code standards

- **Clippy:** `all` + `pedantic` at deny level. Run `cargo clippy --all-targets --all-features` before pushing.
- **Formatting:** `cargo fmt` must be clean.
- **Tests:** All existing tests must pass. Add tests for new functionality.
- **No `git add .`:** Stage files intentionally. One commit per logical unit.

## Pull requests

- Keep PRs focused on a single concern.
- Include a description of what changed and why.
- If adding a feature, add tests and update the README.
- CI must pass (fmt, clippy, tests) before merge.

## Reporting issues

Open an issue on GitHub with:
- A clear description of the problem
- Steps to reproduce (if applicable)
- Expected vs actual behavior

## License

By contributing, you agree that your contributions will be licensed under the
same dual license (MIT OR Apache-2.0) as the project.
