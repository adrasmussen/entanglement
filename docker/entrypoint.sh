#!/bin/sh
# local-dev entrypoint, two jobs before handing off to the real binary:
#
# 1. common/src/db/postgres.rs verifies the Postgres server's TLS cert against
#    the OS-native trust store (rustls-native-certs), not an arbitrary file --
#    so the certgen-generated dev CA has to be installed as a real system CA
#    inside this container, or every Postgres connection fails TLS verification.
# 2. server/src/main.rs only creates originals/thumbnails/slices under
#    media_srvdir at startup, not the task scan_scratch dir, so create it here.
set -eu

if [ -f /certs/ca.crt ]; then
    cp /certs/ca.crt /usr/local/share/ca-certificates/entanglement-dev-ca.crt
    update-ca-certificates >/dev/null
fi

mkdir -p /media/srv/scratch

exec "$@"
