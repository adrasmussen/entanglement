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
if psql("SELECT count(*) FROM libraries WHERE path IN ('/counttest-rbac', '/counttest-rbac-dev')") != "2":
    sys.exit("seed missing: apply docker/devtest/seed_rbac.sql first")

L_OTHER = psql("SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac'")
L_DEV = psql("SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac-dev'")
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
V_IN_OTHER = psql("SELECT media_uuid FROM media WHERE path = '/counttest-rbac-dev/dev_00001.jpg'")  # also in the foreign collection
V_TWIN = psql("SELECT media_uuid FROM media WHERE path = '/counttest-rbac-dev/dev_00007.jpg'")      # same phash as F_SIMILAR
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
code, d = call("SearchMediaInLibrary", {"library_uuid": L_DEV, "hidden": None,
                                        "opts": opts(q(clauses=[("tag", "Has", "ct-rbac-secret")]))})
cases.append(["SearchMediaInLibrary dev library tag:ct-rbac-secret", "0", d["meta"]["total"], leaked(d["media"])])
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
# a forbidden id is answered exactly like one that doesn't exist (404, empty body), so a
# caller can't learn which ids exist
UNKNOWN = "00000000-0000-7000-8000-00000000dead"
cases = []
for label, ep, body, expect in [
    ("GetMedia foreign", "GetMedia", {"media_uuid": F_PLAIN}, "404"),
    ("GetMedia foreign hidden", "GetMedia", {"media_uuid": F_HIDDEN}, "404"),
    ("GetMedia bridged (positive control)", "GetMedia", {"media_uuid": F_BRIDGED}, "200"),
    ("GetCollection foreign", "GetCollection", {"collection_uuid": C_OTHER}, "404"),
    ("GetLibrary foreign", "GetLibrary", {"library_uuid": L_OTHER}, "404"),
    ("GetComment on foreign media", "GetComment", {"comment_uuid": COMMENT}, "404"),
    ("SimilarMedia from a foreign source", "SimilarMedia", {"media_uuid": F_SIMILAR, "distance": 64}, "404"),
]:
    code, d = call(ep, body)
    cases.append([f"{label} (http {code})", expect, str(code), 0])
code, d = call("GetMedia", {"media_uuid": V_IN_OTHER})
cases.append(["GetMedia visible item in a foreign collection: foreign collection listed", "False",
              str(C_OTHER in (d or {}).get("collections", [])), 0])
code, d = call("SimilarMedia", {"media_uuid": V_TWIN, "distance": 1})
cases.append(["SimilarMedia from a visible twin of a foreign phash: foreign twin returned", "False",
              str(F_SIMILAR in (d or {}).get("media", [])), leaked((d or {}).get("media", []))])
for kind in ("thumbnails", "originals"):
    c = get(f"media/{kind}/{F_PLAIN}")
    cases.append([f"stream {kind} foreign (http {c})", "404", str(c), 0])
grid("direct lookups and streams (forbidden = 404)", cases)


def raw(ep, body):
    # status and exact body bytes
    p = subprocess.run(
        ["curl", "-sk", *CERT, "-o", "-", "-w", "\n%{http_code}", "-X", "POST",
         "-H", "content-type: application/json", "--data-binary", "@-", f"{U}/api/{ep}"],
        input=json.dumps(body).encode(), capture_output=True)
    text, _, code = p.stdout.decode().rpartition("\n")
    return code, text


cases = []
for label, ep, field, forbidden in [
    ("GetMedia", "GetMedia", "media_uuid", F_PLAIN),
    ("SimilarMedia", "SimilarMedia", "media_uuid", F_PLAIN),
    ("GetCollection", "GetCollection", "collection_uuid", C_OTHER),
    ("UpdateCollection", "UpdateCollection", "collection_uuid", C_OTHER),
    ("DeleteCollection", "DeleteCollection", "collection_uuid", C_OTHER),
    ("GetLibrary", "GetLibrary", "library_uuid", L_OTHER),
    ("GetComment", "GetComment", "comment_uuid", COMMENT),
    ("UpdateComment", "UpdateComment", "comment_uuid", COMMENT),
    ("DeleteComment", "DeleteComment", "comment_uuid", COMMENT),
]:
    extra = {"update": {"name": None, "note": None, "tags": None}} if ep == "UpdateCollection" else \
            {"text": "[counttest] rbac overwritten"} if ep == "UpdateComment" else \
            {"distance": 1} if ep == "SimilarMedia" else {}
    f = raw(ep, {field: forbidden, **extra})
    u = raw(ep, {field: UNKNOWN, **extra})
    cases.append([f"{label}: forbidden {f} vs unknown {u}", "same 404", "same 404" if f == u == ("404", "") else "DIFFERENT", 0])
