# entanglement: guidance for Claude

A media organization app.

| Crate | What it is |
|---|---|
| `api` | Shared serde types, compiled for both wasm and native |
| `common` | DB backends: Postgres and MariaDB, behind the `DbBackend` trait |
| `server` | axum HTTP plus message-passing services (`DbMsg`, `AuthMsg`) |
| `webapp` | Dioxus 0.6 SPA |
| `tools` | CLI tooling (rocksdb) |

Setup for humans is in `docker/QUICKSTART.md`. For testing help (what to run, writing tests, seed data, interpreting results), use the **`devtest` skill** (`.claude/skills/devtest/SKILL.md`).

## Build and check
- **Native on macOS:** `api` and `webapp` only. Use `cargo test -p api -p webapp`, `cargo clippy -p webapp` and `cargo fmt --all --check`.
  - The only accepted clippy warning is the existing `enum_variant_names` in `webapp/src/components/modal/mod.rs`.
- **`server` and `common` don't build on macOS** (libgssapi). Check them in Docker, reusing the named cache volumes. The `touch` step matters: the shared target volume otherwise serves stale artifacts when switching between branches or worktrees.

  ```sh
  docker run --rm -v "$PWD":/src:ro -v entg-cargo-registry:/usr/local/cargo/registry -v entg-check-target:/target rust:bookworm bash -c 'apt-get update -qq && apt-get install -y -qq libkrb5-dev clang pkg-config >/dev/null && mkdir /build && cd /src && tar --exclude=./target --exclude=./.claude -cf - . | tar -xf - -C /build && cd /build && find . -name "*.rs" -exec touch {} + && CARGO_TARGET_DIR=/target cargo check -p server -p common && CARGO_TARGET_DIR=/target cargo test -p common'
  ```

- **Never build `-p tools`** unless asked; rocksdb is very slow.
- **Deploy to the dev stack:** `docker compose build server && docker compose up -d server`. The image also compiles the wasm webapp. A warm build takes about 15 minutes, a cold one about 30.
- **Docker build cache fills the VM disk** (about 2 GB left behind per build). Watch `docker system df`. When it fills, `docker builder prune -a -f` frees it without touching volumes; plain `--reserved-space` pruning may free nothing.
- **Never delete Docker volumes:** the project's `entanglement_*` and `entg-*` volumes, and another project's `ysera-dev_*` volumes on the same machine.

## Dev stack
- **App:** https://127.0.0.1:8444/entanglement/app. **API:** `POST /entanglement/api/<Endpoint>`.
- **Postgres:** 127.0.0.1:55433. Shell: `docker exec -i entanglement-postgres-1 psql -U entanglement -d entanglement`.
- **Auth is mTLS.** `docker/dev-certs/client.crt` and `client.key` identify user `dev`, who is in `devgroup` (from `users.toml`). With zsh, pass curl's cert flags as an array:

  ```sh
  C=(--cert docker/dev-certs/client.crt --key docker/dev-certs/client.key); curl -sk "${C[@]}" ...
  ```

- **The server speaks HTTP/2 only.** Python `urllib` fails against it; use curl or Playwright (`clientCertificates`). A fresh connection occasionally drops a stream (curl exit 16), so scripts retry.
- **Secrets:** they live in the gitignored `.env`. Never print, copy or commit `.env` values or dev passwords.
- **Ports:** Docker host ports bind to `127.0.0.1` only, never `0.0.0.0`.

## Rules that apply to all work
- **Every PR carries its own tests:** unit tests for pure logic, and `docker/devtest` seeds plus scripted checks for anything touching SQL, access control or the UI. Seeds must work on that PR's base. See the `devtest` skill.
- **DB access:**
  - Every user value goes in a **bound parameter**; column names and operators come only from static tables (e.g. `FieldSpec`).
  - Push LIMIT and COUNT into SQL, and back paged or counted queries with indexes. Check them with `EXPLAIN (ANALYZE)` on a realistic row count.
  - New indexes go in `db/migrations/<date>_<name>.sql`, mirrored into `db/schema.sql`, with the MariaDB equivalent in comments.
- **Access control (RBAC):**
  - **Visibility rule:** a media item is visible to a user when it isn't hidden and its library's `gid`, or the `gid` of a collection containing it, is one of the user's groups.
  - Every search, count, lookup and stream must apply it, and every new endpoint gets out-of-group leak cases in `docker/devtest/rbac_check.py`.
  - An RBAC bug is fixed on the branch that introduced it, then merged forward.
- **Git:**
  - Never force-push; merge the base into a stacked branch instead.
  - A standalone `cargo fmt` or lint commit at the end of a chain is fine.
  - Commit or push only when asked; open PRs only when asked.
- **PR descriptions:** concise, one bullet per major change, with a Testing section that links `docker/QUICKSTART.md` rather than restating it.

## Platform quirks learned the hard way
- **dioxus-web 0.6 inverts `prevent_default()` on form submit.** Forms never submit natively unless `onsubmit` calls `prevent_default()`, which *re-enables* a full-page native submission. Take the event as `|_|` and don't call it there. For other events it behaves normally.
- **Don't override the router's scroll restoration.** The project treats that as an antipattern. Back-navigation works because grid pages render their cached items on the first frame (`webapp/src/common/grid_cache.rs`, `use_paged`), so the page is full height when the router restores `scrollY`. Anything that changes card height after that first frame shifts the position. That's why the card's image frame carries the aspect ratio and a failed thumbnail becomes a fixed-size "No preview" tile.
- **Postgres full text** (`media.ts_vec`, `'english'` config) matches whole stemmed words. Plain words in a structured query match as prefixes (`'word':*`). Matching inside a word (`eckp`) needs `pg_trgm`, which is planned.
- **Exactly one `ModalBox` per route.** Every `ModalBox` renders the top of the global `MODAL_STACK`, so a page with two of them draws every modal twice. The collection and library detail pages had that. Keep it in the outer component, outside the `ErrorBoundary`.
- **The webapp's search state is the canonical query text.** Pages read it from `?q=` (local storage is the fallback), and chips are a view of the parsed text. Tags are typed with any separator; `webapp/src/common/tags.rs` turns them into a set, and the backend's `|` folding stays internal to MariaDB.
