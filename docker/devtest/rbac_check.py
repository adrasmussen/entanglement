#!/usr/bin/env python3
# RBAC leak checks against the dev stack.
#
# Needs docker/devtest/seed_rbac.sql applied: records owned by counttest-othergroup, which the
# dev user is not in, next to a small library dev can see.  Searches, totals, direct lookups,
# streams and writes are checked for out-of-group records.  Expected values come from raw SQL
# that applies the backend's visibility rule; the match column is computed here.
#
# visibility rule (common/src/db/*): a media item is visible to a user when it is not hidden
# and its library's gid, or the gid of a collection containing it, is one of the user's
# groups.  so the 10 foreign media in the devgroup collection counttest-rbac-bridge are
# visible by design (the positive control); every other foreign item must never surface.
#
# run from the repo root:  python3 -I docker/devtest/rbac_check.py
# exits non-zero on any mismatch.  the http client is curl, since the server speaks http/2.
# only request shapes that main already accepts are used, so this runs on any later branch too.

import json
import subprocess
import sys

U = "https://127.0.0.1:8444/entanglement"
CERT = ["--cert", "docker/dev-certs/client.crt", "--key", "docker/dev-certs/client.key"]
GID = "devgroup"
FAILS = 0


def psql(q):
    out = subprocess.check_output(
        ["docker", "exec", "-i", "entanglement-postgres-1", "psql", "-U", "entanglement",
         "-d", "entanglement", "-At", "-v", "ON_ERROR_STOP=1"],
        input=q.encode())
    return out.decode().strip()


def rows(q):
    return [line for line in psql(q).split("\n") if line]


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
        data = json.loads(text) if text else None
    except ValueError:
        data = None
    return int(code or 0), data


def get(path):
    p = subprocess.run(["curl", "-sk", *CERT, "-o", "/dev/null", "-w", "%{http_code}", f"{U}/{path}"],
                       capture_output=True)
    return int(p.stdout.decode() or 0)


def words(*ws):
    return {"SubstringAny": {"filter": list(ws)}}


def opts(filt=None, limit=None, with_total=True):
    return {"filter": filt or words(), "order": "DateDesc", "limit": limit, "offset": 0,
            "with_total": with_total}


def grid(title, cases):
    global FAILS
    print(f"\n== {title}")
    hdr = ["case", "expected", "observed", "foreign leaked", "match"]
    body = []
    for name, exp, obs, leaked in cases:
        ok = str(exp) == str(obs) and leaked == 0
        FAILS += 0 if ok else 1
        body.append([name, str(exp), str(obs), str(leaked), "OK" if ok else "MISMATCH"])
    w = [max(len(r[i]) for r in body + [hdr]) for i in range(5)]
    line = lambda r: " | ".join(c.ljust(w[i]) for i, c in enumerate(r))
    print(line(hdr))
    print("-+-".join("-" * x for x in w))
    for r in body:
        print(line(r))


# --- fixtures, looked up by name ----------------------------------------------------------
if psql("SELECT count(*) FROM libraries WHERE path IN ('/counttest-rbac', '/counttest-rbac-dev')") != "2":
    sys.exit("seed missing: apply docker/devtest/seed_rbac.sql first")

one = lambda q: psql(q)
L_OTHER = one("SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac'")
L_DEV = one("SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac-dev'")
C_OTHER = one("SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-other'")
C_BRIDGE = one("SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-bridge'")
FOREIGN = set(rows(f"SELECT media_uuid FROM media WHERE library_uuid = '{L_OTHER}'"))
BRIDGED = set(rows(f"""SELECT m.media_uuid FROM media m JOIN collection_contents cc USING (media_uuid)
                       WHERE cc.collection_uuid = '{C_BRIDGE}' AND m.hidden = FALSE"""))
NEVER = FOREIGN - BRIDGED  # foreign media that must never surface for dev
by_path = lambda p: one(f"SELECT media_uuid FROM media WHERE path = '{p}'")
F_PLAIN = by_path("/counttest-rbac/rbac_00002.jpg")
F_HIDDEN = by_path("/counttest-rbac/rbac_00010.jpg")
F_SIMILAR = by_path("/counttest-rbac/rbac_00001.jpg")
F_BRIDGED = by_path("/counttest-rbac/rbac_00201.jpg")
V_IN_OTHER = by_path("/counttest-rbac-dev/dev_00001.jpg")  # dev-visible, also in the foreign collection
V_TWIN = by_path("/counttest-rbac-dev/dev_00007.jpg")      # same phash as F_SIMILAR
COMMENT = one("SELECT comment_uuid FROM comments WHERE text LIKE '[counttest] rbac%'")

