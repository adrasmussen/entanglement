#!/bin/zsh
# live API checks against the dev stack: every case runs raw SQL in the dev postgres
# container next to the endpoint(s), and the grids print raw values with the match
# column computed by the script (nothing is compared by eye).
#
#   zsh docker/devtest/api_check.sh
#
# needs the dev stack up (`docker compose up -d`) with docker/dev-certs/ generated, the
# test libraries (pagetest, bulktest) and collections (bulk-big) from the paging work,
# and for the counttest cases the /counttest library and counttest-cNNN collections.
# reads no .env; uuids are looked up by name, never hard-coded.  exits non-zero when
# any case does not match.
#
# the counttest records carry one identifier, so cleanup is (in this order):
#
#   DELETE FROM collection_contents WHERE collection_uuid IN
#       (SELECT collection_uuid FROM collections WHERE name LIKE 'counttest-%');
#   DELETE FROM collections WHERE name LIKE 'counttest-%';
#   DELETE FROM media WHERE library_uuid = (SELECT library_uuid FROM libraries WHERE path = '/counttest');
#   DELETE FROM libraries WHERE path = '/counttest';

cd ${0:A:h}/../.. || exit 1

C=(--cert docker/dev-certs/client.crt --key docker/dev-certs/client.key)
U=https://127.0.0.1:8444/entanglement/api
PG=entanglement-postgres-1

python3 -I - "$U" "$PG" "${C[@]}" <<'PY'
import json
import subprocess
import sys

U, PG, *C = sys.argv[1:]
GID = "devgroup"  # the dev user's group; the raw SQL below applies it like the backend
F = {"SubstringAny": {"filter": []}}
ORDERS = ["DateAsc", "DateDesc", "Path"]
FAILED = 0


def post(ep, body):
    """POST json, returning (http status, parsed body or raw text)."""
    # the body goes in on stdin, since the larger ones overflow the argument list
    # the dev server now and then drops an http2 stream (curl exit 16) on a fresh
    # connection, so transport failures are retried; http errors are not
    for attempt in range(5):
        run = subprocess.run(
            ["curl", "-sS", "-k", *C, "-w", "\n%{http_code}", "-X", "POST",
             "-H", "content-type: application/json", "--data-binary", "@-", f"{U}/{ep}"],
            input=json.dumps(body), capture_output=True, text=True,
        )
        if run.returncode == 0:
            break
        print(f"  (retry {attempt + 1}: curl exit {run.returncode} on {ep})", file=sys.stderr)
    else:
        sys.exit(f"curl failed for {ep} (exit {run.returncode}): {run.stderr.strip()}")
    out = run.stdout
    text, _, code = out.rpartition("\n")
    try:
        return int(code), json.loads(text)
    except ValueError:
        return int(code), text


def sql(q):
    out = subprocess.run(
        ["docker", "exec", "-i", PG, "psql", "-U", "entanglement", "-d", "entanglement",
         "-At", "-v", "ON_ERROR_STOP=1"],
        input=q, capture_output=True, text=True,
    )
    if out.returncode != 0:
        sys.exit(f"psql failed: {out.stderr.strip()}\n{q}")
    return out.stdout.strip()


def one(q):
    return int(sql(q))


def uuid_of(q, what):
    v = sql(q)
    if not v:
        sys.exit(f"could not look up {what}")
    return v


def opts(order="DateDesc", limit=None, offset=0, with_total=False):
    return {"filter": F, "order": order, "limit": limit, "offset": offset, "with_total": with_total}


def grid(title, header, rows):
    """print rows of strings as an aligned grid; the last column is the match column."""
    global FAILED
    FAILED += sum(1 for r in rows if r[-1] != "OK")
    cells = [header] + rows
    w = [max(len(str(r[i])) for r in cells) for i in range(len(header))]
    print(f"\n== {title}")
    for n, r in enumerate(cells):
        print(" | ".join(str(c).ljust(w[i]) for i, c in enumerate(r)).rstrip())
        if n == 0:
            print("-+-".join("-" * x for x in w))


def kv(d):
    return " ".join(f"{k}={v}" for k, v in d.items())


