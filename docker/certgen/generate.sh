#!/bin/sh
# generates a throwaway dev CA + server cert/key (shared by postgres and server)
# plus a client cert/key (CN=dev) for mTLS, since entanglement's x509cert authn
# backend is the only auth path actually wired up over HTTP -- see
# docker/NOTES.md for why tomlfile auth doesn't work here.
#
# the CA is name-constrained to the local names below, so trusting it in a browser
# or OS trust store (QUICKSTART "Trust the dev CA") can never vouch for any other
# site, even if ca.key leaks.  the server cert follows the browser rules a locally
# trusted cert must still meet (macOS: <= 825 days, serverAuth EKU), and is renewed
# automatically by `docker compose up` when it nears expiry.
#
# idempotent per-artifact: re-running `docker compose up` won't regenerate
# something that already exists in the bind-mounted /certs dir, except that
#   - a CA from before the name constraints is retired: everything is moved to
#     /certs/legacy-<timestamp>/ and regenerated (re-import client.p12 afterwards)
#   - a server cert within 30 days of expiry is reissued from the same CA
set -eu

CERT_DIR=/certs
mkdir -p "$CERT_DIR"
cd "$CERT_DIR"

# every name the server cert carries must be permitted here
SERVER_SAN='DNS:server,DNS:postgres,DNS:localhost,IP:127.0.0.1,IP:::1'
NAME_CONSTRAINTS='critical,permitted;DNS:server,permitted;DNS:postgres,permitted;DNS:localhost,permitted;IP:127.0.0.1/255.255.255.255,permitted;IP:::1/ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff'
SERVER_DAYS=397
RENEW_SECONDS=$((30 * 24 * 3600))

if [ -f ca.crt ] && ! openssl x509 -in ca.crt -noout -ext nameConstraints 2>/dev/null | grep -q Permitted; then
    legacy="legacy-$(date -u +%Y%m%d%H%M%S)"
    mkdir "$legacy"
    for f in ca.crt ca.key ca.srl server.crt server.key client.crt client.key client.p12 client-modern.p12; do
        if [ -f "$f" ]; then mv "$f" "$legacy/"; fi
    done
    chmod 700 "$legacy"
    echo "retired the unconstrained dev CA and its certs to $CERT_DIR/$legacy; regenerating all of them"
fi

if [ ! -f ca.crt ] || [ ! -f ca.key ]; then
    openssl req -quiet -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
        -keyout ca.key -out ca.crt \
        -subj "/CN=entanglement-dev-ca (localhost only)" \
        -addext "basicConstraints=critical,CA:TRUE,pathlen:0" \
        -addext "keyUsage=critical,keyCertSign,cRLSign" \
        -addext "nameConstraints=$NAME_CONSTRAINTS"
    # a new CA invalidates any server cert signed by an old one
    rm -f server.crt
    echo "generated dev CA (name-constrained to localhost, 127.0.0.1, ::1, server, postgres)"
fi

if [ -f server.crt ] && ! openssl x509 -in server.crt -noout -checkend "$RENEW_SECONDS" >/dev/null; then
    rm -f server.crt
    echo "dev server cert expires within 30 days; reissuing it"
fi

if [ ! -f server.crt ] || [ ! -f server.key ]; then
    if [ ! -f server.key ]; then
        openssl genrsa -quiet -out server.key 4096
    fi
    openssl req -new -key server.key -out server.csr -subj "/CN=entanglement-dev"

    cat > server.ext <<EOF
subjectAltName=$SERVER_SAN
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
authorityKeyIdentifier=keyid
subjectKeyIdentifier=hash
EOF

    openssl x509 -req -in server.csr \
        -CA ca.crt -CAkey ca.key -CAcreateserial \
        -out server.crt -days "$SERVER_DAYS" -sha256 \
        -extfile server.ext

    rm -f server.csr server.ext
    echo "generated dev server cert (valid $SERVER_DAYS days; renewed by docker compose up)"
fi

if [ ! -f client.crt ] || [ ! -f client.key ]; then
    # CN must match a [users.*]/[groups.*.members] entry in docker/entanglement/users.toml
    # -- server/src/http/auth.rs's cert_auth() uses the client cert's CN verbatim as uid
    openssl req -quiet -newkey rsa:4096 -nodes \
        -keyout client.key -out client.csr \
        -subj "/CN=dev"

    cat > client.ext <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=clientAuth
authorityKeyIdentifier=keyid
subjectKeyIdentifier=hash
EOF

    openssl x509 -req -in client.csr \
        -CA ca.crt -CAkey ca.key -CAcreateserial \
        -out client.crt -days 3650 -sha256 \
        -extfile client.ext

    rm -f client.csr client.ext client.p12 client-modern.p12
    echo "generated dev client cert (CN=dev)"
fi

# convenience bundle for importing into a browser/OS keychain. the export password
# comes from ENTG_CLIENT_P12_PASSWORD in the gitignored ./.env and is passed to
# openssl via the environment (not argv). this is regenerated whenever client.p12
# is missing, so to change the password: update .env, delete client.p12, and run
# `docker compose up certgen` -- the client key/cert themselves are unchanged.
if [ ! -f client.p12 ]; then
    : "${ENTG_CLIENT_P12_PASSWORD:?ENTG_CLIENT_P12_PASSWORD must be set (see .env.example)}"
    export ENTG_CLIENT_P12_PASSWORD

    openssl pkcs12 -export -legacy \
        -inkey client.key -in client.crt \
        -certfile ca.crt \
        -out client.p12 -passout env:ENTG_CLIENT_P12_PASSWORD \
        2>/dev/null || \
    openssl pkcs12 -export \
        -inkey client.key -in client.crt \
        -certfile ca.crt \
        -out client.p12 -passout env:ENTG_CLIENT_P12_PASSWORD

    echo "generated client.p12 (password: ENTG_CLIENT_P12_PASSWORD in .env)"
fi

# the same identity with modern encryption (AES-256, PBKDF2, a SHA-256 MAC).  client.p12 above
# uses the legacy RC2-40 cipher, which older macOS keychains need but current Firefox rejects
# (it reports a wrong password), so Firefox imports this one instead.  same password
if [ ! -f client-modern.p12 ]; then
    : "${ENTG_CLIENT_P12_PASSWORD:?ENTG_CLIENT_P12_PASSWORD must be set (see .env.example)}"
    export ENTG_CLIENT_P12_PASSWORD

    openssl pkcs12 -export \
        -inkey client.key -in client.crt \
        -certfile ca.crt -name "entanglement dev (CN=dev)" \
        -out client-modern.p12 -passout env:ENTG_CLIENT_P12_PASSWORD

    echo "generated client-modern.p12 for Firefox (same password)"
fi

chmod 644 ca.crt server.crt client.crt client.p12 2>/dev/null || true
chmod 600 client-modern.p12 2>/dev/null || true
chmod 600 server.key ca.key client.key 2>/dev/null || true
# the official postgres image runs as uid/gid 999; it refuses to start if its
# key file is owned by someone else with looser permissions than 0600.
chown 999:999 server.crt server.key 2>/dev/null || true

echo "certs ready in $CERT_DIR"
