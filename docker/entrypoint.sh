#!/bin/sh
# local-dev entrypoint, two jobs before handing off to the real binary:
#
# 1. common/src/db/postgres.rs verifies the Postgres server's TLS cert against
#    the OS-native trust store (rustls-native-certs), not an arbitrary file --
#    so the certgen-generated dev CA has to be installed as a real system CA
#    inside this container, or every Postgres connection fails TLS verification.
# 2. server/src/main.rs only creates originals/thumbnails/slices under
#    media_srvdir at startup, not the task scan_scratch dir, so create it here.
# 3. the tracked config.toml is mounted as a template (config.toml.in) with a
#    __ENTG_DB_PASSWORD__ placeholder, so the db password only ever lives in the
#    gitignored ./.env; render the real config here.
set -eu

if [ -f /etc/entanglement/config.toml.in ]; then
    : "${ENTG_DB_PASSWORD:?ENTG_DB_PASSWORD must be set (see .env.example)}"
    # the password is spliced into a URL and a sed replacement, so restrict it to
    # characters that need no escaping in either (openssl rand -hex output is fine)
    case "$ENTG_DB_PASSWORD" in
        *[!A-Za-z0-9._~-]*)
            echo "entrypoint: ENTG_DB_PASSWORD may only contain A-Z a-z 0-9 . _ ~ -" >&2
            exit 1
            ;;
    esac
    umask 077
    sed "s/__ENTG_DB_PASSWORD__/$ENTG_DB_PASSWORD/g" \
        /etc/entanglement/config.toml.in > /etc/entanglement/config.toml
fi

if [ -f /certs/ca.crt ]; then
    cp /certs/ca.crt /usr/local/share/ca-certificates/entanglement-dev-ca.crt
    update-ca-certificates >/dev/null
fi

mkdir -p /media/srv/scratch

exec "$@"