VIS = f"""m.hidden = FALSE AND (
    m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '{GID}')
    OR EXISTS (SELECT 1 FROM collection_contents cc JOIN collections c USING (collection_uuid)
               WHERE cc.media_uuid = m.media_uuid AND c.gid = '{GID}'))"""


def raw_count(where):
    return int(psql(f"SELECT count(*) FROM media m WHERE {VIS} AND {where}"))


def leaked(uuids):
    return len(set(uuids) & NEVER)


def total(d):
    return (d or {}).get("total")


print(f"fixtures: foreign={len(FOREIGN)} bridged(visible)={len(BRIDGED)} never-visible={len(NEVER)}")

# --- media searches -----------------------------------------------------------------------
cases = []
code, d = call("SearchMedia", {"opts": opts()})
cases.append(["SearchMedia empty: total", raw_count("TRUE"), total(d), leaked(d["media"])])
for label, flt, where in [
    ("words rbac", words("rbac"), "m.ts_vec @@ to_tsquery('english', '''rbac''')"),
    ("words lighthouse beach", words("lighthouse", "beach"),
     "m.ts_vec @@ to_tsquery('english', '''lighthouse'' | ''beach''')"),
    ("words harbour", words("harbour"), "m.ts_vec @@ to_tsquery('english', '''harbour''')"),
]:
    code, d = call("SearchMedia", {"opts": opts(flt)})
    cases.append([f"SearchMedia {label}", raw_count(where), total(d), leaked(d["media"])])
    code, b = call("BatchSearchAndSort", {"req": {"Media": {"opts": opts(flt, limit=200)}}})
    ids = [r["media_uuid"] for r in b["media"]]
    labels = sum(C_OTHER in r["collections"] for r in b["media"])
    cases.append([f"BatchSearchAndSort {label}: total / foreign collection labels",
                  f"{raw_count(where)} / 0", f"{total(b)} / {labels}", leaked(ids)])
grid("media searches (expected = raw SQL under dev's groups)", cases)

# --- scoped searches ----------------------------------------------------------------------
cases = []
for hidden in (None, False, True):
    code, d = call("SearchMediaInLibrary", {"library_uuid": L_OTHER, "hidden": hidden, "opts": opts()})
    cases.append([f"SearchMediaInLibrary foreign library hidden={hidden}", 0,
                  total(d) if code == 200 else 0, leaked((d or {}).get("media", []))])
code, d = call("SearchMediaInCollection", {"collection_uuid": C_OTHER, "opts": opts()})
cases.append(["SearchMediaInCollection foreign collection", 0,
              total(d) if code == 200 else 0, leaked((d or {}).get("media", []))])
code, d = call("SearchMediaInCollection", {"collection_uuid": C_BRIDGE, "opts": opts()})
cases.append(["SearchMediaInCollection bridge (positive control)", len(BRIDGED), total(d),
              0 if set(d["media"]) == BRIDGED else 1])
code, b = call("BatchSearchAndSort", {"req": {"Library": {"library_uuid": L_DEV, "hidden": None,
                                                           "opts": opts(limit=200)}}})
labels = sum(C_OTHER in r["collections"] for r in b["media"])
cases.append(["BatchSearchAndSort dev library: foreign collection labels", 0, labels, 0])
grid("scoped searches", cases)

# --- collections and libraries ------------------------------------------------------------
cases = []
coll = lambda w: int(psql(f"SELECT count(*) FROM collections WHERE gid = '{GID}' AND {w}"))
lib = lambda w: int(psql(f"SELECT count(*) FROM libraries WHERE gid = '{GID}' AND {w}"))
for flt, where in (("", "TRUE"), ("rbac", "name LIKE '%rbac%'"), ("other", "name LIKE '%other%'")):
    code, d = call("SearchCollections", {"filter": flt, "limit": None, "offset": 0, "with_total": True})
    cases.append([f"SearchCollections filter '{flt}'", coll(where), total(d), int(C_OTHER in d["collections"])])
