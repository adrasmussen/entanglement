#!/bin/sh
# copies files into a library under media-src/, then triggers a scan so
# entanglement actually indexes them (copying alone is not enough -- see
# docker/QUICKSTART.md "Uploading media").
#
# usage: docker/scripts/upload-media.sh <library-path> <file-or-dir> [more files/dirs...]
#   e.g.: docker/scripts/upload-media.sh testlib ~/Pictures/vacation/*.jpg
set -eu

if [ "$#" -lt 2 ]; then
    echo "usage: $0 <library-path> <file-or-dir> [more files/dirs...]" >&2
    exit 1
fi

LIBRARY_PATH="$1"
shift

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEST="$REPO_ROOT/media-src/$LIBRARY_PATH"
CERT="$REPO_ROOT/docker/dev-certs/client.crt"
KEY="$REPO_ROOT/docker/dev-certs/client.key"
BASE_URL="https://127.0.0.1:8443/entanglement/api"

if [ ! -f "$CERT" ] || [ ! -f "$KEY" ]; then
    echo "error: $CERT / $KEY not found -- run 'docker compose up -d certgen' first" >&2
    exit 1
fi

mkdir -p "$DEST"
cp -R "$@" "$DEST/"
echo "copied into $DEST"

# a directory created under an existing bind mount doesn't always show up
# inside the container immediately on this Docker Desktop setup -- restarting
# is cheap and guarantees the server container's view is current
echo "restarting server container to pick up the new files..."
(cd "$REPO_ROOT" && docker compose restart server >/dev/null)

# give the server a moment to finish restarting before we hit its API
for i in $(seq 1 15); do
    curl -sk -o /dev/null "https://127.0.0.1:8443/entanglement/app" && break
    sleep 1
done

LIBRARY_UUID=$(docker exec entanglement-postgres-1 psql -U entanglement -d entanglement -t -A \
    -c "SELECT library_uuid FROM libraries WHERE path = '$LIBRARY_PATH';")

if [ -z "$LIBRARY_UUID" ]; then
    echo "error: no library registered with path '$LIBRARY_PATH'." >&2
    echo "register one first: docker compose --profile tools run --rm dbtool entg-db --config /etc/entanglement/config.toml add-library --path $LIBRARY_PATH --uid dev --gid devgroup" >&2
    exit 1
fi

echo "found library $LIBRARY_UUID, starting scan..."
curl -sk --cert "$CERT" --key "$KEY" -X POST "$BASE_URL/StartTask" \
    -H "Content-Type: application/json" \
    -d "{\"library_uuid\":\"$LIBRARY_UUID\",\"task_type\":\"ScanLibrary\"}" \
    -o /dev/null -w "StartTask HTTP status: %{http_code}\n"

sleep 2

echo "task history:"
curl -sk --cert "$CERT" --key "$KEY" -X POST "$BASE_URL/ShowTasks" \
    -H "Content-Type: application/json" \
    -d "{\"library\":{\"User\":{\"library_uuid\":\"$LIBRARY_UUID\"}}}"
echo

echo "media now in library:"
curl -sk --cert "$CERT" --key "$KEY" -X POST "$BASE_URL/SearchMediaInLibrary" \
    -H "Content-Type: application/json" \
    -d "{\"library_uuid\":\"$LIBRARY_UUID\",\"hidden\":null,\"opts\":{\"filter\":{\"SubstringAny\":{\"filter\":[]}},\"order\":\"DateDesc\",\"limit\":null,\"offset\":0}}"
echo
