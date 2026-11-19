#!/usr/bin/env python3
# Abandoned searches stop running in Postgres.
#
# A psql session holds an exclusive lock on `collections`, so any search touching it blocks.
# Then:
#   * OmniSearch's Collections and Media groups hit their 1.5s timeout (Libraries doesn't
#     touch the table and still answers)
#   * a plain SearchCollections whose client gives up after 1s
# and while the lock is still held, pg_stat_activity must show none of those statements
# still waiting: the server cancelled them (common/src/db/postgres.rs, Cancellable).  A
# collection's media search that nobody abandons is the positive control: it must show as
# waiting the whole time, and answer once the lock goes.  Once
# the lock is gone, a burst of searches checks the pool got every connection back.
#
# STATEMENT_TIMEOUT=1 also holds the lock past 60s to see the pool-wide statement_timeout
# end a search nobody abandoned (adds about a minute).
#
# Needs the dev stack up.  creates nothing.
#   python3 -I docker/devtest/search_cancel_check.py
# exits non-zero on any mismatch.

import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

U = "https://127.0.0.1:8444/entanglement"
CERT = ["--cert", "docker/dev-certs/client.crt", "--key", "docker/dev-certs/client.key"]
PSQL = ["docker", "exec", "-i", "entanglement-postgres-1", "psql", "-U", "entanglement",
        "-d", "entanglement", "-At", "-v", "ON_ERROR_STOP=1"]
SLOW = os.environ.get("STATEMENT_TIMEOUT") == "1"
FAILS = 0


def psql(q):
    return subprocess.check_output(PSQL, input=q.encode()).decode().strip()


def call(ep, body, max_time=None):
    extra = ["--max-time", str(max_time)] if max_time else []
    for _ in range(5):
        p = subprocess.run(
            ["curl", "-sk", *CERT, *extra, "-o", "-", "-w", "\n%{http_code}", "-X", "POST",
             "-H", "content-type: application/json", "--data-binary", "@-", f"{U}/api/{ep}"],
            input=json.dumps(body).encode(), capture_output=True)
        # 28 is curl's own timeout: the point of the max_time calls
        if p.returncode in (0, 28):
            break
    text, _, code = p.stdout.decode().rpartition("\n")
    try:
        return int(code or 0), json.loads(text) if text else None
    except ValueError:
        return int(code or 0), None


def waiting(tag):
    # the server's statements carry a leading "-- <name>" comment
    return int(psql(f"""SELECT count(*) FROM pg_stat_activity
                        WHERE pid <> pg_backend_pid() AND state = 'active'
                          AND query ~ '-- {tag}\\s' AND query NOT LIKE '%pg_stat_activity%'"""))


def hold_lock(seconds):
    p = subprocess.Popen(PSQL, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    p.stdin.write(f"BEGIN; LOCK TABLE collections IN ACCESS EXCLUSIVE MODE; "
                  f"SELECT pg_sleep({seconds}); COMMIT;\n".encode())
    p.stdin.close()
    for _ in range(50):
        if psql("SELECT count(*) FROM pg_locks l JOIN pg_class c ON c.oid = l.relation "
                "WHERE c.relname = 'collections' AND l.mode = 'AccessExclusiveLock' AND l.granted") == "1":
            return p
        time.sleep(0.1)
    sys.exit("could not take the lock on collections")


rows = []


def check(name, expected, observed):
    global FAILS
    ok = str(expected) == str(observed)
    FAILS += 0 if ok else 1
    rows.append([name, str(expected), str(observed), "OK" if ok else "MISMATCH"])


BRIDGE = psql("SELECT collection_uuid FROM collections WHERE gid = 'devgroup' ORDER BY name LIMIT 1")

# baseline: nothing of ours running
check("before: search statements running", 0, waiting("search_[a-z_]+"))

lock = hold_lock(10)
control = ThreadPoolExecutor(1).submit(
    call, "SearchMediaInCollection",
    {"collection_uuid": BRIDGE, "opts": {"filter": {"Query": {"text": [], "clauses": []}}, "order": "DateDesc",
                                         "limit": 1, "offset": 0, "with_total": True}}, 30)
t0 = time.monotonic()
code, d = call("OmniSearch", {"query": "counttest"})
took = time.monotonic() - t0
status = {g["kind"]: (g["status"] if isinstance(g["status"], str) else next(iter(g["status"])))
          for g in (d or {}).get("groups", [])}
check(f"OmniSearch under the lock: http (answered in {took:.1f}s)", 200, code)
check("OmniSearch under the lock: group status", {"Media": "TimedOut", "Collection": "TimedOut", "Library": "Ok"}, status)

code, _ = call("SearchCollections", {"filter": "counttest", "with_total": True}, max_time=1)
check("SearchCollections, client gives up after 1s: no answer", 0, code)

time.sleep(1)
still_locked = lock.poll() is None
check("the lock is still held while we look", True, still_locked)
check("abandoned search_media statements still running", 0, waiting("search_media"))
check("abandoned search_collections statements still running", 0, waiting("search_collections"))
check("control: the search nobody abandoned is still waiting", 1, waiting("search_media_in_collection"))
lock.wait()
check("control: it answers once the lock is gone", 200, control.result()[0])

# every connection made it back to the pool: 30 searches at once all answer
with ThreadPoolExecutor(30) as ex:
    codes = list(ex.map(lambda _: call("SearchCollections", {"filter": "counttest", "with_total": True})[0], range(30)))
check("after the lock: 30 concurrent searches answered 200", 30, codes.count(200))
code, d = call("OmniSearch", {"query": "counttest"})
check("after the lock: OmniSearch groups all Ok", ["Ok"] * len((d or {}).get("groups", [])),
      [g["status"] for g in (d or {}).get("groups", [])])

if SLOW:
    lock = hold_lock(75)
    t0 = time.monotonic()
    code, _ = call("SearchCollections", {"filter": "counttest", "with_total": True}, max_time=90)
    took = time.monotonic() - t0
    check(f"STATEMENT_TIMEOUT: a search nobody abandons ends with an error (after {took:.0f}s)", 500, code)
    check("STATEMENT_TIMEOUT: ended by the 60s statement_timeout", True, 55 <= took <= 70)
    subprocess.run(PSQL, input=b"SELECT pg_cancel_backend(pid) FROM pg_stat_activity WHERE query LIKE '%pg_sleep(75)%' AND pid <> pg_backend_pid();",
                   capture_output=True)
    lock.wait()

hdr = ["case", "expected", "observed", "match"]
w = [max(len(r[i]) for r in rows + [hdr]) for i in range(4)]
fmt = lambda r: " | ".join(c.ljust(w[i]) for i, c in enumerate(r))
print("== abandoned searches are cancelled")
print(fmt(hdr))
print("-+-".join("-" * x for x in w))
for r in rows:
    print(fmt(r))
print("\nALL CASES MATCH" if FAILS == 0 else f"\n{FAILS} CASE(S) MISMATCH")
sys.exit(1 if FAILS else 0)
