#!/usr/bin/env python3
# Group membership changes reach the server without a restart.
#
# The server caches each user's groups; every 30s it asks the providers again and replaces
# what changed (server/src/auth/svc.rs, USER_REFRESH).  This adds the dev user to a new
# group in users.toml, waits for GetCurrentUser and a collection search to show it, then
# takes it away again and waits for both to drop it.  The file is edited in place (a
# single-file bind mount keeps following the same inode) and always restored.
#
# Needs the dev stack up.  Seeds one collection owned by the new group, removed at the end.
#
# run from the checkout whose docker/entanglement/users.toml the stack mounts:
#   python3 -I docker/devtest/group_refresh_check.py
#   USERS_TOML=/path/to/users.toml python3 -I .../group_refresh_check.py   (another checkout)
# exits non-zero on any mismatch.

import json
import os
import subprocess
import sys
import time

U = "https://127.0.0.1:8444/entanglement"
CERT = ["--cert", "docker/dev-certs/client.crt", "--key", "docker/dev-certs/client.key"]
USERS_TOML = os.environ.get("USERS_TOML", "docker/entanglement/users.toml")
GROUP = "counttest-refresh"
COLL = "counttest-refresh-c"
# one refresh period plus slack for the provider round trip
WAIT_S = 45
FAILS = 0


def psql(q):
    out = subprocess.check_output(
        ["docker", "exec", "-i", "entanglement-postgres-1", "psql", "-U", "entanglement",
         "-d", "entanglement", "-At", "-v", "ON_ERROR_STOP=1"],
        input=q.encode())
    return out.decode().strip()


def call(ep, body):
    for _ in range(5):
        p = subprocess.run(
            ["curl", "-sk", *CERT, "-o", "-", "-w", "\n%{http_code}", "-X", "POST",
             "-H", "content-type: application/json", "--data-binary", "@-", f"{U}/api/{ep}"],
            input=json.dumps(body).encode(), capture_output=True)
        if p.returncode == 0:
            break
    text, _, code = p.stdout.decode().rpartition("\n")
    try:
        return int(code or 0), json.loads(text) if text else None
    except ValueError:
        return int(code or 0), None


def state():
    _, me = call("GetCurrentUser", {})
    _, found = call("SearchCollections", {"filter": COLL, "with_total": True})
    return GROUP in (me or {}).get("groups", []), (found or {}).get("meta", {}).get("total")


def wait_for(in_group):
    start = time.monotonic()
    while time.monotonic() - start < WAIT_S:
        s = state()
        if s[0] == in_group:
            return s, round(time.monotonic() - start)
        time.sleep(2)
    return state(), None


def write_in_place(path, text):
    # same inode, so the container's single-file bind mount sees the change
    with open(path, "r+") as f:
        f.seek(0)
        f.write(text)
        f.truncate()


rows = []


def check(name, expected, observed):
    global FAILS
    ok = str(expected) == str(observed)
    FAILS += 0 if ok else 1
    rows.append([name, str(expected), str(observed), "OK" if ok else "MISMATCH"])


with open(USERS_TOML) as f:
    original = f.read()
if f"[groups.{GROUP}]" in original:
    sys.exit(f"{USERS_TOML} already has [groups.{GROUP}]; restore it first")

psql(f"DELETE FROM collections WHERE name = '{COLL}'")
psql(f"""INSERT INTO collections (collection_uuid, uid, gid, name, note, tags, cover)
         VALUES (gen_random_uuid(), 'counttest-refresh', '{GROUP}', '{COLL}',
                 '[counttest] group refresh', ''::hstore, NULL)""")

try:
    check("before: dev in the group / its collection found", (False, 0), state())

    write_in_place(USERS_TOML, original + f'\n[groups.{GROUP}]\nmembers = ["dev"]\n')
    s, secs = wait_for(True)
    check(f"added to users.toml: dev in the group / collection found (after {secs}s)", (True, 1), s)
    check(f"seen within {WAIT_S}s without a restart", True, secs is not None)

    write_in_place(USERS_TOML, original)
    s, secs = wait_for(False)
    check(f"removed from users.toml: dev in the group / collection found (after {secs}s)", (False, 0), s)
    check(f"dropped within {WAIT_S}s without a restart", True, secs is not None)
finally:
    with open(USERS_TOML) as f:
        if f.read() != original:
            write_in_place(USERS_TOML, original)
    psql(f"DELETE FROM collections WHERE name = '{COLL}'")

hdr = ["case", "expected", "observed", "match"]
w = [max(len(r[i]) for r in rows + [hdr]) for i in range(4)]
fmt = lambda r: " | ".join(c.ljust(w[i]) for i, c in enumerate(r))
print("== group membership refresh")
print(fmt(hdr))
print("-+-".join("-" * x for x in w))
for r in rows:
    print(fmt(r))
print("\nALL CASES MATCH" if FAILS == 0 else f"\n{FAILS} CASE(S) MISMATCH")
sys.exit(1 if FAILS else 0)
