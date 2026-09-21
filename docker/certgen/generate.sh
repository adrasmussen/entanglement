#!/bin/sh
# generates a throwaway dev CA + server cert/key (shared by postgres and server)
# plus a client cert/key (CN=dev) for mTLS, since entanglement's x509cert authn
# backend is the only auth path actually wired up over HTTP -- see
# docker/NOTES.md for why tomlfile auth doesn't work here.
#
# idempotent per-artifact: re-running `docker compose up` won't regenerate
# something that already exists in the bind-mounted /certs dir.
set -eu

CERT_DIR=/certs
mkdir -p "$CERT_DIR"

if [ ! -f "$CERT_DIR/ca.crt" ] || [ ! -f "$CERT_DIR/ca.key" ]; then
    openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
        -keyout "$CERT_DIR/ca.key" -out "$CERT_DIR/ca.crt" \
        -subj "/CN=entanglement-dev-ca"
    echo "generated dev CA"
fi

if [ ! -f "$CERT_DIR/server.crt" ] || [ ! -f "$CERT_DIR/server.key" ]; then
    openssl req -newkey rsa:4096 -nodes \
        -keyout "$CERT_DIR/server.key" -out "$CERT_DIR/server.csr" \
        -subj "/CN=entanglement-dev"

    printf 'subjectAltName=DNS:server,DNS:postgres,DNS:localhost,IP:127.0.0.1,IP:::1\n' \
        > "$CERT_DIR/server.ext"

    openssl x509 -req -in "$CERT_DIR/server.csr" \
        -CA "$CERT_DIR/ca.crt" -CAkey "$CERT_DIR/ca.key" -CAcreateserial \
        -out "$CERT_DIR/server.crt" -days 3650 -sha256 \
        -extfile "$CERT_DIR/server.ext"

    rm -f "$CERT_DIR/server.csr" "$CERT_DIR/server.ext"
    echo "generated dev server cert"
fi

if [ ! -f "$CERT_DIR/client.crt" ] || [ ! -f "$CERT_DIR/client.key" ]; then
    # CN must match a [users.*]/[groups.*.members] entry in docker/entanglement/users.toml
    # -- server/src/http/auth.rs's cert_auth() uses the client cert's CN verbatim as uid
    openssl req -newkey rsa:4096 -nodes \
        -keyout "$CERT_DIR/client.key" -out "$CERT_DIR/client.csr" \
        -subj "/CN=dev"

    openssl x509 -req -in "$CERT_DIR/client.csr" \
        -CA "$CERT_DIR/ca.crt" -CAkey "$CERT_DIR/ca.key" -CAcreateserial \
        -out "$CERT_DIR/client.crt" -days 3650 -sha256

    rm -f "$CERT_DIR/client.csr"

    # convenience bundle for importing into a browser/OS keychain; empty
    # passphrase is fine, this is throwaway local-dev material
    openssl pkcs12 -export -legacy \
        -inkey "$CERT_DIR/client.key" -in "$CERT_DIR/client.crt" \
        -certfile "$CERT_DIR/ca.crt" \
        -out "$CERT_DIR/client.p12" -passout pass:devclientcert \
        2>/dev/null || \
    openssl pkcs12 -export \
        -inkey "$CERT_DIR/client.key" -in "$CERT_DIR/client.crt" \
        -certfile "$CERT_DIR/ca.crt" \
        -out "$CERT_DIR/client.p12" -passout pass:devclientcert

    echo "generated dev client cert (CN=dev) + client.p12 (password: devclientcert)"
fi

chmod 644 "$CERT_DIR/ca.crt" "$CERT_DIR/server.crt" "$CERT_DIR/client.crt" "$CERT_DIR/client.p12" 2>/dev/null || true
chmod 600 "$CERT_DIR/server.key" "$CERT_DIR/ca.key" "$CERT_DIR/client.key" 2>/dev/null || true
# the official postgres image runs as uid/gid 999; it refuses to start if its
# key file is owned by someone else with looser permissions than 0600.
chown 999:999 "$CERT_DIR/server.crt" "$CERT_DIR/server.key" 2>/dev/null || true

echo "certs ready in $CERT_DIR"
