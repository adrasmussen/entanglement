# entanglement: local docker-compose quickstart

Brings up entanglement (server + Postgres) entirely in containers, isolated to
your machine. No Rust, Postgres, ffmpeg, or Kerberos installed on the host.
See `docker/NOTES.md` for *why* things are set up this way (in particular: why
auth is mTLS, not username/password).

Run everything below from the repo root.

## 0. Create your local `.env`

Do this first. Every `docker compose` command, including the build, refuses to
run without the two passwords set. `.env` is gitignored and holds secrets, so
never commit it:

```sh
cp .env.example .env
sed -i '' "s/^ENTG_DB_PASSWORD=.*/ENTG_DB_PASSWORD=$(openssl rand -hex 24)/" .env
sed -i '' "s/^ENTG_CLIENT_P12_PASSWORD=.*/ENTG_CLIENT_P12_PASSWORD=$(openssl rand -hex 16)/" .env
```

(`sed -i ''` is the macOS form; on Linux use `sed -i` with no `''`.)

If you already have a `pg-data` volume from before `.env` existed, its role
still has the old password. Either reset it (see "Resetting everything") or
follow "Rotating the database password" in the root README, using the value
you just put in `.env`.

## 1. Build the image

```sh
docker compose build server
```

First build compiles the full Rust workspace + the Dioxus WASM frontend —
expect 15-40+ minutes depending on your machine and build-cache state.
Subsequent builds are much faster (layer caching).

## 2. Point at some test media (optional but recommended)

```sh
mkdir -p media-src/testlib
cp ~/Pictures/some-test-photos/*.jpg media-src/testlib/
```

`media-src/` is bind-mounted read-only into the container as `media_srcdir`;
it's gitignored, so this is entirely local to your machine.

## 3. Bring up the stack

The app is published on `https://127.0.0.1:8444` by default, bound to
loopback only. To run it alongside other local stacks, copy `.env.example`
to `.env` and change `ENTG_HTTPS_PORT`; `docker compose` and
`docker/scripts/upload-media.sh` both read it. The examples below assume the
default port. Postgres is likewise published on `127.0.0.1:55433`
(`ENTG_DB_PORT`); see "Inspecting the database".

```sh
docker compose up -d certgen postgres
docker compose up -d server
```

`certgen` generates a throwaway dev CA + server cert + client cert (CN=`dev`)
into `docker/dev-certs/` (gitignored, regenerated per-machine). `postgres`
applies `db/schema.sql` on first boot. Check `docker compose logs server` for
`startup complete!` with no panics.

`db/schema.sql` only runs against an empty `pg-data` volume. If your volume
predates a schema change, apply the matching file from `db/migrations/` by hand
instead of resetting (each file is idempotent and documents how to run it), e.g.:

```sh
docker compose exec -T postgres psql -U entanglement -d entanglement \
  -v ON_ERROR_STOP=1 < db/migrations/2026-11-12_paging_indexes.sql
```

## 4. Register a library

```sh
docker compose --profile tools run --rm dbtool entg-db \
  --config /etc/entanglement/config.toml \
  add-library --path testlib --uid dev --gid devgroup
```