def check(rows, legend, name, sql_text, raw, obs, exp):
    """one case: `raw` is the raw SQL result (or '-'), obs/exp are dicts compared here."""
    legend.append((name, " ".join(sql_text.split())))
    rows.append((name, raw, kv(obs), kv(exp), "OK" if obs == exp else "FAIL"))


def show_legend(legend):
    print("\nraw SQL per case:")
    for name, text in legend:
        print(f"  {name}: {text}")


# ---- raw SQL, applying the same visibility rules as the backend ----

def sql_media_visible():
    # SearchMedia: not hidden, and in a library or a collection owned by one of the user's groups
    return f"""SELECT count(*) FROM media m WHERE m.hidden = FALSE AND (
        m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '{GID}')
        OR EXISTS (SELECT 1 FROM collection_contents cc JOIN collections c USING (collection_uuid)
                   WHERE cc.media_uuid = m.media_uuid AND c.gid = '{GID}'))"""


def sql_collections(like="%"):
    return f"SELECT count(*) FROM collections WHERE gid = '{GID}' AND name LIKE '{like}'"


def sql_libraries():
    return f"SELECT count(*) FROM libraries WHERE gid = '{GID}'"


def sql_media_in_library(lib, hidden):
    h = "" if hidden is None else f" AND m.hidden = {'TRUE' if hidden else 'FALSE'}"
    return f"""SELECT count(*) FROM media m
        WHERE m.library_uuid = '{lib}'
        AND m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '{GID}'){h}"""


def sql_media_in_collection(coll):
    # only visible media, in a collection owned by one of the user's groups
    return f"""SELECT count(*) FROM collection_contents cc
        JOIN collections c ON c.collection_uuid = cc.collection_uuid
        JOIN media m ON m.media_uuid = cc.media_uuid
        WHERE c.gid = '{GID}' AND c.collection_uuid = '{coll}' AND m.hidden = FALSE"""


# ---- lookups by name ----
BULK = uuid_of("SELECT library_uuid FROM libraries WHERE path = 'bulktest'", "library bulktest")
BIG = uuid_of("SELECT collection_uuid FROM collections WHERE name = 'bulk-big'", "collection bulk-big")
CT_LIB = sql("SELECT library_uuid FROM libraries WHERE path = '/counttest'")
CT_C007 = sql("SELECT collection_uuid FROM collections WHERE name = 'counttest-c007'")
CT_C000 = sql("SELECT collection_uuid FROM collections WHERE name = 'counttest-c000'")
NIL = "00000000-0000-0000-0000-000000000000"

N = one(sql_media_visible())
NC = one(sql_collections())
NL = one(sql_libraries())

# ---------------------------------------------------------------- phase 1: meta
rows, legend = [], []


def batch(**kw):
    code, d = post("BatchSearchAndSort", {"req": {"Media": {"opts": opts(**kw)}}})
    return d


d = batch(limit=60, with_total=True)
check(rows, legend, "batch first page", sql_media_visible(), N,
      {"media": len(d["media"]), "total": d["meta"]["total"], "row_count": d["meta"]["row_count"],
       "has_more": d["meta"]["has_more"], "legacy_total": d["total"], "page_len": d["page_len"]},
      {"media": 60, "total": N, "row_count": 60, "has_more": True, "legacy_total": N, "page_len": 60})

d = batch(limit=500)
check(rows, legend, "batch limit 500 clamps", "-", "-",
      {"limit": d["meta"]["limit"], "row_count": d["meta"]["row_count"], "has_more": d["meta"]["has_more"]},
      {"limit": 200, "row_count": 200, "has_more": True})

d = batch(limit=60, offset=N - 60)
check(rows, legend, "batch exact last page", sql_media_visible(), N,
      {"offset": N - 60, "row_count": d["meta"]["row_count"], "has_more": d["meta"]["has_more"]},
      {"offset": N - 60, "row_count": 60, "has_more": False})

d = batch(limit=60, offset=N - 61)
check(rows, legend, "batch one before last", sql_media_visible(), N,
      {"offset": N - 61, "row_count": d["meta"]["row_count"], "has_more": d["meta"]["has_more"]},
      {"offset": N - 61, "row_count": 60, "has_more": True})

