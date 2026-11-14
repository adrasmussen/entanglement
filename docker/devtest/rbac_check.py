#!/usr/bin/env python3
# RBAC leak checks against the dev stack.
#
# Needs docker/devtest/seed_rbac.sql applied (records owned by counttest-othergroup, which
# the dev user is not in).  Every search, count, paging total, direct lookup, stream and
# write path is checked for out-of-group records.  Expected values come from raw SQL that
# applies the backend's visibility rule; the match column is computed here.
#
# visibility rule (common/src/db/*): a media item is visible to a user when it is not hidden
# and its library's gid, or the gid of a collection containing it, is one of the user's
# groups.  so the 10 foreign media in the devgroup collection counttest-rbac-bridge are
# visible by design (the positive control); every other foreign item must never surface.
#
# run from the repo root:  python3 -I docker/devtest/rbac_check.py
# exits non-zero on any mismatch.  the http client is curl, since the server speaks http/2.

import json
import os
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
    s = psql(q)
    return [l for l in s.split("\n") if l]


def call(ep, body, path="api"):
    for _ in range(5):
        p = subprocess.run(
            ["curl", "-sk", *CERT, "-o", "-", "-w", "\n%{http_code}", "-X", "POST",
             "-H", "content-type: application/json", "--data-binary", "@-", f"{U}/{path}/{ep}"],
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


def q(text=(), clauses=()):
    return {"Query": {"text": list(text), "clauses": [
        {"field": f, "op": o, "value": v} for f, o, v in clauses]}}


def opts(filt=None, limit=None, with_total=True, offset=0):
    return {"filter": filt or q(), "order": "DateDesc", "limit": limit, "offset": offset,
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
    fmt = lambda r: " | ".join(c.ljust(w[i]) for i, c in enumerate(r))
    print(fmt(hdr))
    print("-+-".join("-" * x for x in w))
    for r in body:
        print(fmt(r))


# --- fixture lookups ------------------------------------------------------------------
if psql("SELECT count(*) FROM libraries WHERE path = '/counttest-rbac'") != "1":
    sys.exit("seed missing: apply docker/devtest/seed_rbac.sql first")

L_OTHER = psql("SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac'")
L_CT = psql("SELECT library_uuid FROM libraries WHERE path = '/counttest'")
C_OTHER = psql("SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-other'")
C_BRIDGE = psql("SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-bridge'")
FOREIGN = set(rows(f"SELECT media_uuid FROM media WHERE library_uuid = '{L_OTHER}'"))
BRIDGED = set(rows(f"""SELECT m.media_uuid FROM media m JOIN collection_contents cc USING (media_uuid)
                       WHERE cc.collection_uuid = '{C_BRIDGE}' AND m.hidden = FALSE"""))
NEVER = FOREIGN - BRIDGED  # foreign media that must never surface for dev
F_PLAIN = psql(f"SELECT media_uuid FROM media WHERE path = '/counttest-rbac/rbac_00002.jpg'")
F_HIDDEN = psql(f"SELECT media_uuid FROM media WHERE path = '/counttest-rbac/rbac_00010.jpg'")
F_SIMILAR = psql(f"SELECT media_uuid FROM media WHERE path = '/counttest-rbac/rbac_00001.jpg'")
F_BRIDGED = psql(f"SELECT media_uuid FROM media WHERE path = '/counttest-rbac/rbac_00201.jpg'")
V_IN_OTHER = psql("SELECT media_uuid FROM media WHERE path = '/counttest/ct_00001.jpg'")
V_TWIN = psql("SELECT media_uuid FROM media WHERE path = '/counttest/ct_00007.jpg'")
COMMENT = psql("SELECT comment_uuid FROM comments WHERE text LIKE '[counttest] rbac%'")

VIS = f"""m.hidden = FALSE AND (
    m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '{GID}')
    OR EXISTS (SELECT 1 FROM collection_contents cc JOIN collections c USING (collection_uuid)
               WHERE cc.media_uuid = m.media_uuid AND c.gid = '{GID}'))"""


def raw_count(where):
    return int(psql(f"SELECT count(*) FROM media m WHERE {VIS} AND {where}"))


def leaked(uuids):
    return len(set(uuids) & NEVER)


print(f"fixtures: foreign={len(FOREIGN)} bridged(visible)={len(BRIDGED)} never-visible={len(NEVER)}")

# --- media searches --------------------------------------------------------------------
cases = []
code, d = call("SearchMedia", {"opts": opts(limit=None, with_total=True)})
cases.append(["SearchMedia empty: total", raw_count("TRUE"), d["meta"]["total"], leaked(d["media"])])
for label, flt, where in [
    ("tag:ct-rbac-secret", q(clauses=[("tag", "Has", "ct-rbac-secret")]), "m.tags ? 'ct-rbac-secret'"),
    ("tag:ct-mod7", q(clauses=[("tag", "Has", "ct-mod7")]), "m.tags ? 'ct-mod7'"),
    ("text rbac", q(text=["rbac"]), "m.ts_vec @@ to_tsquery('english', '''rbac''')"),
    ("description:lighthouse", q(clauses=[("description", "Contains", "lighthouse")]), "m.note ILIKE '%lighthouse%'"),
    ("old SubstringAny rbac", {"SubstringAny": {"filter": ["rbac"]}}, "m.ts_vec @@ to_tsquery('english', '''rbac''')"),
    # the hand-testing tags and phrases are on every foreign item too
    ("tag:sunshine", q(clauses=[("tag", "Has", "sunshine")]), "m.tags ? 'sunshine'"),
    ("tag:cheer", q(clauses=[("tag", "Has", "cheer")]), "m.tags ? 'cheer'"),
    ('description:"quiet morning"', q(clauses=[("description", "Contains", "quiet morning")]), "m.note ILIKE '%quiet morning%'"),
    ("text bakery (prefix)", q(text=["baker"]), "m.ts_vec @@ to_tsquery('english', '''baker'':*')"),
]:
    code, d = call("SearchMedia", {"opts": opts(flt, limit=None)})
    cases.append([f"SearchMedia {label}", raw_count(where), d["meta"]["total"], leaked(d["media"])])
    code, b = call("BatchSearchAndSort", {"req": {"Media": {"opts": opts(flt, limit=200)}}})
    ids = [r["media_uuid"] for r in b["media"]]
    cols = sum(C_OTHER in r["collections"] for r in b["media"])
    cases.append([f"BatchSearchAndSort {label}: total / foreign collection labels",
                  f"{raw_count(where)} / 0", f"{b['meta']['total']} / {cols}", leaked(ids)])
grid("media searches (expected = raw SQL under dev's groups)", cases)

# --- scoped searches -------------------------------------------------------------------
cases = []
for hidden in (None, False, True):
    code, d = call("SearchMediaInLibrary", {"library_uuid": L_OTHER, "hidden": hidden, "opts": opts()})
    cases.append([f"SearchMediaInLibrary foreign library hidden={hidden}", "0",
                  (d or {}).get("meta", {}).get("total", f"http {code}") if code == 200 else 0,
                  leaked((d or {}).get("media", []))])
code, d = call("SearchMediaInLibrary", {"library_uuid": L_OTHER, "hidden": None,
                                        "opts": opts(q(clauses=[("tag", "Has", "ct-rbac-secret")]))})
cases.append(["SearchMediaInLibrary foreign library tag:ct-rbac-secret", "0",
              (d or {}).get("meta", {}).get("total", 0) if code == 200 else 0, leaked((d or {}).get("media", []))])
code, d = call("SearchMediaInCollection", {"collection_uuid": C_OTHER, "opts": opts()})
cases.append(["SearchMediaInCollection foreign collection", "0",
              (d or {}).get("meta", {}).get("total", 0) if code == 200 else 0, leaked((d or {}).get("media", []))])
code, d = call("SearchMediaInCollection", {"collection_uuid": C_BRIDGE, "opts": opts()})
cases.append(["SearchMediaInCollection bridge (positive control)", len(BRIDGED), d["meta"]["total"],
              0 if set(d["media"]) == BRIDGED else 1])
code, d = call("SearchMediaInLibrary", {"library_uuid": L_CT, "hidden": None,
                                        "opts": opts(q(clauses=[("tag", "Has", "ct-rbac-secret")]))})
cases.append(["SearchMediaInLibrary counttest tag:ct-rbac-secret", "0", d["meta"]["total"], leaked(d["media"])])
grid("scoped searches", cases)

# --- collections and libraries ---------------------------------------------------------
cases = []


def coll_raw(where):
    return int(psql(f"SELECT count(*) FROM collections WHERE gid = '{GID}' AND {where}"))


for label, query, where in [
    ("empty", q(), "TRUE"),
    ("name:rbac", q(clauses=[("name", "Contains", "rbac")]), "name ILIKE '%rbac%'"),
    ("text rbac", q(text=["rbac"]), "name ILIKE '%rbac%'"),
    ("tag:ct-rbac-secret", q(clauses=[("tag", "Has", "ct-rbac-secret")]), "tags ? 'ct-rbac-secret'"),
    ("description:lighthouse", q(clauses=[("description", "Contains", "lighthouse")]), "note ILIKE '%lighthouse%'"),
]:
    code, d = call("SearchCollections", {"filter": "", "query": query["Query"], "limit": None, "with_total": True})
    lk = int(C_OTHER in (d or {}).get("collections", []))
    cases.append([f"SearchCollections {label}", coll_raw(where), (d or {}).get("meta", {}).get("total", f"http {code}"), lk])
code, d = call("SearchCollections", {"filter": "rbac", "limit": None, "with_total": True})
cases.append(["SearchCollections old filter 'rbac'", coll_raw("name LIKE '%rbac%'"), d["meta"]["total"],
              int(C_OTHER in d["collections"])])


def lib_raw(where):
    return int(psql(f"SELECT count(*) FROM libraries WHERE gid = '{GID}' AND {where}"))


for label, body, where in [
    ("empty", {"filter": "", "query": q()["Query"]}, "TRUE"),
    ("path:rbac", {"filter": "", "query": q(clauses=[("path", "Contains", "rbac")])["Query"]}, "path ILIKE '%rbac%'"),
    ("text counttest", {"filter": "", "query": q(text=["counttest"])["Query"]}, "path ILIKE '%counttest%'"),
    ("old filter 'rbac'", {"filter": "rbac"}, "path LIKE '%rbac%'"),
]:
    code, d = call("SearchLibraries", {**body, "limit": None, "with_total": True})
    lk = int(L_OTHER in (d or {}).get("libraries", []))
    cases.append([f"SearchLibraries {label}", lib_raw(where), (d or {}).get("meta", {}).get("total", f"http {code}"), lk])
grid("collections and libraries (expected = raw SQL under dev's groups)", cases)

# --- counts ----------------------------------------------------------------------------
scopes = [
    ("Media", "Media", raw_count("TRUE")),
    ("Collections", "Collections", coll_raw("TRUE")),
    ("Libraries", "Libraries", lib_raw("TRUE")),
    ("MediaInLibrary foreign null", {"MediaInLibrary": {"library_uuid": L_OTHER, "hidden": None}}, 0),
    ("MediaInLibrary foreign false", {"MediaInLibrary": {"library_uuid": L_OTHER, "hidden": False}}, 0),
    ("MediaInLibrary foreign true", {"MediaInLibrary": {"library_uuid": L_OTHER, "hidden": True}}, 0),
    ("MediaInCollection foreign", {"MediaInCollection": {"collection_uuid": C_OTHER}}, 0),
    ("MediaInCollection bridge", {"MediaInCollection": {"collection_uuid": C_BRIDGE}}, len(BRIDGED)),
]
code, d = call("GetCounts", {"scopes": [s for _, s, _ in scopes]})
got = [c["count"] for c in (d or {}).get("counts", [])] or [f"http {code}"] * len(scopes)
grid("GetCounts", [[f"GetCounts {n}", e, g, 0] for (n, _, e), g in zip(scopes, got)])

# --- direct lookups and streams --------------------------------------------------------
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
    code, d = call(ep, body)
    obs = "200" if code == 200 else "denied"
    cases.append([f"{label} (http {code})", expect, obs, 0])
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

# --- writes (each checked against the database afterwards) ----------------------------
cases = []
before = psql(f"SELECT note FROM media WHERE media_uuid = '{F_PLAIN}'")
code, _ = call("UpdateMedia", {"media_uuid": F_PLAIN, "update": {"note": "[counttest] rbac overwritten"}})
after = psql(f"SELECT note FROM media WHERE media_uuid = '{F_PLAIN}'")
cases.append([f"UpdateMedia foreign (http {code})", "unchanged", "unchanged" if after == before else "CHANGED", 0])

n0 = psql(f"SELECT count(*) FROM collection_contents WHERE collection_uuid = '{C_BRIDGE}'")
code, _ = call("AddMediaToCollection", {"collection_uuid": C_BRIDGE, "media_uuid": F_PLAIN})
n1 = psql(f"SELECT count(*) FROM collection_contents WHERE collection_uuid = '{C_BRIDGE}'")
cases.append([f"AddMediaToCollection foreign media into a dev collection (http {code})", n0, n1, 0])

n0 = psql(f"SELECT count(*) FROM collection_contents WHERE collection_uuid = '{C_OTHER}'")
code, _ = call("RmMediaFromCollection", {"collection_uuid": C_OTHER, "media_uuid": V_IN_OTHER})
n1 = psql(f"SELECT count(*) FROM collection_contents WHERE collection_uuid = '{C_OTHER}'")
cases.append([f"RmMediaFromCollection from a foreign collection (http {code})", n0, n1, 0])

before = psql(f"SELECT name FROM collections WHERE collection_uuid = '{C_OTHER}'")
code, _ = call("UpdateCollection", {"collection_uuid": C_OTHER, "update": {"name": "counttest-rbac-pwned"}})
after = psql(f"SELECT name FROM collections WHERE collection_uuid = '{C_OTHER}'")
cases.append([f"UpdateCollection foreign (http {code})", before, after, 0])
grid("writes against foreign records (expected = database unchanged)", cases)

print("\nALL RBAC CASES MATCH" if FAILS == 0 else f"\n{FAILS} RBAC CASE(S) MISMATCH")
sys.exit(1 if FAILS else 0)
