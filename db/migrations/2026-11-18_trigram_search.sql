--
-- trigram indexes: matching inside words (postgres)
--
-- plain search words match tsvector prefixes ('check' finds checkpoint) but not the inside
-- of a word ('eckp'), and the Contains fields are ILIKE '%x%', which no btree can serve.
-- pg_trgm GIN indexes serve both: every term of three or more characters can match
-- anywhere in a word, through an index.
--
-- brings an existing database in line with db/schema.sql; fresh installs already have
-- this.  idempotent, and CONCURRENTLY avoids blocking writes while the indexes build --
-- which also means this file must NOT run inside a transaction, so apply it with plain
-- psql (no --single-transaction), e.g. for the docker-compose stack:
--
--   docker compose exec -T postgres psql -U entanglement -d entanglement \
--     -v ON_ERROR_STOP=1 < db/migrations/2026-11-18_trigram_search.sql
--
-- pg_trgm is a trusted extension, so the database owner can create it.  if a
-- CONCURRENTLY build is interrupted it leaves an INVALID index behind that IF NOT EXISTS
-- will skip; drop it (DROP INDEX CONCURRENTLY <name>) and rerun.
--
-- the EXPLAIN (ANALYZE, BUFFERS) before/after grid is in the PR that added this file.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- the text plain words search, lowercased: the same fields as media.ts_vec (date,
-- description, tags).  IMMUTABLE so it can be indexed; queries must call it with the same
-- arguments, media_search_text(media.date, media.note, media.tags), for the index to apply
-- (akeys is schema-qualified because index builds run with a restricted search_path)
CREATE OR REPLACE FUNCTION media_search_text(d text, n text, t hstore) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT lower(d || ' ' || n || ' ' || array_to_string(public.akeys(t), ' '));
$$;

-- plain words, inside words
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_search_trgm"
    ON "media" USING GIN (media_search_text("date", "note", "tags") gin_trgm_ops);

-- description:
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_note_trgm"
    ON "media" USING GIN ("note" gin_trgm_ops);

-- name: (the file name, not the folder path)
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_name_trgm"
    ON "media" USING GIN ((substring("path" from '[^/]*$')) gin_trgm_ops);

-- collection words and name:/description:, library words and path:
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_collection_name_trgm"
    ON "collections" USING GIN ("name" gin_trgm_ops);
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_collection_note_trgm"
    ON "collections" USING GIN ("note" gin_trgm_ops);
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_library_path_trgm"
    ON "libraries" USING GIN ("path" gin_trgm_ops);

ANALYZE "media";
ANALYZE "collections";
ANALYZE "libraries";

-- mariadb has no trigram indexes; its LIKE '%x%' and RLIKE stay table scans there, which
-- is fine at personal-library sizes.  (an n-gram FULLTEXT parser exists only in MySQL.)