code, d = post("SearchMedia", {"opts": opts(limit=0, with_total=True)})
check(rows, legend, "count-only SearchMedia", sql_media_visible(), N,
      {"total": d["meta"]["total"], "legacy_total": d["total"], "items": len(d["media"]),
       "row_count": d["meta"]["row_count"], "has_more": d["meta"]["has_more"]},
      {"total": N, "legacy_total": N, "items": 0, "row_count": 0, "has_more": None})

raw = one(sql_media_in_library(BULK, None))
code, d = post("SearchMediaInLibrary", {"library_uuid": BULK, "hidden": None, "opts": opts(limit=0, with_total=True)})
check(rows, legend, "count-only library bulktest", sql_media_in_library(BULK, None), raw,
      {"total": d["meta"]["total"], "items": len(d["media"])}, {"total": raw, "items": 0})

raw = one(sql_media_in_collection(BIG))
code, d = post("SearchMediaInCollection", {"collection_uuid": BIG, "opts": opts(limit=0, with_total=True)})
check(rows, legend, "count-only collection bulk-big", sql_media_in_collection(BIG), raw,
      {"total": d["meta"]["total"], "items": len(d["media"])}, {"total": raw, "items": 0})

code, d = post("SearchCollections", {"filter": "", "limit": 10, "offset": 0, "with_total": True})
check(rows, legend, "collections page 10 + total", sql_collections(), NC,
      {"total": d["meta"]["total"], "legacy_total": d["total"], "items": len(d["collections"]),
       "has_more": d["meta"]["has_more"]},
      {"total": NC, "legacy_total": NC, "items": min(10, NC), "has_more": NC > 10})

code, d = post("SearchCollections", {"filter": "", "limit": 0, "offset": 0, "with_total": True})
check(rows, legend, "collections count-only", sql_collections(), NC,
      {"total": d["meta"]["total"], "items": len(d["collections"])}, {"total": NC, "items": 0})

raw = one(sql_collections("%bulk big%"))
code, d = post("SearchCollections", {"filter": "bulk big", "limit": 10, "offset": 0, "with_total": True})
check(rows, legend, "collections filter with a space", sql_collections("%bulk big%"), raw,
      {"total": d["meta"]["total"]}, {"total": raw})

code, d = post("SearchLibraries", {"filter": ""})
check(rows, legend, "libraries, old client JSON", sql_libraries(), NL,
      {"items": len(d["libraries"]), "total": d["meta"]["total"]}, {"items": NL, "total": None})

code, d = post("SearchLibraries", {"filter": "", "limit": 1, "offset": 0, "with_total": True})
check(rows, legend, "libraries limit 1 + total", sql_libraries(), NL,
      {"total": d["meta"]["total"], "items": len(d["libraries"]), "has_more": d["meta"]["has_more"]},
      {"total": NL, "items": min(1, NL), "has_more": NL > 1})

