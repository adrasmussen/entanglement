# entanglement: local docker-compose quickstart

Brings up entanglement (server + Postgres) entirely in containers, isolated to
your machine. No Rust, Postgres, ffmpeg, or Kerberos installed on the host.
See `docker/NOTES.md` for *why* things are set up this way (in particular: why
auth is mTLS, not username/password).

Run everything below from the repo root.

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

```sh
docker compose up -d certgen postgres
docker compose up -d server
```

`certgen` generates a throwaway dev CA + server cert + client cert (CN=`dev`)
into `docker/dev-certs/` (gitignored, regenerated per-machine). `postgres`
applies `db/schema.sql` on first boot. Check `docker compose logs server` for
`startup complete!` with no panics.

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
  -X POST https://127.0.0.1:8443/entanglement/api/StartTask \
  -H "Content-Type: application/json" \
  -d "{\"library_uuid\":\"$LIBRARY_UUID\",\"task_type\":\"ScanLibrary\"}"

# 5. check what got indexed
curl -sk --cert docker/dev-certs/client.crt --key docker/dev-certs/client.key \
  -X POST https://127.0.0.1:8443/entanglement/api/SearchMediaInLibrary \
  -H "Content-Type: application/json" \
  -d "{\"library_uuid\":\"$LIBRARY_UUID\",\"hidden\":null,\"opts\":{\"filter\":{\"SubstringAny\":{\"filter\":[]}},\"order\":\"DateDesc\",\"limit\":null,\"offset\":0}}"
```

A scan with `"status":"Failure"` almost always means the path doesn't exist
inside the container yet — go back to the restart step. Check
`docker compose logs server` for the specific error either way.

## 6. Import the client certificate

The app uses mutual TLS: identity comes from a client certificate, not a
password (see `docker/NOTES.md`). Your browser needs `docker/dev-certs/client.p12`
(password: `devclientcert`) imported before `https://127.0.0.1:8443` will work.

**macOS** — import into your login keychain so Safari/Chrome can present it:

```sh
security import docker/dev-certs/client.p12 \
  -k ~/Library/Keychains/login.keychain-db \
  -P devclientcert \
  -T /Applications/Google\ Chrome.app \
  -T /Applications/Safari.app
```

- Drop a `-T /Applications/<Browser>.app` for any browser you don't have, and
  add one for others (e.g. `-T /Applications/Firefox.app` — though Firefox
  actually uses its own cert store, see below).
- The first time a site requests it, macOS may still prompt you to allow the
  browser to access the keychain item — click "Always Allow".
- If you get "SecKeychainItemImport: The specified data is either damaged or
  wasn't in the correct format", double check the `-P` password matches the
  one `docker compose logs certgen` printed (`devclientcert`, unless you've
  changed `docker/certgen/generate.sh`).

**Firefox** doesn't use the macOS keychain — import manually instead:
Settings → Privacy & Security → Certificates → View Certificates →
"Your Certificates" tab → Import → select `docker/dev-certs/client.p12`,
password `devclientcert`.

**Skip the browser entirely** and just hit the API with curl:

```sh
curl -sk --cert docker/dev-certs/client.crt --key docker/dev-certs/client.key \
  -X POST https://127.0.0.1:8443/entanglement/api/SearchLibraries \
  -H "Content-Type: application/json" -d '{"filter":""}'
```

## 7. Open the app

Visit `https://127.0.0.1:8443/entanglement/app`. The server's own TLS cert is
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

## Inspecting the database

Postgres has no `ports:` mapping in `docker-compose.yml` — it's deliberately
unreachable from the host or LAN, only from other containers on the `entg-net`
network (see `docker/NOTES.md`). There's no `localhost:5432` to point a
regular Postgres client at.

**From inside the container** (easiest — `local` connections over the
container's own Unix socket are `trust`-authenticated, no password needed):

```sh
docker exec -it entanglement-postgres-1 psql -U entanglement -d entanglement
```

```sql
-- e.g.
SELECT library_uuid, path, uid, gid, count FROM libraries;
SELECT media_uuid, path, media_type FROM media;
```

**Connection string entanglement/dbtool use internally** (only resolves inside
the `entg-net` Docker network — `postgres` is a Compose service DNS name, not
a host you can reach from outside the network, and it requires TLS against the
dev CA):

```
postgres://entanglement:entanglement_dev_password@postgres/entanglement
```

**If you want a GUI client (TablePlus, pgAdmin, etc.) or `psql` from the host**,
temporarily publish the port by adding this under the `postgres` service in
`docker-compose.yml`, then `docker compose up -d postgres`:

```yaml
    ports:
      - "127.0.0.1:5432:5432"
```

You'll still need TLS (the client will need `sslmode=require` or stronger) and
either the `entanglement`/`entanglement_dev_password` credentials or a trust
rule added to `docker/postgres/pg_hba.conf` for host connections — the
existing `hostssl` rules already there use `scram-sha-256`, so
username/password over TLS from `127.0.0.1` will work as-is.

## Resetting everything

```sh
docker compose down -v   # drops Postgres data + media-srv volumes
rm -rf docker/dev-certs  # forces certgen to regenerate certs next `up`
```

`media-src/` (your test photos) is untouched by either of these — it's a plain
host bind mount, not a Docker volume.
