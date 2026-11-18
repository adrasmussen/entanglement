entanglement is a media organization system.

requires libkrb5-dev, libbig_ham, ffmpeg

all contributions licensed under MIT.

## Local development

The docker-compose dev stack is documented in
[docker/QUICKSTART.md](docker/QUICKSTART.md). The basics:

- Settings and secrets live in a gitignored `.env`. Copy `.env.example` and
  fill it in; compose refuses to start without the passwords. **Never commit
  `.env`.**

  | variable                   | used for                                            |
  |----------------------------|-----------------------------------------------------|
  | `ENTG_DB_PASSWORD`         | postgres `entanglement` role (server, dbtool, GUI)  |
  | `ENTG_CLIENT_P12_PASSWORD` | export password of `docker/dev-certs/client.p12` and `client-modern.p12` |
  | `ENTG_HTTPS_PORT`          | app + API on `https://127.0.0.1:<port>` (8444)      |
  | `ENTG_DB_PORT`             | postgres on `127.0.0.1:<port>` (55433, TLS required) |

- Both ports are bound to `127.0.0.1` only. Only the port numbers can be
  overridden, never the bind address.

### Dev certificates at a glance

`certgen` (`docker/certgen/generate.sh`) writes everything into the gitignored
`docker/dev-certs/`. The design and its reasons are in
[docker/NOTES.md](docker/NOTES.md#dev-certificates); setup is QUICKSTART steps 6–7.

| file | what it is | who uses it |
|---|---|---|
| `ca.crt` / `ca.key` | dev CA, **name-constrained** to `localhost`, `127.0.0.1`, `::1`, `server`, `postgres` | trust `ca.crt` once in your keychain / Firefox; the containers trust it too |
| `server.crt` / `server.key` | TLS cert of the app and postgres: 397 days, `serverAuth`, reissued by `docker compose up` within 30 days of expiry | server, postgres |
| `client.crt` / `client.key` | your identity for mTLS, CN `dev` | curl, the devtest scripts |
| `client.p12` | the client identity for the **macOS keychain** (Safari, Chrome); legacy RC2-40 encryption | `security import` |
| `client-modern.p12` | the same identity with AES-256 encryption, for **Firefox**, which rejects `client.p12` with a misleading "wrong password" | Firefox "Your Certificates" |
| `legacy-<timestamp>/` | certs from the old unconstrained CA, retired automatically on upgrade | delete once the new ones work |

`docker/devtest/cert_check.sh` checks all of it (constraints, browser rules, a
rogue cert being rejected, verified TLS end to end).

### Rotating the client certificate password

The `.p12` bundles are what you import into your browser or keychain.
Re-exporting them keeps the same key and certificate, so existing imports keep
working until you replace them.

```sh
sed -i '' "s/^ENTG_CLIENT_P12_PASSWORD=.*/ENTG_CLIENT_P12_PASSWORD=$(openssl rand -hex 16)/" .env
rm docker/dev-certs/client.p12 docker/dev-certs/client-modern.p12
docker compose up certgen        # re-exports both bundles with the new password
```

Then re-import them where you need them (see QUICKSTART step 6):
`client.p12` into the macOS keychain, `client-modern.p12` into Firefox.

### Regenerating all dev certificates (CA, server, client)

Do this if the certs leak or need different hostnames. Expiry is handled for
you: `docker compose up` reissues the server cert from the same CA when it nears
expiry. Regenerating replaces the dev CA, so every old certificate stops being
trusted. A new hostname has to be added to both `SERVER_SAN` and
`NAME_CONSTRAINTS` in `docker/certgen/generate.sh`, or the CA refuses to vouch
for it.

```sh
docker compose down                       # keeps the pg-data volume
rm -rf docker/dev-certs
docker compose up -d                      # certgen recreates everything first
```

Afterwards:

1. **Keychain (Safari, Chrome):** delete the old `CN=dev` identity and the old
   CA by fingerprint (QUICKSTART step 7, "Upgrading"; run from the repo root),
   then re-import `docker/dev-certs/client.p12` with `ENTG_CLIENT_P12_PASSWORD`.
2. **Firefox:** in View Certificates, delete the old `dev` certificate, the old
   CA under Authorities and any old Servers exception, then import
   `docker/dev-certs/client-modern.p12` and restart Firefox.
3. **Trusted dev CA:** trust the new `docker/dev-certs/ca.crt` (QUICKSTART step 7).
4. **pgAdmin and other GUI clients:** if they use `sslmode=verify-ca`, point
   them at the new `ca.crt`. With `sslmode=require` nothing changes.

### Rotating the database password

`POSTGRES_PASSWORD` is only applied when the `pg-data` volume is first created,
so on an existing volume change the role password first, then `.env`:

```sh
NEW=$(openssl rand -hex 24)
docker exec entanglement-postgres-1 psql -U entanglement -d entanglement \
  -c "ALTER ROLE entanglement PASSWORD '$NEW';"
sed -i '' "s/^ENTG_DB_PASSWORD=.*/ENTG_DB_PASSWORD=$NEW/" .env
docker compose up -d server               # entrypoint re-renders config.toml
```

Then update the saved password in pgAdmin or any other client. The password
may only contain `A-Z a-z 0-9 . _ ~ -`; the `openssl rand -hex` output above
always fits.