`--path` is relative to `media_srcdir` (i.e. a subfolder of `media-src/`).
`--gid devgroup` must match a group in `docker/entanglement/users.toml` that
the `dev` user (the client cert's CN) is a member of, or you won't see the
library through the API.

Library registrations live in Postgres, not on disk: `docker compose down -v`
(or anything else that drops the `pg-data` volume, e.g. reapplying
`db/schema.sql`) wipes them along with the schema, even though the files
under `media-src/` are untouched. Redo this step after any such reset.

## 5. Uploading media

Copying files into `media-src/<library>/` alone doesn't make them show up in
the app — entanglement only indexes a library when you tell it to scan.
Two things to know:

- **A directory created under `media-src/` after the `server` container is
  already running doesn't always show up inside the container immediately**
  on Docker Desktop's bind-mount implementation (observed on macOS). Restarting
  the container (`docker compose restart server`) forces it to pick up the
  change — this is a local Docker quirk, not an entanglement bug.
- **Scanning is triggered over the API** (`StartTask` with `task_type:
  "ScanLibrary"`), using the same client mTLS cert as everything else, not a
  CLI command.

**Helper script** (does the copy + restart + scan + confirms the media landed,
all in one step):

```sh
docker/scripts/upload-media.sh testlib ~/Pictures/vacation/*.jpg
```

```
usage: docker/scripts/upload-media.sh <library-path> <file-or-dir> [more files/dirs...]
```

`<library-path>` must already be registered (step 4's `add-library`). Sample
output:

```
copied into .../media-src/testlib
restarting server container to pick up the new files...
found library 01a0c50e-279e-7212-881e-f3c05ac94d5f, starting scan...
StartTask HTTP status: 200
task history:
{"tasks":[{"task_type":"ScanLibrary",...,"status":"Success","warnings":0,...}]}
media now in library:
{"media":["01a0c545-...","01a0c544-..."]}
```

**Doing it by hand**, if you'd rather not use the script:

```sh
# 1. copy files into the library's folder under media-src/
mkdir -p media-src/testlib
cp ~/Pictures/vacation/*.jpg media-src/testlib/

# 2. restart so the server container's view of media-src/ is current
docker compose restart server

# 3. look up the library's uuid
LIBRARY_UUID=$(docker exec entanglement-postgres-1 psql -U entanglement -d entanglement \
  -t -A -c "SELECT library_uuid FROM libraries WHERE path = 'testlib';")

# 4. trigger a scan
curl -sk --cert docker/dev-certs/client.crt --key docker/dev-certs/client.key \
  -X POST https://127.0.0.1:8444/entanglement/api/StartTask \
  -H "Content-Type: application/json" \
  -d "{\"library_uuid\":\"$LIBRARY_UUID\",\"task_type\":\"ScanLibrary\"}"

# 5. check what got indexed
curl -sk --cert docker/dev-certs/client.crt --key docker/dev-certs/client.key \
  -X POST https://127.0.0.1:8444/entanglement/api/SearchMediaInLibrary \
  -H "Content-Type: application/json" \
  -d "{\"library_uuid\":\"$LIBRARY_UUID\",\"hidden\":null,\"opts\":{\"filter\":{\"SubstringAny\":{\"filter\":[]}},\"order\":\"DateDesc\",\"limit\":null,\"offset\":0}}"
```

A scan with `"status":"Failure"` almost always means the path doesn't exist
inside the container yet — go back to the restart step. Check
`docker compose logs server` for the specific error either way.

## 6. Import the client certificate

The app uses mutual TLS: identity comes from a client certificate, not a
password (see `docker/NOTES.md`). Your browser needs `docker/dev-certs/client.p12`
(password: `ENTG_CLIENT_P12_PASSWORD` from `.env`) imported before `https://127.0.0.1:8444` will work.

**macOS** — import into your login keychain so Safari/Chrome can present it:

```sh
security import docker/dev-certs/client.p12 \
  -k ~/Library/Keychains/login.keychain-db \
  -P "$(sed -n 's/^ENTG_CLIENT_P12_PASSWORD=//p' .env)" \
  -T /Applications/Google\ Chrome.app \
  -T /Applications/Safari.app
```

- Drop a `-T /Applications/<Browser>.app` for any browser you don't have, and
  add one for others (e.g. `-T /Applications/Firefox.app` — though Firefox
  actually uses its own cert store, see below).
- The first time a site requests it, macOS may still prompt you to allow the
  browser to access the keychain item — click "Always Allow".
- If you get "SecKeychainItemImport: The specified data is either damaged or
  wasn't in the correct format", double check the `-P` password matches
  `ENTG_CLIENT_P12_PASSWORD` in `.env`. If you changed that value after
  `client.p12` was generated, delete `docker/dev-certs/client.p12` and run
  `docker compose up certgen` to re-export it with the new password.

**Firefox** doesn't use the macOS keychain — import manually instead:
Settings → Privacy & Security → Certificates → View Certificates →
"Your Certificates" tab → Import → select `docker/dev-certs/client.p12`,
password: the `ENTG_CLIENT_P12_PASSWORD` value in `.env`.

**Skip the browser entirely** and just hit the API with curl:

```sh
curl -sk --cert docker/dev-certs/client.crt --key docker/dev-certs/client.key \
  -X POST https://127.0.0.1:8444/entanglement/api/SearchLibraries \
  -H "Content-Type: application/json" -d '{"filter":""}'
```

## 7. Open the app

Visit `https://127.0.0.1:8444/entanglement/app`. The server's own TLS cert is
self-signed (separate from the client cert above), so your browser will warn
about it — click through ("Advanced" → "Proceed"). If you'd rather not see
that warning at all, trust the dev CA system-wide (optional, more invasive —
this affects every app on your machine that consults the system trust store,
so only do it if that tradeoff is fine for you):

```sh
sudo security add-trusted-cert -d -r trustRoot \
  -k /Library/Keychains/System.keychain docker/dev-certs/ca.crt
```

When your browser is prompted for a client certificate, pick the one with
subject `entanglement-dev` / CN `dev`.

To check the API against the database, run `zsh docker/devtest/api_check.sh`: it
compares raw SQL with the paging and count endpoints and prints a grid of results.

## Inspecting the database

**From the host (pgAdmin, TablePlus, `psql`, ...):** Postgres is published on
`127.0.0.1:55433` (loopback only; override with `ENTG_DB_PORT` in `.env`).
`docker/postgres/pg_hba.conf` requires TLS plus a password for every
non-socket connection:

| setting  | value                       |
|----------|-----------------------------|
| host     | `127.0.0.1`                 |
| port     | `55433`                     |
| database | `entanglement`              |
| user     | `entanglement`              |
| password | `ENTG_DB_PASSWORD` from `.env` |
| SSL mode | `require`                   |

In pgAdmin the SSL mode is under Register Server → Parameters. `require`
encrypts the connection without verifying the cert. For `verify-ca`, point the
client's root certificate at `docker/dev-certs/ca.crt`.

```sh
set -a; . ./.env; set +a   # load ENTG_DB_PASSWORD from the gitignored .env
psql "postgresql://entanglement:$ENTG_DB_PASSWORD@127.0.0.1:55433/entanglement?sslmode=require"
```

**From inside the container** (no password: `local` connections over the
container's own Unix socket are `trust`-authenticated):

```sh
docker exec -it entanglement-postgres-1 psql -U entanglement -d entanglement
```

```sql
-- e.g.
SELECT library_uuid, path, uid, gid, count FROM libraries;
SELECT media_uuid, path, media_type FROM media;
```

**Connection string entanglement/dbtool use internally.** This only resolves
inside the `entg-net` Docker network: `postgres` is a Compose service DNS name.
The tracked `docker/entanglement/config.toml` holds a `__ENTG_DB_PASSWORD__`
placeholder, which `docker/entrypoint.sh` replaces at container start:

```
postgres://entanglement:<ENTG_DB_PASSWORD>@postgres/entanglement
```

## Changing the database password

`POSTGRES_PASSWORD` only takes effect when the `pg-data` volume is first
created. To change the password on an existing volume, first update the role,
then `.env`, then restart the server:

```sh
NEW=$(openssl rand -hex 24)
docker exec entanglement-postgres-1 psql -U entanglement -d entanglement \
  -c "ALTER ROLE entanglement PASSWORD '$NEW';"
sed -i '' "s/^ENTG_DB_PASSWORD=.*/ENTG_DB_PASSWORD=$NEW/" .env
docker compose up -d server
```

## Rotating passwords and regenerating certificates

The step-by-step procedures (client `.p12` password, full dev-cert
regeneration, database password) are in the root
[README.md](../README.md#local-development) under "Local development".

## Resetting everything

```sh
docker compose down -v   # drops Postgres data + media-srv volumes
rm -rf docker/dev-certs  # forces certgen to regenerate certs next `up`
```

`media-src/` (your test photos) is untouched by either of these — it's a plain
host bind mount, not a Docker volume.
