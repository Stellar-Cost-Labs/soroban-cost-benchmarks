# Gap-closing review — `soroban-cost-benchmarks`

**Session date:** 2026-09-13
**Scope:** close the items left `BLOCKED` / `NOT STARTED` by the Plan-B gap-closing
prompt, and report honestly on what could and could not be proven.

**Bottom line:** the PR comment bot is now genuinely proven live — and the first
live call exposed a real, shipped bug that no unit test could have caught. The
remaining gaps are not blocked on effort; they are blocked on **two specific
GitHub App permission grants**, described in §1.

---

## Status summary

| Phase | Item | Status |
|---|---|---|
| 0 | Credential diagnosis | **Measured — original premise was wrong** (§1) |
| 1.1 | File upstream issue in the sibling repo | **Blocked** — `403 createIssue` (§3) |
| 3 | PR comment bot proven live | **Done and proven** (§2) |
| 4 | Org transfer | **Deferred by maintainer** (deliberate, not blocked) |
| 4 | `origin` remote corrected | **Done** (§4.1) |
| 4 | Branch-name mismatch (`main` vs `master`) | **One click from done** — remote `master` undeletable until the default moves (§4.2) |
| 4 | Branch protection / ruleset | **Blocked** — `403` (§4.3) |
| 5 | Description + topics | **Blocked** — `403` (§5) |
| 6 | Drips-shaped issue backlog | **Done** — 7 issues (§6) |
| 7 | Drips-readiness check | **Done** (§9) |

---

## 1. Phase 0 — the credential diagnosis was wrong

Phase 0 assumed a single root cause: "the active token is an App-installer token
with no `Issues: Write`." That framing does not survive measurement. The token is
a GitHub App **user-to-server** token:

```
$ gh auth status
  ✓ Logged in to github.com account aigbagbobila (GITHUB_TOKEN)
  - Token: ghu_************************************

$ gh api -i user | grep -i x-oauth-scopes
X-Oauth-Scopes:                      # empty, as expected for an App token
Github-Authentication-Token-Expiration: 2026-09-13 19:55:30 UTC
```

Rather than trust the earlier `403` or the misleading `permissions` block (which
reports `admin: true, push: true` on both repos), permissions were probed
empirically with a **reversible** operation — create a label, then delete it:

```
label create/delete on aigbagbobila/soroban-cost-benchmarks      -> exit 0
label create/delete on Stellar-Cost-Labs/soroban-cost-estimator  -> HTTP 403
   Resource not accessible by integration
```

So there are **two distinct, per-installation** gaps, not one token-wide one:

1. **`Issues: write` on `Stellar-Cost-Labs/soroban-cost-estimator`** — the App
   installation is read-only there, while the *same token* can write issues on
   the personal repo. This is what blocks §3.
2. **`Administration: write`** — absent entirely. Probed with a self-deleting,
   zero-impact ruleset (targeting a branch pattern that cannot match):

   | Operation | Result |
   |---|---|
   | `POST …/branches/master/rename` | `403` |
   | `PATCH …/repos/{owner}/{repo}` (description, default branch) | `403` |
   | `PUT …/topics` (no-op probe) | `403` |
   | `GET …/branches/master/protection` | `403` → **state unknown** |
   | `POST …/rulesets` | `403` (probe created nothing) |

This blocks §4.2, §4.3 and §5. Consequences worth stating plainly:

- The branch-protection state is **unknown**, not "unprotected". A `403` on a
  read endpoint is not evidence of absence.
- A token **cannot be minted from this environment**. Phase 0's instruction to
  "generate a token" is a browser/App-settings action, so what is reported here
  is a measurement of the credential that exists, not a fix.

---

## 2. Phase 3 — the PR comment bot, proven live

This was the one substantive item that was neither blocked nor already done. It
is now proven end-to-end, and it **found a real bug** on the first attempt.

### 2.1 The defect

The first live call panicked before opening a socket:

```
thread 'main' panicked at .../rustls-0.23.44/src/crypto/mod.rs:249:14:
Could not automatically determine the process-level CryptoProvider from Rustls
crate features.
```

**Cause.** Two rustls providers were compiled into the same binary:

- `octocrab`'s default features enable `rustls-ring`
  (`octocrab -> hyper-rustls/ring -> rustls/ring`).
- `soroban-cost-estimator`'s `reqwest` enables `rustls` with `aws-lc-rs`
  (`reqwest/default-tls -> reqwest/rustls -> hyper-rustls/aws-lc-rs`).