grid("phase 1: paging meta (raw SQL vs endpoint)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# ---------------------------------------------------------------- phase 2: counts
rows, legend = [], []
# (label, scope for GetCounts, raw SQL, count-only search endpoint + body)
cases = [
    ("Media (visible)", "Media", sql_media_visible(),
     ("SearchMedia", {"opts": opts(limit=0, with_total=True)})),
    ("Collections", "Collections", sql_collections(),
     ("SearchCollections", {"filter": "", "limit": 0, "offset": 0, "with_total": True})),
    ("Libraries", "Libraries", sql_libraries(),
     ("SearchLibraries", {"filter": "", "limit": 0, "offset": 0, "with_total": True})),
]
libs = [("bulktest", BULK)] + ([("counttest", CT_LIB)] if CT_LIB else []) + [("unknown library", NIL)]
for label, lib in libs:
    for h in (None, False, True):
        cases.append((
            f"MediaInLibrary {label} hidden={'null' if h is None else str(h).lower()}",
            {"MediaInLibrary": {"library_uuid": lib, "hidden": h}},
            sql_media_in_library(lib, h),
            ("SearchMediaInLibrary", {"library_uuid": lib, "hidden": h, "opts": opts(limit=0, with_total=True)}),
        ))
colls = [("bulk-big", BIG)] + [(n, u) for n, u in (("counttest-c000", CT_C000), ("counttest-c007", CT_C007)) if u]
colls.append(("unknown collection", NIL))
for label, coll in colls:
    cases.append((
        f"MediaInCollection {label}",
        {"MediaInCollection": {"collection_uuid": coll}},
        sql_media_in_collection(coll),
        ("SearchMediaInCollection", {"collection_uuid": coll, "opts": opts(limit=0, with_total=True)}),
    ))

code, d = post("GetCounts", {"scopes": [c[1] for c in cases]})
by_pos = d["counts"] if code == 200 else []
for i, (label, scope, q, (ep, body)) in enumerate(cases):
    raw = one(q)
    sc, sd = post(ep, body)
    searched = sd["meta"]["total"] if sc == 200 else f"http {sc}"
    got = by_pos[i] if i < len(by_pos) else {"scope": None, "count": f"http {code}"}
    check(rows, legend, label, q, raw,
          {"GetCounts": got["count"], "scope_echoed": got["scope"] == scope, "count_only_search": searched},
          {"GetCounts": raw, "scope_echoed": True, "count_only_search": raw})
grid("phase 2: counts (raw SQL vs GetCounts vs count-only search; expected is the raw SQL value)",
     ("scope", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# ---- scope limits and ordering
rows, legend = [], []
sixteen = [{"MediaInLibrary": {"library_uuid": BULK, "hidden": None}}] * 8 + ["Libraries"] * 8
reverse = list(reversed([c[1] for c in cases[:3]]))
reverse_expect = [{"Libraries": NL, "Collections": NC, "Media": N}[s] for s in reverse]
for name, scopes, exp_code, exp_n in (
    ("17 scopes", sixteen + ["Media"], 400, None),
    ("16 scopes (the limit)", sixteen, 200, 16),
    ("empty list", [], 200, 0),
):
    code, d = post("GetCounts", {"scopes": scopes})
    n = len(d["counts"]) if code == 200 and isinstance(d, dict) else None
    check(rows, legend, name, "-", "-", {"http": code, "counts": n}, {"http": exp_code, "counts": exp_n})
code, d = post("GetCounts", {"scopes": reverse})
check(rows, legend, "order follows the request", "-", "-",
      {"http": code, "counts": [c["count"] for c in d["counts"]] if code == 200 else None},
      {"http": 200, "counts": reverse_expect})
grid("phase 2: scope limits (raw SQL column is '-', expected values are constants)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)

# ---------------------------------------------------------------- paging integrity
rows, legend = [], []
targets = [("SearchMedia", None, sql_media_visible(), N)]
if CT_LIB:
    targets.append(("SearchMediaInLibrary counttest hidden=null", CT_LIB,
                    sql_media_in_library(CT_LIB, None), one(sql_media_in_library(CT_LIB, None))))
for label, lib, q, raw in targets:
    for order in ORDERS:
        def fetch(**kw):
            if lib is None:
                return post("SearchMedia", {"opts": opts(order=order, **kw)})[1]
            return post("SearchMediaInLibrary", {"library_uuid": lib, "hidden": None, "opts": opts(order=order, **kw)})[1]
        full = fetch(limit=None)["media"]
        got, off, reqs = [], 0, 0
        while True:
            r = fetch(limit=200, offset=off)
            reqs += 1
            got += r["media"]
            off += len(r["media"])
            if not r["meta"]["has_more"]:
                break
        want_reqs = -(-raw // 200)
        check(rows, legend, f"{label} {order}", q, raw,
              {"unpaged": len(full), "paged": len(got), "equal": full == got,
               "dups": len(got) - len(set(got)), "requests": reqs},
              {"unpaged": raw, "paged": raw, "equal": True, "dups": 0, "requests": want_reqs})
grid("paging integrity: pages of 200 vs the unpaged list, per sort order",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

print(f"\n{'ALL CASES MATCH' if FAILED == 0 else f'{FAILED} CASE(S) FAILED'}")
sys.exit(1 if FAILED else 0)
PY
