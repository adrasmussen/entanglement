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
# phase 3 (structured queries) also needs the tags and description words of
# docker/devtest/seed_search.sql, which touches only the counttest media.  phase 4 (collection
# and library queries) needs the same file's counttest collections: notes, tags, a phrase name
# and one collection per hostile value, all named counttest-%, so the cleanup below finds them.
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


def sql(q, **vars_):
    """run psql on q; vars_ are psql variables, used in q as :'name' (quoted as a literal)."""
    out = subprocess.run(
        ["docker", "exec", "-i", PG, "psql", "-U", "entanglement", "-d", "entanglement",
         "-At", "-v", "ON_ERROR_STOP=1", *[a for k, v in vars_.items() for a in ("-v", f"{k}={v}")]],
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


def opts(order="DateDesc", limit=None, offset=0, with_total=False, filt=F):
    return {"filter": filt, "order": order, "limit": limit, "offset": offset, "with_total": with_total}


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

def sql_media_visible(extra=""):
    # SearchMedia: not hidden, and in a library or a collection owned by one of the user's groups
    return f"""SELECT count(*) FROM media m WHERE m.hidden = FALSE AND (
        m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '{GID}')
        OR EXISTS (SELECT 1 FROM collection_contents cc JOIN collections c USING (collection_uuid)
                   WHERE cc.media_uuid = m.media_uuid AND c.gid = '{GID}')){extra}"""


def sql_collections(like="%"):
    return f"SELECT count(*) FROM collections WHERE gid = '{GID}' AND name LIKE '{like}'"


def sql_libraries():
    return f"SELECT count(*) FROM libraries WHERE gid = '{GID}'"


def sql_media_in_library(lib, hidden, extra=""):
    h = "" if hidden is None else f" AND m.hidden = {'TRUE' if hidden else 'FALSE'}"
    return f"""SELECT count(*) FROM media m
        WHERE m.library_uuid = '{lib}'
        AND m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '{GID}'){h}{extra}"""


def sql_media_in_collection(coll, extra=""):
    # only visible media, in a collection owned by one of the user's groups
    return f"""SELECT count(*) FROM collection_contents cc
        JOIN collections c ON c.collection_uuid = cc.collection_uuid
        JOIN media m ON m.media_uuid = cc.media_uuid
        WHERE c.gid = '{GID}' AND c.collection_uuid = '{coll}' AND m.hidden = FALSE{extra}"""


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
# whole-table row counts, every group included, for the "intact afterwards" checks
NC_ALL = one("SELECT count(*) FROM collections")
NL_ALL = one("SELECT count(*) FROM libraries")

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

# ---------------------------------------------------------------- phase 3: structured queries
# the counttest media carry the tags and description words of docker/devtest/seed_search.sql
if not one("SELECT count(*) FROM media WHERE note LIKE '[counttest]%' AND tags ? 'ct-even'"):
    sys.exit("phase 3 needs the search seed: docker exec -i " + PG +
             " psql -U entanglement -d entanglement -v ON_ERROR_STOP=1 < docker/devtest/seed_search.sql")


def query(text, *clauses):
    """a SearchFilter::Query from free-text words and (field, op, value) clauses."""
    return {"Query": {"text": list(text), "clauses": [{"field": f, "op": o, "value": v} for f, o, v in clauses]}}


# raw SQL conditions on media m, written independently of the resolver's templates (position()
# and left() instead of ILIKE and LIKE, plainto_tsquery instead of the quoted lexemes) and
# taking each value as a psql variable, so that nothing is spliced into the sql text
def raw_tag(var):
    return f"m.tags ? :'{var}'"


def raw_description(var):
    return f"position(lower(:'{var}') in lower(m.note)) > 0"


def raw_date(op, var, var2=None):
    def prefix(v):
        return f"left(m.date, length(:'{v}')) = :'{v}'"
    return {
        "Eq": prefix(var),
        "Gt": f"(m.date > :'{var}' AND NOT {prefix(var)})",
        "Ge": f"m.date >= :'{var}'",
        "Lt": f"m.date < :'{var}'",
        "Le": f"(m.date <= :'{var}' OR {prefix(var)})",
        "Range": f"(m.date >= :'{var}' AND (m.date <= :'{var2}' OR {prefix(var2)}))",
    }[op]


def raw_type(var):
    return f"m.media_type::text = :'{var}'"


def raw_text(*vars_):
    """plain words of a structured query: each word is a prefix, ORed (phase 4)."""
    return "m.ts_vec @@ (" + " || ".join(f"to_tsquery('english', quote_literal(:'{v}') || ':*')" for v in vars_) + ")"


def raw_text_whole(*vars_):
    """the older SubstringAny filter: whole words, ORed."""
    return "m.ts_vec @@ (" + " || ".join(f"plainto_tsquery('english', :'{v}')" for v in vars_) + ")"


def count_with(scope_sql, conds, **vars_):
    """the raw SQL count (and its text): the scope's visibility rules plus the conditions."""
    text = scope_sql("".join(f" AND {c}" for c in conds))
    return int(sql(text, **vars_)), text


# scope -> (raw SQL with extra conditions, count/search request, BatchSearchAndSort request)
SCOPES = {
    "gallery": (
        sql_media_visible,
        lambda f, **kw: ("SearchMedia", {"opts": opts(filt=f, **kw)}),
        lambda f, **kw: {"Media": {"opts": opts(filt=f, **kw)}},
    ),
    "counttest library": (
        lambda extra: sql_media_in_library(CT_LIB, None, extra),
        lambda f, **kw: ("SearchMediaInLibrary", {"library_uuid": CT_LIB, "hidden": None, "opts": opts(filt=f, **kw)}),
        lambda f, **kw: {"Library": {"library_uuid": CT_LIB, "hidden": None, "opts": opts(filt=f, **kw)}},
    ),
    "counttest-c007": (
        lambda extra: sql_media_in_collection(CT_C007, extra),
        lambda f, **kw: ("SearchMediaInCollection", {"collection_uuid": CT_C007, "opts": opts(filt=f, **kw)}),
        lambda f, **kw: {"Collection": {"collection_uuid": CT_C007, "opts": opts(filt=f, **kw)}},
    ),
}
if not (CT_LIB and CT_C007):
    sys.exit("phase 3 needs the counttest library and collections")

# (case, scope, text words, clauses, raw conditions, psql vars)
P3 = [
    ("empty query", "gallery", [], [], [], {}),
    ("empty query", "counttest library", [], [], [], {}),
    ("text: one word", "gallery", ["lighthouse"], [], [raw_text("w0")], {"w0": "lighthouse"}),
    ("text: two words (OR)", "gallery", ["lighthouse", "beach"], [], [raw_text("w0", "w1")],
     {"w0": "lighthouse", "w1": "beach"}),
    ("tag:ct-mod7", "gallery", [], [("tag", "Has", "ct-mod7")], [raw_tag("v0")], {"v0": "ct-mod7"}),
    ("tag:ct-even", "counttest library", [], [("tag", "Has", "ct-even")], [raw_tag("v0")], {"v0": "ct-even"}),
    ("tag:ct-odd", "counttest-c007", [], [("tag", "Has", "ct-odd")], [raw_tag("v0")], {"v0": "ct-odd"}),
    ("tag:ct-nonesuch", "gallery", [], [("tag", "Has", "ct-nonesuch")], [raw_tag("v0")], {"v0": "ct-nonesuch"}),
    ("description:lighthouse", "gallery", [], [("description", "Contains", "lighthouse")],
     [raw_description("v0")], {"v0": "lighthouse"}),
    ("description:BEACH (case)", "counttest library", [], [("description", "Contains", "BEACH")],
     [raw_description("v0")], {"v0": "BEACH"}),
    ("date:2022-04 (prefix)", "gallery", [], [("date", "Eq", "2022-04")], [raw_date("Eq", "v0")], {"v0": "2022-04"}),
    ("date:>2024", "gallery", [], [("date", "Gt", "2024")], [raw_date("Gt", "v0")], {"v0": "2024"}),
    ("date:>=2024-06", "gallery", [], [("date", "Ge", "2024-06")], [raw_date("Ge", "v0")], {"v0": "2024-06"}),
    ("date:<2021", "gallery", [], [("date", "Lt", "2021")], [raw_date("Lt", "v0")], {"v0": "2021"}),
    ("date:<=2021-03", "counttest library", [], [("date", "Le", "2021-03")], [raw_date("Le", "v0")],
     {"v0": "2021-03"}),
    ("date:2021-01..2021-06", "gallery", [], [("date", "Range", "2021-01..2021-06")],
     [raw_date("Range", "v0", "v1")], {"v0": "2021-01", "v1": "2021-06"}),
    ("type:Image", "gallery", [], [("type", "Eq", "Image")], [raw_type("v0")], {"v0": "Image"}),
    ("type:Video", "gallery", [], [("type", "Eq", "Video")], [raw_type("v0")], {"v0": "Video"}),
    ("tag + date range", "gallery", [], [("tag", "Has", "ct-mod7"), ("date", "Range", "2021-01..2023-06")],
     [raw_tag("v0"), raw_date("Range", "v1", "v2")], {"v0": "ct-mod7", "v1": "2021-01", "v2": "2023-06"}),
    ("text + tag", "counttest library", ["beach"], [("tag", "Has", "ct-even")],
     [raw_text("w0"), raw_tag("v0")], {"w0": "beach", "v0": "ct-even"}),
    ("text + description + type", "gallery", ["sunset", "forest"],
     [("description", "Contains", "item"), ("type", "Eq", "Image")],
     [raw_text("w0", "w1"), raw_description("v0"), raw_type("v1")],
     {"w0": "sunset", "w1": "forest", "v0": "item", "v1": "Image"}),
]

rows, legend, raws = [], [], {}
for name, scope, text, clauses, conds, vars_ in P3:
    scope_sql, search, batch_req = SCOPES[scope]
    raw, raw_sql = count_with(scope_sql, conds, **vars_)
    raws[(name, scope)] = raw
    f = query(text, *clauses)
    ep, body = search(f, limit=0, with_total=True)
    sc, sd = post(ep, body)
    counted = sd["meta"]["total"] if sc == 200 else f"http {sc}"
    bc, bd = post("BatchSearchAndSort", {"req": batch_req(f, limit=60, with_total=True)})
    batched = bd["meta"]["total"] if bc == 200 else f"http {bc}"
    check(rows, legend, f"{name} [{scope}]", f"{raw_sql} {kv(vars_)}", raw,
          {"count_only": counted, "batch_total": batched},
          {"count_only": raw, "batch_total": raw})
grid("phase 3: structured queries (raw SQL vs search count-only vs BatchSearchAndSort total)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# the seed makes these non-empty (or, for Video, empty), so agreement on zero cannot pass for success
rows, legend = [], []
for name, scope in (("tag:ct-mod7", "gallery"), ("description:lighthouse", "gallery"),
                    ("date:2022-04 (prefix)", "gallery"), ("tag + date range", "gallery"),
                    ("text: two words (OR)", "gallery"), ("type:Image", "gallery"),
                    ("tag:ct-odd", "counttest-c007")):
    raw = raws[(name, scope)]
    check(rows, legend, f"{name} [{scope}] is non-empty", "(raw count above)", raw,
          {"nonzero": raw > 0}, {"nonzero": True})
raw = raws[("type:Video", "gallery")]
check(rows, legend, "type:Video [gallery] is empty", "(raw count above)", raw, {"zero": raw == 0}, {"zero": True})
grid("phase 3: the cases above are not vacuous", ("case", "raw SQL", "observed", "expected", "match"), rows)

# ---------------------------------------------------------------- rejections and safety
rows, legend = [], []
ENDPOINTS = [
    ("SearchMedia", lambda f: ("SearchMedia", {"opts": opts(filt=f, limit=0, with_total=True)})),
    ("SearchMediaInCollection", lambda f: (
        "SearchMediaInCollection", {"collection_uuid": BIG, "opts": opts(filt=f, limit=0, with_total=True)})),
    ("SearchMediaInLibrary", lambda f: (
        "SearchMediaInLibrary", {"library_uuid": BULK, "hidden": None, "opts": opts(filt=f, limit=0, with_total=True)})),
    ("BatchSearchAndSort", lambda f: ("BatchSearchAndSort", {"req": {"Media": {"opts": opts(filt=f, limit=10)}}})),
]
BAD = [
    ("unknown field", query([], ("color", "Eq", "red"))),
    ("field of another class (path)", query([], ("path", "Contains", "x"))),
    ("bad op: tag >", query([], ("tag", "Gt", "x"))),
    ("bad op: description =", query([], ("description", "Eq", "x"))),
    ("bad enum value", query([], ("type", "Eq", "Pdf"))),
    ("bad date value", query([], ("date", "Eq", "2020'; DROP TABLE media; --"))),
    ("range without both ends", query([], ("date", "Range", "2020"))),
    ("tag containing the separator", query([], ("tag", "Has", "a|b"))),
    ("empty value", query([], ("tag", "Has", ""))),
]
for name, f in BAD:
    for label, build in ENDPOINTS:
        ep, body = build(f)
        code, d = post(ep, body)
        check(rows, legend, f"{name} -> {label}", "-", "-", {"http": code}, {"http": 400})
grid("phase 3: rejections (raw SQL column is '-', expected values are constants)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)

# hostile values are matched literally: the endpoint's count equals raw SQL on the literal value,
# bound by psql.  items 1001..1006 of the counttest library carry each of them (seed_search.sql),
# so the counts are not all zero
EVIL = ["'", "%", "_", "\\", "'; DROP TABLE media; --", "$1"]
rows, legend = [], []
for field, op, raw_fn in (("description", "Contains", raw_description), ("tag", "Has", raw_tag)):
    for evil in EVIL:
        raw, raw_sql = count_with(sql_media_visible, [raw_fn("v0")], v0=evil)
        f = query([], (field, op, evil))
        code, d = post("SearchMedia", {"opts": opts(filt=f, limit=0, with_total=True)})
        bc, bd = post("BatchSearchAndSort", {"req": {"Media": {"opts": opts(filt=f, limit=10, with_total=True)}}})
        check(rows, legend, f"{field}:{evil}", f"{raw_sql} {kv({'v0': evil})}", raw,
              {"http": code, "count_only": d["meta"]["total"] if code == 200 else None,
               "batch_http": bc, "batch_total": bd["meta"]["total"] if bc == 200 else None},
              {"http": 200, "count_only": raw, "batch_http": 200, "batch_total": raw})
# and as free text, which goes through the tsquery quoting rather than a LIKE pattern
for evil in EVIL:
    code, d = post("SearchMedia", {"opts": opts(filt=query([evil]), limit=0, with_total=True)})
    check(rows, legend, f"text:{evil}", "-", "-", {"http": code}, {"http": 200})
check(rows, legend, "media table intact afterwards", "SELECT count(*) > 0 FROM media", "-",
      {"has_rows": one("SELECT count(*) FROM media") > 0}, {"has_rows": True})
grid("phase 3: hostile values (raw SQL on the literal vs endpoint; free text only expects http 200)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# phase 4: plain words of a structured query match as prefixes.  the seed puts checkpoint /
# checko / checkpt on counttest items 2001..2003; whole words would match none of them for
# "check", and "eckp" (inside a word) is a phase 5 pg_trgm case, so it matches nothing yet
rows, legend = [], []
for words, expect_nonzero in ((["check"], True), (["checkp"], True), (["checkpoint"], True),
                              (["lighthou"], True), (["check", "sunse"], True), (["eckp"], False)):
    vs = {f"w{i}": w for i, w in enumerate(words)}
    want, raw_sql = count_with(sql_media_visible, [raw_text(*vs)], **vs)
    whole, _ = count_with(sql_media_visible, [raw_text_whole(*vs)], **vs)
    code, d = post("SearchMedia", {"opts": opts(filt=query(words), limit=0, with_total=True)})
    got = d["meta"]["total"] if code == 200 else f"http {code}"
    check(rows, legend, f"text: {' '.join(words)}", raw_sql + " " + kv(vs), want,
          {"count_only": got, "nonzero": (got or 0) > 0 if isinstance(got, int) else None, "whole_word_count": whole},
          {"count_only": want, "nonzero": expect_nonzero, "whole_word_count": whole})
grid("phase 4: plain words match as prefixes (raw SQL uses 'word':*)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# the older request shapes are untouched
rows, legend = [], []
old_any = {"SubstringAny": {"filter": ["lighthouse", "beach"]}}
want, raw_sql = count_with(sql_media_visible, [raw_text_whole("w0", "w1")], w0="lighthouse", w1="beach")
code, d = post("SearchMedia", {"opts": opts(filt=old_any, limit=0, with_total=True)})
check(rows, legend, "SubstringAny stays whole words (OR)", raw_sql, want,
      {"SubstringAny": d["meta"]["total"] if code == 200 else f"http {code}"}, {"SubstringAny": want})
want, raw_sql = count_with(sql_media_visible, [raw_text("w0", "w1")], w0="lighthouse", w1="beach")
code2, d2 = post("SearchMedia", {"opts": opts(filt=query(["lighthouse", "beach"]), limit=0, with_total=True)})
check(rows, legend, "Query text: prefixes (OR)", raw_sql, want,
      {"Query": d2["meta"]["total"] if code2 == 200 else f"http {code2}"}, {"Query": want})
code, d = post("SearchMedia", {"opts": opts(limit=0, with_total=True)})
check(rows, legend, "empty SubstringAny", sql_media_visible(), N,
      {"total": d["meta"]["total"] if code == 200 else f"http {code}"}, {"total": N})
code, d = post("SearchMedia", {"opts": {"filter": old_any, "order": "DateDesc", "limit": 5, "offset": 0}})
check(rows, legend, "old client JSON (no with_total)", "-", "-",
      {"http": code, "items": len(d["media"]) if code == 200 else None, "total": d.get("total") if code == 200 else None},
      {"http": 200, "items": 5, "total": None})
grid("phase 3: older SearchFilter requests", ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# ---------------------------------------------------------------- paging integrity, structured query
rows, legend = [], []
f = query([], ("tag", "Has", "ct-even"), ("date", "Range", "2020-01..2024-12"))
raw, raw_sql = count_with(sql_media_visible, [raw_tag("v0"), raw_date("Range", "v1", "v2")],
                          v0="ct-even", v1="2020-01", v2="2024-12")
for order in ORDERS:
    def fetch(**kw):
        return post("SearchMedia", {"opts": opts(order=order, filt=f, **kw)})[1]
    code, probe = post("SearchMedia", {"opts": opts(order=order, filt=f, limit=1)})
    if code != 200:
        check(rows, legend, f"tag:ct-even date:2020-01..2024-12 {order}", raw_sql, raw,
              {"http": code}, {"http": 200})
        continue
    full = fetch(limit=None)["media"]
    got, off, reqs = [], 0, 0
    while True:
        r = fetch(limit=200, offset=off)
        reqs += 1
        got += r["media"]
        off += len(r["media"])
        if not r["meta"]["has_more"]:
            break
    check(rows, legend, f"tag:ct-even date:2020-01..2024-12 {order}", raw_sql, raw,
          {"unpaged": len(full), "paged": len(got), "equal": full == got,
           "dups": len(got) - len(set(got)), "requests": reqs},
          {"unpaged": raw, "paged": raw, "equal": True, "dups": 0, "requests": -(-raw // 200)})
grid("phase 3: paging integrity, structured query (pages of 200 vs the unpaged list)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# ---------------------------------------------------------------- phase 4: collection and library queries
# the counttest collections carry the notes and tags of docker/devtest/seed_search.sql
if not one("SELECT count(*) FROM collections WHERE name LIKE 'counttest-%' AND tags ? 'ct-even'"):
    sys.exit("phase 4 needs the search seed: docker exec -i " + PG +
             " psql -U entanglement -d entanglement -v ON_ERROR_STOP=1 < docker/devtest/seed_search.sql")


def list_query(text, *clauses):
    """the `query` of a SearchCollectionsReq / SearchLibrariesReq."""
    return {"text": list(text), "clauses": [{"field": f, "op": o, "value": v} for f, o, v in clauses]}


# raw SQL conditions, again written independently of the resolver (position() on lower() for
# ILIKE) and taking every value as a psql variable.  c is collections, l is libraries
def c_text(*vars_):
    return "(" + " OR ".join(f"position(lower(:'{v}') in lower(c.name)) > 0" for v in vars_) + ")"


def c_name(var):
    return f"position(lower(:'{var}') in lower(c.name)) > 0"


def c_note(var):
    return f"position(lower(:'{var}') in lower(c.note)) > 0"


def c_tag(var):
    return f"c.tags ? :'{var}'"


def l_text(*vars_):
    return "(" + " OR ".join(f"position(lower(:'{v}') in lower(l.path)) > 0" for v in vars_) + ")"


def l_path(var):
    return f"position(lower(:'{var}') in lower(l.path)) > 0"


def count_collections(conds, **vars_):
    text = f"SELECT count(*) FROM collections c WHERE c.gid = '{GID}'" + "".join(f" AND {c}" for c in conds)
    return int(sql(text, **vars_)), text


def count_libraries(conds, **vars_):
    text = f"SELECT count(*) FROM libraries l WHERE l.gid = '{GID}'" + "".join(f" AND {c}" for c in conds)
    return int(sql(text, **vars_)), text


def list_total(ep, query_=None, filter_="", **kw):
    """the meta.total of a count-only search (or the http status as text)."""
    body = {"filter": filter_, "limit": 0, "offset": 0, "with_total": True, **kw}
    if query_ is not None:
        body["query"] = query_
    code, d = post(ep, body)
    return d["meta"]["total"] if code == 200 else f"http {code}"


# (case, endpoint, query text words, clauses, raw conditions on c/l, psql vars)
P4 = [
    ("empty query", "SearchCollections", [], [], [], {}),
    ("text: one word (c01)", "SearchCollections", ["c01"], [], [c_text("w0")], {"w0": "c01"}),
    ("text: two words (OR)", "SearchCollections", ["c01", "c02"], [], [c_text("w0", "w1")],
     {"w0": "c01", "w1": "c02"}),
    ("text: case", "SearchCollections", ["PHRASE"], [], [c_text("w0")], {"w0": "PHRASE"}),
    ('name:"phrase big" (spaces)', "SearchCollections", [], [("name", "Contains", "phrase big")],
     [c_name("v0")], {"v0": "phrase big"}),
    ('name:"bulk big" (no such name)', "SearchCollections", [], [("name", "Contains", "bulk big")],
     [c_name("v0")], {"v0": "bulk big"}),
    ("description:harbour", "SearchCollections", [], [("description", "Contains", "harbour")],
     [c_note("v0")], {"v0": "harbour"}),
    ("description:SUNSET (case)", "SearchCollections", [], [("description", "Contains", "SUNSET")],
     [c_note("v0")], {"v0": "SUNSET"}),
    ("tag:ct-mod7", "SearchCollections", [], [("tag", "Has", "ct-mod7")], [c_tag("v0")], {"v0": "ct-mod7"}),
    ("tag:ct-even", "SearchCollections", [], [("tag", "Has", "ct-even")], [c_tag("v0")], {"v0": "ct-even"}),
    ("tag:ct-nonesuch", "SearchCollections", [], [("tag", "Has", "ct-nonesuch")], [c_tag("v0")],
     {"v0": "ct-nonesuch"}),
    ("tag + description + text", "SearchCollections", ["c02"],
     [("tag", "Has", "ct-even"), ("description", "Contains", "sunset")],
     [c_text("w0"), c_tag("v0"), c_note("v1")], {"w0": "c02", "v0": "ct-even", "v1": "sunset"}),
    ("name + tag", "SearchCollections", [], [("name", "Contains", "counttest-c0"), ("tag", "Has", "ct-mod7")],
     [c_name("v0"), c_tag("v1")], {"v0": "counttest-c0", "v1": "ct-mod7"}),
    ("empty query", "SearchLibraries", [], [], [], {}),
    ("text: one word (count)", "SearchLibraries", ["count"], [], [l_text("w0")], {"w0": "count"}),
    ("text: one word (test, all)", "SearchLibraries", ["test"], [], [l_text("w0")], {"w0": "test"}),
    ("text: two words (OR)", "SearchLibraries", ["pagetest", "bulktest"], [], [l_text("w0", "w1")],
     {"w0": "pagetest", "w1": "bulktest"}),
    ("path:count", "SearchLibraries", [], [("path", "Contains", "count")], [l_path("v0")], {"v0": "count"}),
    ("path:BULK (case)", "SearchLibraries", [], [("path", "Contains", "BULK")], [l_path("v0")], {"v0": "BULK"}),
    ("text + path", "SearchLibraries", ["test"], [("path", "Contains", "bulk")],
     [l_text("w0"), l_path("v0")], {"w0": "test", "v0": "bulk"}),
    ("path:/counttest", "SearchLibraries", [], [("path", "Contains", "/counttest")], [l_path("v0")],
     {"v0": "/counttest"}),
]

rows, legend, raws4 = [], [], {}
for name, ep, text, clauses, conds, vars_ in P4:
    counter = count_collections if ep == "SearchCollections" else count_libraries
    raw, raw_sql = counter(conds, **vars_)
    raws4[(name, ep)] = raw
    q_ = list_query(text, *clauses)
    counted = list_total(ep, q_)
    # a page of 10 with the total, as the collections page asks for it
    code, d = post(ep, {"filter": "", "query": q_, "limit": 10, "offset": 0, "with_total": True})
    key = "collections" if ep == "SearchCollections" else "libraries"
    paged = ({"total": d["meta"]["total"], "items": len(d[key]), "has_more": d["meta"]["has_more"]}
             if code == 200 else {"http": code})
    check(rows, legend, f"{name} [{ep}]", f"{raw_sql} {kv(vars_)}", raw,
          {"count_only": counted, **paged},
          {"count_only": raw, "total": raw, "items": min(10, raw), "has_more": raw > 10})
grid("phase 4: collection and library queries (raw SQL vs count-only vs a page of 10)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# the seed makes these non-empty, so agreement on zero cannot pass for success
rows, legend = [], []
for name, ep in (("text: one word (c01)", "SearchCollections"), ("text: two words (OR)", "SearchCollections"),
                 ('name:"phrase big" (spaces)', "SearchCollections"), ("description:harbour", "SearchCollections"),
                 ("tag:ct-mod7", "SearchCollections"), ("tag + description + text", "SearchCollections"),
                 ("path:count", "SearchLibraries"), ("text: two words (OR)", "SearchLibraries")):
    raw = raws4[(name, ep)]
    check(rows, legend, f"{name} [{ep}] is non-empty", "(raw count above)", raw, {"nonzero": raw > 0}, {"nonzero": True})
for name, ep in (('name:"bulk big" (no such name)', "SearchCollections"), ("tag:ct-nonesuch", "SearchCollections")):
    raw = raws4[(name, ep)]
    check(rows, legend, f"{name} [{ep}] is empty", "(raw count above)", raw, {"zero": raw == 0}, {"zero": True})
# the OR must be wider than either word, and the words are not ANDed
o1 = raws4[("text: one word (c01)", "SearchCollections")]
o2 = raws4[("text: two words (OR)", "SearchCollections")]
check(rows, legend, "OR of c01 and c02 is wider than c01", "(raw counts above)", f"{o1} < {o2}",
      {"wider": o1 < o2}, {"wider": True})
grid("phase 4: the cases above are not vacuous", ("case", "raw SQL", "observed", "expected", "match"), rows)

# the older request shapes: no query means the old literal filter, and a query replaces the filter
rows, legend = [], []
for ep, key, raw_fn, count in (
    ("SearchCollections", "collections", lambda v: f"position(:'{v}' in c.name) > 0", count_collections),
    ("SearchLibraries", "libraries", lambda v: f"position(:'{v}' in l.path) > 0", count_libraries),
):
    words = ["c01", "phrase big", "test", ""] if ep == "SearchCollections" else ["test", "count", "bulktest", ""]
    for w in words:
        raw, raw_sql = count([raw_fn("v0")] if w else [], v0=w)
        code, d = post(ep, {"filter": w, "limit": 0, "offset": 0, "with_total": True})
        code2, d2 = post(ep, {"filter": w, "query": None, "limit": 0, "offset": 0, "with_total": True})
        code3, d3 = post(ep, {"filter": w})
        check(rows, legend, f"{ep} old filter {w!r}", f"{raw_sql} {kv({'v0': w})}", raw,
              {"filter": d["meta"]["total"] if code == 200 else f"http {code}",
               "query_null": d2["meta"]["total"] if code2 == 200 else f"http {code2}",
               "no_with_total_items": len(d3[key]) if code3 == 200 else f"http {code3}"},
              {"filter": raw, "query_null": raw, "no_with_total_items": raw})
# the old filter is one literal substring (spaces kept), not words
check(rows, legend, "old filter keeps the spaces: 'phrase big'", "(counted above)", "-",
      {"total": list_total("SearchCollections", filter_="phrase big")},
      {"total": raws4[('name:"phrase big" (spaces)', "SearchCollections")]})
# a query wins over the filter text
raw = raws4[("text: one word (c01)", "SearchCollections")]
check(rows, legend, "query replaces filter", "(counted above)", raw,
      {"total": list_total("SearchCollections", list_query(["c01"]), filter_="zzz-nothing")}, {"total": raw})
grid("phase 4: older request shapes (raw SQL on the literal vs endpoint)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# rejections: a field of another class, a bad operator or value
rows, legend = [], []
BAD_COLLECTION = [
    ("media-only field date", list_query([], ("date", "Eq", "2020"))),
    ("media-only field type", list_query([], ("type", "Eq", "Image"))),
    ("library-only field path", list_query([], ("path", "Contains", "x"))),
    ("unknown field", list_query([], ("color", "Eq", "red"))),
    ("bad op: tag >", list_query([], ("tag", "Gt", "x"))),
    ("bad op: name =", list_query([], ("name", "Eq", "x"))),
    ("bad op: description has", list_query([], ("description", "Has", "x"))),
    ("tag containing the separator", list_query([], ("tag", "Has", "a|b"))),
    ("empty value", list_query([], ("name", "Contains", ""))),
    ("too many words", list_query(["w"] * 33)),
]
BAD_LIBRARY = [
    ("collection-only field name", list_query([], ("name", "Contains", "x"))),
    ("collection-only field tag", list_query([], ("tag", "Has", "x"))),
    ("media-only field description", list_query([], ("description", "Contains", "x"))),
    ("media-only field date", list_query([], ("date", "Eq", "2020"))),
    ("unknown field", list_query([], ("color", "Eq", "red"))),
    ("bad op: path >", list_query([], ("path", "Gt", "x"))),
    ("bad op: path has", list_query([], ("path", "Has", "x"))),
    ("empty value", list_query([], ("path", "Contains", ""))),
]
for ep, bad in (("SearchCollections", BAD_COLLECTION), ("SearchLibraries", BAD_LIBRARY)):
    for name, q_ in bad:
        code, d = post(ep, {"filter": "", "query": q_, "limit": 10, "offset": 0, "with_total": True})
        check(rows, legend, f"{name} -> {ep}", "-", "-", {"http": code}, {"http": 400})
# the same fields are fine on their own class
for ep, q_ in (("SearchCollections", list_query(["a"], ("tag", "Has", "x"), ("name", "Contains", "y"),
                                                  ("description", "Contains", "z"))),
               ("SearchLibraries", list_query(["a"], ("path", "Contains", "y")))):
    code, d = post(ep, {"filter": "", "query": q_, "limit": 10, "offset": 0, "with_total": True})
    check(rows, legend, f"valid query -> {ep}", "-", "-", {"http": code}, {"http": 200})
grid("phase 4: rejections (raw SQL column is '-', expected values are constants)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)

# hostile values on name:, description:, tag: and path: are matched literally.  the seed gives
# every hostile value a collection whose name, note and tag contain it, so those counts are not
# zero; no library path has one, so the library counts are zero, and for % and _ that differs
# from the total (a LIKE wildcard would match every path)
rows, legend = [], []
for field, op, raw_fn in (("name", "Contains", c_name), ("description", "Contains", c_note), ("tag", "Has", c_tag)):
    for evil in EVIL:
        raw, raw_sql = count_collections([raw_fn("v0")], v0=evil)
        total = list_total("SearchCollections", list_query([], (field, op, evil)))
        check(rows, legend, f"collections {field}:{evil}", f"{raw_sql} {kv({'v0': evil})}", raw,
              {"count_only": total, "nonzero": raw > 0, "not_all": raw < NC},
              {"count_only": raw, "nonzero": True, "not_all": True})
for evil in EVIL:
    raw, raw_sql = count_libraries([l_path("v0")], v0=evil)
    total = list_total("SearchLibraries", list_query([], ("path", "Contains", evil)))
    check(rows, legend, f"libraries path:{evil}", f"{raw_sql} {kv({'v0': evil})}", raw,
          {"count_only": total, "not_all": raw < NL}, {"count_only": raw, "not_all": True})
# as free text, which goes through the same LIKE pattern
for evil in EVIL:
    raw, raw_sql = count_collections([c_text("v0")], v0=evil)
    total = list_total("SearchCollections", list_query([evil]))
    check(rows, legend, f"collections text:{evil}", f"{raw_sql} {kv({'v0': evil})}", raw,
          {"count_only": total, "nonzero": raw > 0}, {"count_only": raw, "nonzero": True})
    raw, raw_sql = count_libraries([l_text("v0")], v0=evil)
    total = list_total("SearchLibraries", list_query([evil]))
    check(rows, legend, f"libraries text:{evil}", f"{raw_sql} {kv({'v0': evil})}", raw,
          {"count_only": total}, {"count_only": raw})
check(rows, legend, "collections and libraries intact afterwards",
      "SELECT count(*) FROM collections / libraries", f"{NC_ALL} / {NL_ALL}",
      {"collections": one("SELECT count(*) FROM collections"), "libraries": one("SELECT count(*) FROM libraries")},
      {"collections": NC_ALL, "libraries": NL_ALL})
grid("phase 4: hostile values (raw SQL on the literal vs endpoint)",
     ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

# paging integrity for a structured collection query: pages of 7 vs the unpaged list
rows, legend = [], []
q_ = list_query(["c0"], ("tag", "Has", "ct-even"))
raw, raw_sql = count_collections([c_text("w0"), c_tag("v0")], w0="c0", v0="ct-even")
code, full = post("SearchCollections", {"filter": "", "query": q_})
got, off, reqs = [], 0, 0
while True:
    code, r = post("SearchCollections", {"filter": "", "query": q_, "limit": 7, "offset": off, "with_total": off == 0})
    reqs += 1
    got += r["collections"]
    off += len(r["collections"])
    if not r["meta"]["has_more"]:
        break
check(rows, legend, "text c0 + tag:ct-even, pages of 7", raw_sql, raw,
      {"unpaged": len(full["collections"]), "paged": len(got), "equal": full["collections"] == got,
       "dups": len(got) - len(set(got)), "requests": reqs},
      {"unpaged": raw, "paged": raw, "equal": True, "dups": 0, "requests": -(-raw // 7)})
grid("phase 4: paging integrity, collection query", ("case", "raw SQL", "endpoint(s)", "expected", "match"), rows)
show_legend(legend)

print(f"\n{'ALL CASES MATCH' if FAILED == 0 else f'{FAILED} CASE(S) FAILED'}")
sys.exit(1 if FAILED else 0)
PY
