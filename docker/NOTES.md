# Local docker-compose setup: root causes & follow-ups

Everything below was hit bringing up `docker-compose.yml` for the first time on a
clean machine. Recorded here so the next machine doesn't have to re-diagnose it,
and so it's clear which items are bugs/gaps in the **application** (`entanglement`
itself) vs. things that are just properties of the **local tooling** we built.

## Application-side gap (source code, not our docker tooling)

### `tomlfile` auth is unimplemented over HTTP — this is why we're on mTLS

- `common/src/auth/tomlfile.rs`'s `TomlAuthnFile` (`authenticate_user`/`is_valid_user`)
  exists and works as an `AuthnProvider`, but nothing in the HTTP layer ever calls it.
- `server/src/http/svc.rs` only attaches an auth middleware (`route_layer`) when
  `authn_backend` is `ProxyHeader` or `X509Cert`. For any other backend (including
  `TomlFile`), no middleware runs, so `CurrentUser` is never inserted as a request
  extension.
- `server/src/http/auth.rs` has a `_password_auth`/`_authorize` pair (both
  underscore-prefixed = intentionally unused/dead code) that would do HTTP Basic
  auth against an `AuthnProvider`, but `_authorize` just returns
  `Err("not implemented")` and it's never wired into the router.
- The `webapp` frontend has **no login form anywhere** — it assumes identity is
  established transparently at the TLS layer (mTLS or a trusted reverse-proxy
  header) and never sends an `Authorization` header.
- **Net effect**: with `authn_backend = "tomlfile"`, the server boots fine and
  serves the static WASM app, but *every* JSON API call
  (`GetMedia`/`SearchMedia`/etc.) 500s with `Missing request extension:
  server::http::auth::CurrentUser`. It's not a config mistake — the code path
  simply doesn't exist yet.
- **What we did instead**: switched to `authn_backend = "x509cert"` (mTLS), which
  *is* fully wired up (`cert_auth` in `server/src/http/auth.rs`), and generate a
  throwaway client cert (`docker/certgen/generate.sh`, CN=`dev`) for local
  testing. No application source was changed.
- **If this should be fixed upstream** (not attempted here, out of scope for a
  local-dev setup task): either (a) finish `_password_auth`/`_authorize` and wire
  it into `server/src/http/svc.rs`'s middleware selection, plus build a login
  form in `webapp`, or (b) if `tomlfile` authn was only ever meant as a
  password *store* consulted by some other flow, remove the dead
  `_password_auth`/`_authorize` code and the `TomlFile` variant from
  `AuthnBackend` so the config schema doesn't advertise a backend that can't
  actually authenticate anyone over HTTP.

## Local tooling gotchas (docker/ files in this repo, not the app)

These were all fixed already; listed so a future rebuild on another machine
doesn't need to rediscover them:

- **`to_tsvector()` is `STABLE`, not `IMMUTABLE`** in Postgres, so `media.ts_vec`
  in `db/schema.sql` can't be a `GENERATED ALWAYS AS ... STORED` column — it's
  populated by a `BEFORE INSERT OR UPDATE` trigger instead. Inherent to
  Postgres, not something the original app author necessarily hit (no schema
  was ever checked in to compare against).
- **`dx build` needs the full 5-crate workspace present**, not just `api`/`webapp`
  — `cargo metadata` fails to resolve the workspace if any member's `Cargo.toml`
  is missing. `Dockerfile`'s `wasm-builder` stage copies `common`/`server`/`tools`
  too, even though it never compiles them.
- **`dioxus-cli` must be pinned to `0.6.3`** to match this workspace's
  `dioxus = "0.6.3"` (`cargo install dioxus-cli --version 0.6.3`) — an
  unpinned/latest CLI (0.7.x) builds successfully but is a different major
  version with a different CLI behavior.
- **Even pinned to 0.6.3, `dx build --release` ignores `Dioxus.toml`'s
  `out_dir = "dist"`** and writes to `target/dx/webapp/release/web/public`
  instead — `Dockerfile` copies from that path, not `webapp/dist`.
- **Postgres 18's official image changed its volume convention**: it now expects
  a single mount at `/var/lib/postgresql` (pg_ctlcluster-style, versioned
  subdirs), not `/var/lib/postgresql/data` — see docker-library/postgres#1259.
- **`rustls-native-certs` (used by `common/src/db/postgres.rs` for the Postgres
  TLS connection) verifies against the container's OS trust store, not an
  arbitrary mounted cert file.** Mounting `ca.crt` into the `server`/`dbtool`
  containers isn't enough by itself — `docker/entrypoint.sh` also runs
  `update-ca-certificates` after copying it into
  `/usr/local/share/ca-certificates/` before exec'ing the real binary.
