# The PR comment bot

The second lead feature: a structured cost and rent-forecast summary posted **into a
GitHub PR conversation**, updated in place on every push.

## How it identifies its own comment

Every rendered comment starts with a hidden HTML marker:

```
<!-- soroban-cost-benchmarks bot -->
```

On each run the bot lists the PR's comments, looks for the marker, and:

- **finds it** → updates that comment (`update_comment`);
- **does not find it** → creates a new one.

There is no "last comment wins" heuristic and no duplicate suppression by timestamp. The
marker is the identity, so a re-run can never post a second comment.

## What a comment contains

1. Network, config ledger, `config_source`, and snapshot timestamp.
2. **Storage Rent Forecast** — tier, days, size, rent in stroops, rent in XLM, daily rate.
3. **Total by horizon** — summed stroops and XLM per day horizon.
4. **WASM Metrics** — *only if* `--wasm-metrics <path>` was supplied.
5. **Comparison** — *only if* `--comparison <path>` was supplied.

Sections 4 and 5 are omitted entirely when their inputs are absent rather than rendered
empty.

### Why rows carry a `Size (B)` column

The table originally omitted entry size, so a footprint with two Persistent entries of
different sizes produced two indistinguishable rows. The size column was added after the
dry-run rendering surfaced the ambiguity, and a unit test now asserts it is present.

## ⚠️ The bot does not fetch live config itself

This is the one place where the tool's **"live by default"** policy does not apply.

| Command | Omitting `--config-snapshot` |
|---|---|
| `rent-forecast` | live RPC fetch |
| `benchmark` | live RPC fetch |
| `live-config` | live RPC fetch (that *is* the fetch) |
| **`pr-comment`** | **error — `--forecast <path>` is required** |

`pr-comment` accepts a `--forecast` path, parses it, and errors out if it is missing. It
has no config-fetching path of its own. So the bot's numbers are only as live as the
forecast you hand it — which is real network data if you produced it moments earlier with
`rent-forecast`, and stale if you produced it last week.

The intended pipeline is two steps:

```bash
soroban-cost-benchmarks rent-forecast --entry persistent:1024 --json > forecast.json
GITHUB_TOKEN=... soroban-cost-benchmarks pr-comment \
  --owner myorg --repo myrepo --pr-number 42 --forecast forecast.json
```

## Token requirements

`GITHUB_TOKEN` is required unless `--dry-run` is passed. The token needs
`Issues: write` on the target repository — comments on a PR are *issue* comments, so
`Pull requests: write` alone is not the relevant grant. This is **per-repository on the
installation**: the same token used to prove the bot here returned
`HTTP 403: Resource not accessible by integration` when writing to the sibling repository.

## Preview without posting

```bash
soroban-cost-benchmarks pr-comment \
  --owner myorg --repo myrepo --pr-number 42 \
  --forecast forecast.json --dry-run
```

`--dry-run` prints the banner to stderr, prints the body to stdout, and makes **no
GitHub API call**, so it needs no token. Use it in CI to review output without write
permissions.

## Proven behaviour

Update-in-place is proven against a real PR: two runs returned the **same** comment ID
(`5652127269`), `created_at` stayed fixed, `updated_at` advanced, and the comment count
stayed at **1**. See [Verification](../verification.md#pr-comment-bot) for the raw
transcript.

The exact posted body is archived at
[tests/fixtures/pr-comment-posted-body.md](https://github.com/aigbagbobila/soroban-cost-benchmarks/blob/main/tests/fixtures/pr-comment-posted-body.md).

→ [`pr-comment` command reference](../commands/pr-comment.md) ·
[Limitations](../limitations.md) — the WASM and comparison sections are **not** live-proven.
