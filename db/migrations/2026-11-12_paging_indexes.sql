-- paging indexes for infinite-scroll search (postgres)
--
-- brings an existing database in line with db/schema.sql; fresh installs already
-- have these.  idempotent, and CONCURRENTLY avoids blocking writes while the indexes
-- build -- which also means this file must NOT run inside a transaction, so apply it
-- with plain psql (no --single-transaction), e.g. for the docker-compose stack:
--
--   docker compose exec -T postgres psql -U entanglement -d entanglement \
--     -v ON_ERROR_STOP=1 < db/migrations/2026-11-12_paging_indexes.sql
--
-- if a CONCURRENTLY build is interrupted it leaves an INVALID index behind that
-- IF NOT EXISTS will skip; drop it (DROP INDEX CONCURRENTLY <name>) and rerun.

CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_visible_date"
    ON "media" ("date" DESC, "media_uuid" DESC) WHERE "hidden" = FALSE;

CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_visible_path"
    ON "media" ("path", "media_uuid") WHERE "hidden" = FALSE;

CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_library_date"
    ON "media" ("library_uuid", "date" DESC, "media_uuid" DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_collection_name"
    ON "collections" ("name", "collection_uuid");

-- superseded by idx_media_library_date (same leading column)
DROP INDEX CONCURRENTLY IF EXISTS "idx_media_library";

ANALYZE "media";
ANALYZE "collections";

-- mariadb equivalent (no schema file is kept in this repo for mariadb; partial
-- indexes are unsupported there, so hidden leads the visible-media indexes):
--
--   CREATE INDEX idx_media_visible_date ON media (hidden, date, media_uuid);
--   CREATE INDEX idx_media_visible_path ON media (hidden, path, media_uuid);
--   CREATE INDEX idx_media_library_date ON media (library_uuid, date, media_uuid);
--   CREATE INDEX idx_collection_name    ON collections (name, collection_uuid);