for flt, where in (("", "TRUE"), ("rbac", "path LIKE '%rbac%'")):
    code, d = call("SearchLibraries", {"filter": flt})
    cases.append([f"SearchLibraries filter '{flt}': count", lib(where), len(d["libraries"]),
                  int(L_OTHER in d["libraries"])])
grid("collections and libraries (expected = raw SQL under dev's groups)", cases)

# --- direct lookups and streams -----------------------------------------------------------
cases = []
for label, ep, body, expect in [
    ("GetMedia foreign", "GetMedia", {"media_uuid": F_PLAIN}, "denied"),
    ("GetMedia foreign hidden", "GetMedia", {"media_uuid": F_HIDDEN}, "denied"),
    ("GetMedia bridged (positive control)", "GetMedia", {"media_uuid": F_BRIDGED}, "200"),
    ("GetCollection foreign", "GetCollection", {"collection_uuid": C_OTHER}, "denied"),
    ("GetLibrary foreign", "GetLibrary", {"library_uuid": L_OTHER}, "denied"),
    ("GetComment on foreign media", "GetComment", {"comment_uuid": COMMENT}, "denied"),
    ("SimilarMedia from a foreign source", "SimilarMedia", {"media_uuid": F_SIMILAR, "distance": 64}, "denied"),
]:
    code, _ = call(ep, body)
    cases.append([f"{label} (http {code})", expect, "200" if code == 200 else "denied", 0])
code, d = call("GetMedia", {"media_uuid": V_IN_OTHER})
cases.append(["GetMedia visible item in a foreign collection: foreign collection listed", "False",
              str(C_OTHER in (d or {}).get("collections", [])), 0])
code, d = call("SimilarMedia", {"media_uuid": V_TWIN, "distance": 1})
cases.append(["SimilarMedia from a visible twin of a foreign phash: foreign twin returned", "False",
              str(F_SIMILAR in (d or {}).get("media", [])), leaked((d or {}).get("media", []))])
for kind in ("thumbnails", "originals"):
    c = get(f"media/{kind}/{F_PLAIN}")
    cases.append([f"stream {kind} foreign (http {c})", "denied", "denied" if c in (401, 403) else str(c), 0])
grid("direct lookups and streams (denied = 401/403 or any non-200)", cases)

# --- writes (each checked against the database afterwards) -------------------------------
cases = []
before = one(f"SELECT note FROM media WHERE media_uuid = '{F_PLAIN}'")
code, _ = call("UpdateMedia", {"media_uuid": F_PLAIN, "update": {"note": "[counttest] rbac overwritten"}})
after = one(f"SELECT note FROM media WHERE media_uuid = '{F_PLAIN}'")
cases.append([f"UpdateMedia foreign (http {code})", "unchanged", "unchanged" if after == before else "CHANGED", 0])
n = lambda c: one(f"SELECT count(*) FROM collection_contents WHERE collection_uuid = '{c}'")
n0 = n(C_BRIDGE)
code, _ = call("AddMediaToCollection", {"collection_uuid": C_BRIDGE, "media_uuid": F_PLAIN})
cases.append([f"AddMediaToCollection foreign media into a dev collection (http {code})", n0, n(C_BRIDGE), 0])
n0 = n(C_OTHER)
code, _ = call("RmMediaFromCollection", {"collection_uuid": C_OTHER, "media_uuid": V_IN_OTHER})
cases.append([f"RmMediaFromCollection from a foreign collection (http {code})", n0, n(C_OTHER), 0])
before = one(f"SELECT name FROM collections WHERE collection_uuid = '{C_OTHER}'")
code, _ = call("UpdateCollection", {"collection_uuid": C_OTHER, "update": {"name": "counttest-rbac-pwned"}})
cases.append([f"UpdateCollection foreign (http {code})", before,
              one(f"SELECT name FROM collections WHERE collection_uuid = '{C_OTHER}'"), 0])
grid("writes against foreign records (expected = database unchanged)", cases)

print("\nALL RBAC CASES MATCH" if FAILS == 0 else f"\n{FAILS} RBAC CASE(S) MISMATCH")
sys.exit(1 if FAILS else 0)