for kind in ("thumbnails", "originals"):
    f, u = get(f"media/{kind}/{F_PLAIN}"), get(f"media/{kind}/{UNKNOWN}")
    cases.append([f"stream {kind}: forbidden {f} vs unknown {u}", "same 404", "same 404" if f == u == 404 else "DIFFERENT", 0])
for label, ep, body in [
    ("StartTask on a foreign library", "StartTask", {"library_uuid": L_OTHER, "task_type": "ScanLibrary"}),
    ("StartTask on an unknown library", "StartTask", {"library_uuid": UNKNOWN, "task_type": "ScanLibrary"}),
    ("AddMediaToCollection, unknown media", "AddMediaToCollection", {"collection_uuid": C_BRIDGE, "media_uuid": UNKNOWN}),
]:
    code, _ = raw(ep, body)
    cases.append([f"{label} (http {code})", "404", code, 0])
grid("unknown vs forbidden ids (expected = the same 404 with an empty body)", cases)

# --- GetSlugs: the tag values the filters offer ---------------------------------------
# every slug must be a tag of a record dev can see, counted over visible records only
FORBIDDEN_MEDIA_TAGS = {"ct-rbac-foreign-only", "ct-rbac-hidden-only"}
FORBIDDEN_COLL_TAGS = {"ct-rbac-foreign-coll"}


def slug_rows(sql):
    return [tuple(r.split("|")) for r in rows(sql)]


def slugs(kind, prefix="", limit=200):
    code, d = call("GetSlugs", {"kind": kind, "field": "tag", "prefix": prefix, "limit": limit})
    return code, [(s["value"], str(s["count"])) for s in (d or {}).get("slugs", [])]


cases = []
for prefix in ("", "ct-rbac"):
    like = prefix.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "%"
    raw = slug_rows(f"""SELECT k, count(*) FROM media m, skeys(m.tags) k
                        WHERE {VIS} AND lower(k) LIKE '{like}'
                        GROUP BY k ORDER BY count(*) DESC, k COLLATE "C" LIMIT 200""")
    code, got = slugs("Media", prefix)
    bad = len({v for v, _ in got} & FORBIDDEN_MEDIA_TAGS)
    cases.append([f"media tags, prefix {prefix!r} (http {code}): list vs raw SQL", len(raw),
                  len(got) if got == raw else f"differs ({len(got)} returned)", bad])
code, got = slugs("Media", "ct-rbac")
counts = dict(got)
cases.append(["media tag ct-rbac-secret counted over the 10 bridged items only",
              raw_count("m.tags ? 'ct-rbac-secret'"), counts.get("ct-rbac-secret"), 0])
cases.append(["media tags offered from foreign or hidden records", 0,
              len(set(counts) & FORBIDDEN_MEDIA_TAGS), len(set(counts) & FORBIDDEN_MEDIA_TAGS)])
raw = slug_rows(f"""SELECT k, count(*) FROM collections c, skeys(c.tags) k WHERE c.gid = '{GID}'
                    GROUP BY k ORDER BY count(*) DESC, k COLLATE "C" LIMIT 200""")
code, got = slugs("Collection")
bad = len({v for v, _ in got} & FORBIDDEN_COLL_TAGS)
cases.append([f"collection tags (http {code}): list vs raw SQL", len(raw),
              len(got) if got == raw else f"differs ({len(got)} returned)", bad])
