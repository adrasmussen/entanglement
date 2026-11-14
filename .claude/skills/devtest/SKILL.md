---
name: devtest
description: Testing guide for entanglement — the testing principles, unit-test requirements per crate, the docker/devtest seeds and end-to-end scripts (API vs raw SQL, RBAC leak checks, Playwright UI checks), how to run and read them, and how to route a user's testing request. Use whenever someone asks to test, verify, write tests, seed test data, check for leaks/RBAC, check performance, or interpret test output.
---

# Testing entanglement

The repo has no CI and no test database harness yet, so testing combines two things:
1. **Unit tests** for pure logic.
2. **Scripted end-to-end checks in `docker/devtest/`** that run against the local dev stack (see `CLAUDE.md` for the stack, certs and build commands).

This skill says what to run, what to write, and how to answer a testing request.

## Principles (non-negotiable)
1. **Every PR carries its own tests.** No PR relies on tests that only exist on another branch. Seeds must work on the PR's base branch.
2. **Compare against raw SQL, never against expectations typed by hand.** Each scripted case computes its expected value with SQL that applies the backend's visibility rule, then compares it with the endpoint(s).
3. **Print grids of raw values**, laid out as `case | raw SQL | endpoint(s) | expected | match`. The script computes the match column, so there's no mental arithmetic and no "looks right". Show the grid, or its totals, to the user.
4. **Prove cases aren't empty.** If a case passes because both sides return 0, it has to be an intentional zero case. Otherwise add a "non-empty" check.
5. **Seeded data carries a cleanup identifier** (`counttest`: library paths `/counttest…`, note prefix `[counttest]`, collection names `counttest-…`). It may go into the dev database. Clean it up only when the user asks.
6. **Every search, count, lookup, stream or write path gets RBAC leak cases.** Seed out-of-group records and assert they never surface, including in totals, labels and IDs. A leak is fixed on the branch that introduced it, then merged forward.
7. **Hostile input is matched literally.** For each text field, test `'`, `%`, `_`, `\`, `'; DROP TABLE media; --` and `$1`: the count must equal raw SQL on the literal value, and the table must be intact afterwards.
8. **Performance:** any new or changed query gets `EXPLAIN (ANALYZE, BUFFERS)` at a realistic size, before and after any index, reported as a grid (query | top node | index | rows | ms).

## Unit tests: requirements and where they live
Run them with `cargo test -p api -p webapp` (natively) and `cargo test -p common` (in Docker; see `CLAUDE.md`). The `server` crate has no tests yet, so handler and access logic is covered by the scripts below.

| Area | What must be unit tested | Existing examples |
|---|---|---|
| `api` wire types | Old client JSON still parses (missing fields default); new JSON round-trips; flattened fields keep their wire keys | `api/src/page.rs` tests; `page_len_defaults_to_none_when_missing` in `api/src/search.rs` |
| `api` SQL generation | Snapshot tests of the generated SQL **and** the parameter list for **both** backends, showing user input appears only in parameters; free text, every field and op, empty input | the resolver tests in `api/src/search.rs` |
| `api` parsers | Every syntax form, unknown fields and bad ops as issues (not errors), and `parse(to_text(q)) == q` | `api/src/query.rs` |
| `common` helpers | Paging probe and trim, count-only pages, column checks | `common/src/db/mod.rs` |
| `webapp` logic | Keep logic out of components so it can be tested: paging state machines with a fake `server()` page source, URL/`?q=` encoding round-trips through `Route`, chip↔text edits, completions, tag parsing | `webapp/src/common/paging.rs`, `query_ui.rs`, `tags.rs`, `main.rs` route tests |

**Rule of thumb:** if logic can be a pure function, it lives in `api/` or `webapp/src/common/` with tests next to it. If it needs SQL or the stack, it gets a scripted case.

## Seeds (`docker/devtest/*.sql`)
Apply each one with:

```sh
docker exec -i entanglement-postgres-1 psql -U entanglement -d entanglement -v ON_ERROR_STOP=1 < docker/devtest/<file>.sql
```

All of them are idempotent.

| File | Creates | Notes |
|---|---|---|
| `seed_counttest.sql` | Library `/counttest` with **20,000 media** (2,000 hidden) and collections `counttest-c000..c039` (400 each; `c000` is all hidden) | **Ask the user before running it.** It loads 20k searchable rows into their dev DB; offer the smaller seeds if they only need a few cases. The rows have no image files, so they show "No preview" tiles. |
| `seed_search.sql` | Rewrites only counttest rows: tags (`ct-even/odd`, `ct-mod7`, `ct-rare`), description words, hostile values, `checkpoint/checko/checkpt`, plus a hand-testing set of ~1000 items with happy tags (`sunshine/joyful/delight/cheer`) and English phrases; adds `counttest-phrase…` / `counttest-evil…` collections | Needs `seed_counttest.sql` first |
| `seed_rbac.sql` | Standalone. Library `/counttest-rbac` (300 media owned by `counttest-othergroup`), `/counttest-rbac-dev` (20 media dev can see), a foreign collection, a devgroup "bridge" collection (10 foreign media made visible to dev by design), a foreign comment, and a phash twin | Works on any branch |

**Cleanup**, everything counttest, in this order:

```sql
DELETE FROM comments WHERE text LIKE '[counttest]%';
DELETE FROM collection_contents WHERE collection_uuid IN (SELECT collection_uuid FROM collections WHERE name LIKE 'counttest-%');
DELETE FROM collections WHERE name LIKE 'counttest-%';
DELETE FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path LIKE '/counttest%');
DELETE FROM libraries WHERE path LIKE '/counttest%';
```

