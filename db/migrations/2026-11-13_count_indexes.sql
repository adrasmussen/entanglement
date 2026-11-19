-- count indexes for the GetCounts endpoint (postgres)
--
-- brings an existing database in line with db/schema.sql; fresh installs already
-- have these.  idempotent, and CONCURRENTLY avoids blocking writes while the indexes
-- build -- which also means this file must NOT run inside a transaction, so apply it
-- with plain psql (no --single-transaction), e.g. for the docker-compose stack:
--
--   docker compose exec -T postgres psql -U entanglement -d entanglement \
--     -v ON_ERROR_STOP=1 < db/migrations/2026-11-13_count_indexes.sql
--
-- if a CONCURRENTLY build is interrupted it leaves an INVALID index behind that
-- IF NOT EXISTS will skip; drop it (DROP INDEX CONCURRENTLY <name>) and rerun.
--
-- measured with EXPLAIN (ANALYZE, BUFFERS) on 21.7k media (20k of them in one library,
-- 10% hidden): the library counts (MediaInLibrary, any hidden filter) and the collection
-- count (MediaInCollection) seq-scanned media; with these they are index-only scans /
-- primary-key-like lookups and the collection count drops from ~6.5 ms to ~1.9 ms

-- MediaInLibrary counts: library_uuid plus the optional hidden filter, index-only
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_library_hidden"
    ON "media" ("library_uuid", "hidden");

-- MediaInCollection counts: joins collection_contents to visible media by media_uuid,
-- and this lets the join check hidden = FALSE without reading the media row
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_visible_uuid"
    ON "media" ("media_uuid") WHERE "hidden" = FALSE;

ANALYZE "media";

-- mariadb equivalent (partial indexes are unsupported there, so hidden leads or trails
-- in a plain composite index):
--
--   CREATE INDEX idx_media_library_hidden ON media (library_uuid, hidden);
--   CREATE INDEX idx_media_uuid_hidden    ON media (hidden, media_uuid);
