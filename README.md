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
  | `ENTG_CLIENT_P12_PASSWORD` | export password of `docker/dev-certs/client.p12`    |
  | `ENTG_HTTPS_PORT`          | app + API on `https://127.0.0.1:<port>` (8444)      |
  | `ENTG_DB_PORT`             | postgres on `127.0.0.1:<port>` (55433, TLS required) |

- Both ports are bound to `127.0.0.1` only. Only the port numbers can be
  overridden, never the bind address.

### Rotating the client certificate (`client.p12`) password

The `.p12` bundle is what you import into your browser or keychain. Re-exporting
it keeps the same key and certificate, so existing imports keep working until
you replace them.

```sh
sed -i '' "s/^ENTG_CLIENT_P12_PASSWORD=.*/ENTG_CLIENT_P12_PASSWORD=$(openssl rand -hex 16)/" .env
rm docker/dev-certs/client.p12
docker compose up certgen        # re-exports client.p12 with the new password
```

Then re-import `docker/dev-certs/client.p12` into your browser or keychain if
you need it there (see QUICKSTART step 6).

### Regenerating all dev certificates (CA, server, client)

Do this if the certs expire, leak, or need different hostnames. It replaces the
dev CA, so every old certificate stops being trusted.

```sh
docker compose down                       # keeps the pg-data volume
rm -rf docker/dev-certs
docker compose up -d                      # certgen recreates everything first
```

Afterwards:

1. **Browser and keychain:** delete the old `entanglement-dev` / `CN=dev`
   identity. On macOS, use Keychain Access (search "dev") or
   `security delete-identity -c dev`. Then re-import the new
   `docker/dev-certs/client.p12` with `ENTG_CLIENT_P12_PASSWORD`.
2. **Trusted dev CA:** if you trusted the old CA system-wide (optional
   QUICKSTART step 7), remove it from the System keychain and trust the new
   `docker/dev-certs/ca.crt` instead.
3. **pgAdmin and other GUI clients:** if they use `sslmode=verify-ca`, point
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