Indexes that migrations added stay in place.

## End-to-end scripts (`docker/devtest/`)
Run them from the repo root against the deployed dev stack. Each exits non-zero on any mismatch. The Playwright scripts need `playwright` resolvable; set `PW_MODULE=/path/to/node_modules` if it isn't installed in the repo.

Some scripts arrive with later branches of the search/paging stack (PR #8 and the branches stacked on it). If one is missing on your branch, that's why.

| Script | Proves | Needs |
|---|---|---|
| `api_check.sh` (zsh + python) | Paging metadata (`meta`, `has_more`, clamps), count-only requests, `GetCounts`, structured field queries, prefix matching of plain words, rejections (400), hostile values, old request shapes, paging integrity (pages vs unpaged: equal, no dupes), all vs raw SQL | counttest seeds; the `pagetest`/`bulktest` libraries for the older grids |
| `rbac_check.py` | Out-of-group records never surface: media searches and totals, batch labels, scoped searches, collection and library lists, `GetCounts`, direct lookups (401), `SimilarMedia`, streams, and writes (DB unchanged) | `seed_rbac.sql` (the richer version also uses the search seeds) |
| `ui_check.js` (Playwright) | Search bar: chips, completions, double-click field hints, parse issues, filter builder, `?q=` prefill/commit/reload, local-storage fallback, back-navigation landing on the same card, "No preview" tiles | counttest and search seeds |
| `ui_modal_check.js` (Playwright) | Create/edit collection modal: width and padding, the close button, tags typed with any separator stored as a set, help text | the dev stack only (creates and deletes its own `counttest-modal-*` collection) |

**Reading the output:**
- Every grid row shows both the expected and the observed values. When something fails, quote the row, don't paraphrase it.
- The "raw SQL per case" legend under each grid gives the exact query. Rerun it in psql to dig in.
- A run reports `ALL CASES MATCH`, or `N CASE(S) FAILED`, or `MISMATCH`.

## Verifying a change end to end
1. **Unit tests:** `cargo test -p api -p webapp` and `cargo clippy -p webapp`, plus the Docker check for `server`/`common` when they changed.
2. **Deploy:** `docker compose build server && docker compose up -d server` from the checkout of the branch under test. Building from a detached HEAD is fine. Confirm with `docker images entanglement-server:dev` and the server log.
3. **Seeds:** apply what the scripts need. Ask before the 20k seed.
4. **Run** `api_check.sh`, `rbac_check.py` and the relevant Playwright scripts, then report pass/fail totals per suite as a grid and the rows of anything that failed.
5. **Investigating a UI failure:** before blaming the change, write a small Playwright probe that samples state frame by frame (scrollY, document height, card count and height, `history.state`, the network requests made). Earlier failures turned out to be the test data (files that don't exist) rather than the feature.

## Routing a user's testing request
| They ask… | Do this |
|---|---|
| "Test my change" / "is it working?" | Run the unit tests for the crates touched, then deploy and run the scripts covering that area (table above). Report totals per suite, plus failing rows. |
| "Write tests for X" | Pure logic → unit tests next to the code (see the requirements table). An endpoint or SQL → add a grid to `api_check.sh` (raw SQL vs endpoint, a non-empty proof, hostile values) **and** RBAC cases to `rbac_check.py`. UI → a Playwright case in `ui_check.js` or a new `ui_*_check.js`. Seed whatever the cases need, with the counttest marker. |
| "Is anything leaking?" / RBAC | `seed_rbac.sql` then `rbac_check.py`. If they add an endpoint, add its leak cases first. |
| "Search doesn't find X" | Compare raw SQL with the endpoint for that query. Check prefix vs whole-word vs substring semantics (`CLAUDE.md`). Check the data has the value (e.g. seeded rows vs their own). |
| "It's slow" | `EXPLAIN (ANALYZE, BUFFERS)` on the generated SQL with real bound values at a realistic row count (the 20k seed, after asking). Grid of before/after; propose an index migration only for a clear win. |
| "I want data to click around with" | `seed_search.sql`'s hand-testing set (happy tags and phrases) on top of `seed_counttest.sql`. Ask before the 20k base. Tell them seeded media have no thumbnails. |
| "Clean up the test data" | The cleanup block above, after confirming. Report the row counts deleted. |
| CI / test database questions | There is none yet. Planned: a throwaway Postgres (and MariaDB) per run with Rust integration tests in `common`/`server` and a GitHub Actions workflow. GitHub Issues are disabled on the repo, so it's tracked in the maintainer's todo list. |

## Pitfalls seen before
- The server speaks HTTP/2 only: use curl or Playwright, not Python urllib. Retry on curl exit 16.
- `docker/dev-certs/` is gitignored. Run scripts from the main checkout, or symlink the certs into a worktree. Never copy the keys around.
- Seeded counttest media have no files. Use media with real files (e.g. `type:Image` on the gallery, or the `pagetest`/`bulktest` libraries) when a check depends on images loading.
- `prevent_default()` in a Dioxus `onsubmit` reloads the page (see `CLAUDE.md`). A Playwright `framenavigated` to `?<input name>=…` is the tell.
- A full Docker disk breaks Postgres writes ("No space left on device"). Check `docker system df` and prune the build cache.