cases.append(["collection tags offered from the foreign collection", 0, bad, bad])
# any-of searches on tags only foreign or hidden records carry find nothing, and mixing in a
# visible tag finds exactly the visible ones
for label, value in [("foreign-only,hidden-only", "ct-rbac-foreign-only,ct-rbac-hidden-only"),
                     ("foreign-only,dev", "ct-rbac-foreign-only,ct-rbac-dev")]:
    code, d = call("SearchMedia", {"opts": opts(q(clauses=[("tag", "HasAny", value)]))})
    exp = raw_count("m.tags ?| ARRAY[" + ",".join(f"'{t}'" for t in value.split(",")) + "]")
    cases.append([f"SearchMedia tag:{label} (any of)", exp,
                  (d or {}).get("meta", {}).get("total", f"http {code}"), leaked((d or {}).get("media", []))])
code, d = call("SearchCollections", {"filter": "", "query": q(clauses=[("tag", "HasAny", "ct-rbac-foreign-coll,ct-rbac-bridge")])["Query"],
                                     "limit": None, "with_total": True})
cases.append(["SearchCollections tag:ct-rbac-foreign-coll,ct-rbac-bridge (any of)",
              coll_raw("tags ?| ARRAY['ct-rbac-foreign-coll','ct-rbac-bridge']"),
              (d or {}).get("meta", {}).get("total", f"http {code}"),
              int(C_OTHER in (d or {}).get("collections", []))])
grid("GetSlugs (expected = raw SQL over dev-visible records)", cases)

# --- phase 5b: matching inside words, name:, and OmniSearch -------------------------
# the foreign records carry 'lighthouse' in their notes, ct-rbac-foreign-only in their tags
# and rbac_ in their file names; dev sees only the bridged ones
cases = []
TXT = "lower(m.date || ' ' || m.note || ' ' || array_to_string(akeys(m.tags), ' '))"
for label, text in [("inside a word: ighthous", "ighthous"),
                    ("inside a foreign-only tag: eign-onl", "eign-onl")]:
    code, d = call("SearchMedia", {"opts": opts(q(text=[text]))})
    cases.append([f"SearchMedia words {label}", raw_count(f"position('{text}' in {TXT}) > 0"),
                  (d or {}).get("meta", {}).get("total", f"http {code}"), leaked((d or {}).get("media", []))])
code, d = call("SearchMedia", {"opts": opts(q(clauses=[("name", "Contains", "rbac_")]))})
cases.append(["SearchMedia name:rbac_ (foreign file names)",
              raw_count("position('rbac_' in regexp_replace(m.path, '^.*/', '')) > 0"),
              (d or {}).get("meta", {}).get("total", f"http {code}"), leaked((d or {}).get("media", []))])
unfiltered = int(psql(f"SELECT count(*) FROM media m WHERE position('eign-onl' in {TXT}) > 0"))
cases.append(["(the foreign-only infix exists unfiltered: non-zero)", "non-zero",
              "non-zero" if unfiltered > 0 else "zero", 0])


def omni_groups(text, **kw):
    code, d = call("OmniSearch", {"query": text, **kw})
    return code, {g["kind"]: g for g in (d or {}).get("groups", [])}


def hit_ids(g):
    return [list(h["id"].values())[0] for h in g.get("hits", [])]


for text in ("lighthouse", "rbac", "ct-rbac-secret", "tag:ct-rbac-secret", "name:rbac_", "eign-onl"):
    code, groups = omni_groups(text, per_group=20)
    mi, ci, li = (hit_ids(groups.get(k, {})) for k in ("Media", "Collection", "Library"))
    cases.append([f"OmniSearch {text!r} (http {code}): foreign-only media among hits", 0,
                  len(set(mi) & NEVER), len(set(mi) & NEVER)])
    cases.append([f"OmniSearch {text!r}: foreign collection / library among hits", "False/False",
                  f"{C_OTHER in ci}/{L_OTHER in li}", int(C_OTHER in ci) + int(L_OTHER in li)])

# totals over visible records only
LIGHT = f"(m.ts_vec @@ to_tsquery('english', quote_literal('lighthouse') || ':*') OR position('lighthouse' in {TXT}) > 0)"
code, groups = omni_groups("lighthouse")
cases.append(["OmniSearch lighthouse: media total = visible raw SQL", raw_count(LIGHT),
              groups.get("Media", {}).get("meta", {}).get("total"), 0])
cases.append(["OmniSearch lighthouse: collection total = devgroup raw SQL", coll_raw("name ILIKE '%lighthouse%'"),
              groups.get("Collection", {}).get("meta", {}).get("total"), 0])
