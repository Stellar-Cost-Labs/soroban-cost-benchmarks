# Security Policy

## Reporting vulnerabilities

If you discover a security vulnerability in this project, please report it
responsibly. **Do not open a public issue.**

Instead, please email the maintainers at the address listed in the repository
settings, or use GitHub's private vulnerability reporting feature.

## Scope

This tool provides cost **estimates** for Soroban smart contracts. It is not
a production-critical system. However, the following could be security-relevant:

- **Incorrect rent calculations** — if the rent formula implementation diverges
  from the protocol, developers could underestimate costs.
- **Stale config data** — if cached config snapshots are used without
  verification, pricing changes could go undetected.
- **GitHub token handling** — the PR comment bot requires a `GITHUB_TOKEN`.
  Tokens should be stored securely (e.g., GitHub Actions secrets) and never
  committed to the repository.

## Disclaimer

This is unaudited developer tooling. Always verify fee estimates against the
target network before deploying contracts to mainnet.