With both present, rustls 0.23 refuses to pick a process-level default and
panics. `reqwest` was unaffected because it builds its TLS configuration with an
explicit provider; `octocrab` relies on the process default.

**Why it shipped.** Every existing test is a unit test. None of them opens a
socket, so the TLS provider was never exercised.

### 2.2 The fix

`Cargo.toml` — opt `octocrab` out of its ring default and onto the same provider
the estimator already uses, leaving exactly one provider in the graph. Its other
defaults are restated so nothing else is dropped:

```toml
octocrab = { version = "0.44", default-features = false, features = [
    "default-client", "follow-redirect", "retry",
    "rustls", "rustls-aws-lc-rs", "timeout", "tracing",
] }
```

**Verified, not assumed:**

```
$ cargo tree -e features -i rustls | grep 'feature "ring"'
  -> rustls ring feature NOT enabled (good)
$ cargo tree -i ring | grep -A2 rustls
  -> ring no longer under rustls (good)
```

### 2.3 The round-trip

A throwaway PR (`test/pr-comment-bot-live`, PR #1) against
`aigbagbobila/soroban-cost-benchmarks`, driven by a **fresh** live forecast
(testnet ledger `4652629`, not demo data):

```
RUN 1 (first post)                          Comment posted/updated: ID 5652127269
                                            created_at 08:08:27Z, updated_at 08:08:27Z

RUN 2 (second commit pushed to the branch)  Comment posted/updated: ID 5652127269
                                            created_at 08:08:27Z, updated_at 08:08:37Z
                                            comment count on PR #1: 1
```

Same ID across both runs, `created_at` unchanged, `updated_at` advanced, and the
count stays at **1** — the second run edited the existing comment instead of
posting a duplicate. **Update-in-place is proven, not inferred.** The exact
posted body is preserved in `tests/fixtures/pr-comment-posted-body.md`.

### 2.4 Permission result, measured

The earlier review flagged `Pull requests: write` as inferred. It is now
measured: `gh pr create` succeeded, and both comment create and update
succeeded. Both are **per-repository on the installation**, not token-wide.

### 2.5 Cleanup

PR #1 closed (`state: CLOSED`, `closedAt: 2026-09-13T08:08:44Z`), branch
`test/pr-comment-bot-live` deleted, throwaway file removed. Only `master`/`main`
remain.

> **Note on the prompt's assumption:** Phase 3 expected the bot to pick up live
> config automatically because "the repo now hard-errors on missing live config
> by default". That is not true of `pr-comment` — it requires an explicit
> `--forecast <path>` and never fetches config itself. The run used a freshly
> generated live forecast, so the test is still real network data, but the
> plumbing is not automatic.

---

## 3. Phase 1.1 — upstream issue (blocked)

The exact documented command was re-run with the current credential:

```
$ gh issue create --repo Stellar-Cost-Labs/soroban-cost-estimator \
    --title "fix(config): config snapshot reports a stale ledger instead of the current one" \
    --label bug --label "complexity: trivial" --label "Stellar Wave" \
    --body-file docs/upstream-issue-config-snapshot-stale-ledger.md
GraphQL: Resource not accessible by integration (createIssue)
```

Nothing was created (open issues unchanged at 82).

The prepared report's label assumptions **were** verified as correct before the
attempt — `gh label list` on the sibling repo confirms `bug`,
`complexity: trivial|medium|high` and `Stellar Wave` all exist. The content is
fine; only the permission is missing.

Because the issue cannot be filed, `PLACEHOLDER` remains in
`live_config::UPSTREAM_ISSUE_URL`, and in `README.md` and
`tests/fixtures/live-config-evidence.txt`.

**Action required:** grant the App `Issues: write` on
`Stellar-Cost-Labs/soroban-cost-estimator`, or file the issue by hand and
substitute the number in those three places.

---

## 4. Phase 4 — transfer, branch naming, protection

### 4.1 `origin` — fixed

The remote still pointed at the pre-rename name. Git had been printing the
redirect on every push:

```
remote: This repository moved. Please use the new location:
remote:   https://github.com/aigbagbobila/soroban-cost-benchmarks.git
```

`origin` now points at the canonical URL. This removes the crutch of GitHub's
rename redirect.

### 4.2 Branch-name mismatch — one click from resolved

The prompt targets `main`; the actual default is (still) `master`; CI triggers on
`[main, master]`. The maintainer chose **rename to `main`**, and asked for
`master` to be deleted outright.

`main` exists and is byte-identical to `master`. The local checkout now tracks
`main`, and the local `master` and leftover `test/pr-comment-bot-live` branches
have been deleted, so the working copy has a single branch.

The remote `master`, however, **cannot** be deleted while it is the default
branch — GitHub rejects it before permissions are even consulted:

```
$ git push origin --delete master
 ! [remote rejected] master (refusing to delete the current branch: refs/heads/master)
```

And repointing the default is itself blocked, because it needs
`Administration: write`:

```
$ gh api -X PATCH repos/… -f default_branch=main
{"message":"Resource not accessible by integration", … "status":403}
```

So the remaining step is one UI click — Settings → General → Default branch →
`main` — after which `master` can be deleted.

The CI trigger list was **deliberately not narrowed** to `[main]`: `master` is
still the default, so trimming it now would stop CI firing on the branch people
actually push to. Narrow the triggers once the default has moved.

### 4.3 Ruleset / protection — blocked

Fresh state read, refusing to assume anything carried over:

```
$ gh api repos/…/rulesets                 -> []      (genuinely empty)
$ gh api repos/…/branches/master/protection -> 403   (state UNKNOWN)
$ gh api -X POST repos/…/rulesets           -> 403   (probe, created nothing)
```

No ruleset exists and none could be created. Note that the required status-check
names, had this been possible, would be `Formatting`, `Clippy`, `Tests`, `Build`
— taken from the job `name:` fields in `.github/workflows/ci.yml`, not guessed.

### 4.4 Transfer — deferred by the maintainer

Explicitly re-confirmed with the maintainer rather than inferred from the
original prompt. The answer was **not yet** — there is still a documentation file
and other changes to make first. No transfer was attempted.

---

## 5. Phase 5 — description and topics (blocked)

Gated on the transfer, and independently blocked anyway:

```
$ gh api repos/…/topics        -> []          (currently unset)
$ gh api -X PUT repos/…/topics (no-op probe)  -> 403
$ gh api repos/…                -> description: "" (unset)
```

Setting either requires the same missing `Administration: write`. The intended
values remain: description accurate to §2 (the bot **can** now be described as
working), and topics `stellar`, `soroban`, `rust`, `ci-cd`, `cost-analysis`,
`github-actions`.

---

## 6. Phase 6 — issue backlog

Seven issues filed (**#2–#8**) in `aigbagbobila/soroban-cost-benchmarks`, from
the candidate list in the review. Their bodies deliberately follow the sibling
repo's exact shape, taken from a real sibling issue (#142):

`## Summary` / `## Background` / `## Acceptance criteria` / `## Implementation
hints` / `## Repo-specific notes` / `## Out of scope` / `## How to claim and
submit`

The label vocabulary was **read from the sibling repo before anything was
created** — `complexity: trivial|medium|high` (100/150/200 pts), `Stellar Wave`,
`testing`, `ci/cd`, `refactor` — and matching labels were created here first, so
no third naming scheme was introduced.

| # | Title | Type label |
|---|---|---|
| 2 | `feat(rent): model Instance storage explicitly instead of reusing the Persistent denominator` | enhancement |
| 3 | `fix(config): fetch LiveSorobanStateSizeWindow for the --config-snapshot path too` | bug |
| 4 | `feat(compare): per-tier regression thresholds instead of one global percentage` | enhancement |
| 5 | `fix(export): handle empty entry lists, unicode paths, and large horizon sets` | bug |
| 6 | `feat(benchmark): expand the suite beyond the four standard scenarios` | enhancement |
| 7 | `test(live-config): assert the live fetch's ledger matches Horizon` | testing |
| 8 | `chore: decide whether to track Cargo.lock` | ci/cd |

Two were opened and quoted from the **rendered GitHub page** (Chrome is not
available in this environment, so the public HTML was fetched — the same content
a browser displays):

- **#7** — Labels: `Stellar Wave`, `complexity: high (200 pts)`, `testing`; Status: Open
- **#8** — Labels: `Stellar Wave`, `ci/cd`, `complexity: trivial (100 pts)`; Status: Open

**Subsequent change (maintainer request):** labels were then **removed from all
seven issues** — first `Stellar Wave` and `complexity: *`, then the type labels
(`bug` / `enhancement` / `testing` / `ci/cd`). Every issue now has an empty label
set, so the table above records the labels used at creation time, not the current
state. The label *definitions* still exist on the repository and can be reused
later.

---

## 7. Repository changes, file by file

Four commits, one per file, pushed to both `master` and `main` (mirrored so the
future default does not silently miss the TLS fix):

| Commit | File | What and why |
|---|---|---|
| `62d46d1` | `Cargo.toml` | Select `rustls-aws-lc-rs` for `octocrab` so exactly one rustls provider is compiled (§2.2). |
| `2a346ab` | `src/live_config.rs` | Replace the *asserted* blocker with the *measured* one, so the next reader knows this is a permission, not a code, problem. |
| `02ca8f2` | `tests/fixtures/README.md` | Replace the "unproven at the API layer" note with the real transcript (§2.3) plus the rustls defect and the measured write-permission result. |
| `2da7f5f` | `tests/fixtures/pr-comment-posted-body.md` | New fixture: the exact body actually posted, as distinct from the dry-run rendering. |

`origin/master` and `origin/main` are both at `2da7f5f`.

---

## 8. Verification

Run after every code change:

```
cargo fmt --all -- --check                                  -> clean
cargo test --workspace                                      -> 39 passed; 0 failed
cargo clippy --all-targets --all-features -- -D warnings    -> clean
```

The fixture record was also checked for internal consistency: the root `README`
claims the bot "creates/updates a comment in-place on every push" — a claim that
was **previously unbacked** and is now actually supported by §2.3. The README's
statement that the upstream issue is "not yet filed" is still true.

---

## 9. Limitations — what is still not proven

An honest list, in the spirit of the review's §9:

- **The upstream issue is not filed**, and `PLACEHOLDER` remains in three files.
  Blocked on `Issues: write` on the sibling repo.
- **The branch-protection state is unknown**, not "absent". Every read and write
  on that endpoint returns `403`.
- **No ruleset exists**, so the bot and the CI have never run under required
  status checks.
- **Default branch, description and topics are unset/unchanged** — all three need
  `Administration: write`.
- **Remote `main` and `master` are duplicated.** `master` cannot be deleted
  until the default branch is moved to `main` (§4.2).
- **The bot's `wasm_metrics` and `comparison` sections have only ever rendered
  from synthetic unit-test data** — no real WASM file or comparison has been fed
  through a live post. The live test exercised the rent-forecast path only.
- **The bot has only run against a repo with no protection rules in force.**
- **No transfer was attempted**, so the org-side behaviour (rulesets, protected
  branches, issue transfer) is entirely untested.

---

## 10. What is needed to finish

1. **Grant `Administration: write`** to the App installation. This single change
   unblocks: flipping the default branch to `main`, deleting `master`, creating
   the ruleset, setting the description and topics, and the transfer itself.
2. **Grant `Issues: write` on `Stellar-Cost-Labs/soroban-cost-estimator`** (or
   file the prepared issue by hand). This unblocks §3 and clears every
   `PLACEHOLDER`.
3. Then: transfer, create the ruleset with a username-bound bypass (not
   "Repository role: admin"), verify it with a real push-and-revert, and set the
   description and topics.

Once (1) and (2) are in place, nothing in the original plan remains blocked on
work — only on those two grants.

---

## Definition of done

- [ ] Phase 0 credential fixed and confirmed active → **measured, but not fixable from here**; two per-installation grants identified instead of one (§1)
- [ ] Upstream issue filed, `PLACEHOLDER` gone → **blocked** (`403 createIssue`) (§3)
- [x] PR bot proven live: real post, real ID, real update-in-place, throwaway PR closed, branch deleted (§2)
- [ ] Repo transferred with `origin` updated → **deferred by maintainer**; `origin` corrected (§4.1, §4.4)
- [ ] `main`/`master` mismatch resolved and `master` deleted → **local side done** (checkout on `main`; local `master` and throwaway branch removed); remote `master` persists and needs one settings click (§4.2)
- [ ] Fresh ruleset, verified by push-and-revert → **blocked** (`403`), and coupled to the deferred transfer (§4.3)
- [ ] Topics and description set → **blocked** (`403`) and gated on transfer (§5)
- [x] 5–10 real issues created with sibling-matching labels, two quoted from the rendered page; all labels later removed at maintainer request (§6)
- [x] Honesty check: consistent story, explicit limitations, a plain verdict (§9)
