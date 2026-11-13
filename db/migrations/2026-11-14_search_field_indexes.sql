--
-- search field indexes for the structured media search (postgres)
--
-- brings an existing database in line with db/schema.sql; fresh installs already have
-- this.  idempotent, and CONCURRENTLY avoids blocking writes while the index builds --
-- which also means this file must NOT run inside a transaction, so apply it with plain
-- psql (no --single-transaction), e.g. for the docker-compose stack:
--
--   docker compose exec -T postgres psql -U entanglement -d entanglement \
--     -v ON_ERROR_STOP=1 < db/migrations/2026-11-14_search_field_indexes.sql
--
-- if a CONCURRENTLY build is interrupted it leaves an INVALID index behind that
-- IF NOT EXISTS will skip; drop it (DROP INDEX CONCURRENTLY <name>) and rerun.
--
-- measured with EXPLAIN (ANALYZE) on 21.7k media (20k seeded with tags and description
-- words), through the same visibility filter as SearchMedia:
--
--   date:2022-04            page 10.1 ms -> 1.3 ms,  count 42.3 ms -> 1.0 ms
--   date:2021-01..2021-06   count 45.5 ms -> 3.2 ms
--
-- (a rare tag combined with a date range goes from 0.4 ms to 1.2 ms, as the planner then
-- ANDs three bitmaps; still far below anything noticeable)
--
-- the other new fields needed nothing: `tag:` already uses the GIN index on tags (bitmap
-- scans, under 10 ms even for half the table), and a type: or date:> / date:>= page walks
-- idx_media_visible_date and stops at the LIMIT.  description: is ILIKE '%x%', which no
-- btree can serve, so it stays a filtered scan (~65 ms on this table); pg_trgm is the
-- phase 5 answer.

-- date:YYYY[-MM[-DD]] prefixes (LIKE 'x%') and the upper bound of a..b.  the database
-- collation is en_US.utf8, where a plain btree can't serve LIKE, hence text_pattern_ops
CREATE INDEX CONCURRENTLY IF NOT EXISTS "idx_media_date_prefix"
    ON "media" ("date" text_pattern_ops);

ANALYZE "media";

-- mariadb equivalent (its default collations let a plain index serve LIKE 'x%'):
--
--   CREATE INDEX idx_media_date_prefix ON media (date);