code, groups = omni_groups("eign-onl")
cases.append(["OmniSearch eign-onl: media total (foreign-only infix) = visible raw SQL",
              raw_count(f"position('eign-onl' in {TXT}) > 0"), groups.get("Media", {}).get("meta", {}).get("total"), 0])
code, groups = omni_groups("rbac")
cases.append(["OmniSearch rbac: library total = devgroup raw SQL", lib_raw("path ILIKE '%rbac%'"),
              groups.get("Library", {}).get("meta", {}).get("total"), int(L_OTHER in hit_ids(groups.get("Library", {})))])
grid("phase 5b: inside-word matching, name: and OmniSearch (expected = raw SQL over visible records)", cases)

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

# a new collection's cover must be media the creator can see
COVER_NAME = "counttest-rbac-cover"
psql(f"DELETE FROM collections WHERE name = '{COVER_NAME}'")
new = lambda cover: {"collection": {"uid": "dev", "gid": GID, "name": COVER_NAME, "note": "", "tags": [], "cover": cover}}
for label, cover, expect_code, expect_rows in [
    ("AddCollection with a foreign cover", F_PLAIN, 404, "0"),
    ("AddCollection with a foreign hidden cover", F_HIDDEN, 404, "0"),
    ("AddCollection with an unknown cover", UNKNOWN, 404, "0"),
    ("AddCollection with a bridged cover (positive control)", F_BRIDGED, 200, "1"),
]:
    code, _ = call("AddCollection", new(cover))
    n = psql(f"SELECT count(*) FROM collections WHERE name = '{COVER_NAME}'")
    psql(f"DELETE FROM collections WHERE name = '{COVER_NAME}'")
    cases.append([f"{label} (http {code}): collections created", f"{expect_code}/{expect_rows}", f"{code}/{n}", 0])
grid("writes against foreign records (expected = database unchanged)", cases)

# --- hostile search text: a 400 with a reason, never a 500 ------------------------------
# no NUL anywhere, and at most 100 characters of search text in any filter form
LONG, CAP = "x" * 101, "x" * 100
cases = []
for label, ep, body, expect in [
    ("SearchMedia query word with NUL", "SearchMedia", {"opts": opts(q(["be\0ach"]))}, 400),
    ("SearchMedia query tag with NUL", "SearchMedia", {"opts": opts(q(clauses=[("tag", "Has", "a\0")]))}, 400),
    ("SearchMedia legacy SubstringAny with NUL", "SearchMedia", {"opts": opts({"SubstringAny": {"filter": ["a\0"]}})}, 400),
    ("SearchMedia legacy SubstringAll, 103 characters of terms", "SearchMedia",
     {"opts": opts({"SubstringAll": {"filter": [f"t{i:02}" for i in range(26)]}})}, 400),
    ("SearchMedia legacy Fulltext 101 characters", "SearchMedia", {"opts": opts({"Fulltext": {"filter": LONG}})}, 400),
    ("SearchMedia query of 101 characters", "SearchMedia", {"opts": opts(q([LONG]))}, 400),
    ("SearchMedia query of exactly 100 characters", "SearchMedia", {"opts": opts(q([CAP]))}, 200),
    ("SearchCollections literal with NUL", "SearchCollections", {"filter": "a\0"}, 400),
    ("SearchCollections literal of 101 characters", "SearchCollections", {"filter": LONG}, 400),
    ("SearchLibraries literal of 100 characters", "SearchLibraries", {"filter": CAP}, 200),
    ("OmniSearch with NUL", "OmniSearch", {"query": "beach\0"}, 400),
    ("OmniSearch of 101 characters", "OmniSearch", {"query": LONG}, 400),
    ("OmniSearch of 100 characters", "OmniSearch", {"query": CAP}, 200),
    ("GetSlugs prefix with NUL", "GetSlugs", {"kind": "Media", "field": "tag", "prefix": "a\0"}, 400),
]:
    code, _ = call(ep, body)
    cases.append([f"{label} (http {code})", expect, code, 0])
grid("hostile search text (expected = 400, never 500)", cases)

print("\nALL RBAC CASES MATCH" if FAILS == 0 else f"\n{FAILS} RBAC CASE(S) MISMATCH")
sys.exit(1 if FAILS else 0)
