-- the counttest base data: a 20000-item library and 40 collections, for paging, count, search
-- and performance checks at a realistic size.  run this first; seed_search.sql then layers
-- searchable tags and descriptions on top, and seed_rbac.sql adds the out-of-group records.
--
--   docker exec -i entanglement-postgres-1 psql -U entanglement -d entanglement \
--     -v ON_ERROR_STOP=1 < docker/devtest/seed_counttest.sql
--
-- the rows are database records only: there are no image files behind them, so their
-- thumbnails 404 and the grid shows "No preview" tiles.  idempotent: it removes its own
-- previous rows first (uuids change on every run).
--
-- layout (item n = 0..19999)
--   library     /counttest, uid dev, gid devgroup
--   media       /counttest/ct_NNNNN.jpg, note '[counttest] item n', tag ct-even / ct-odd
--               date 2020-01-01 + (n * 419 % 2500) days, so dates spread over 2020..2026
--               hidden when n % 10 = 0 (2000 items)
--   collections counttest-c000 .. counttest-c039, uid dev, gid devgroup; counttest-cNN holds
--               the items with n % 50 = NN (400 each).  c000 holds only hidden items (n % 50 = 0
--               implies n % 10 = 0), so it counts 0 for dev -- a useful edge case
--
-- cleanup: see docker/devtest/README.md (everything counttest, in one block)
BEGIN;

DELETE FROM collection_contents WHERE collection_uuid IN
    (SELECT collection_uuid FROM collections WHERE name ~ '^counttest-c[0-9]+$');
DELETE FROM collections WHERE name ~ '^counttest-c[0-9]+$';
DELETE FROM collection_contents WHERE media_uuid IN
    (SELECT media_uuid FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path = '/counttest'));
DELETE FROM comments WHERE media_uuid IN
    (SELECT media_uuid FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path = '/counttest'));
DELETE FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path = '/counttest');
DELETE FROM libraries WHERE path = '/counttest';

INSERT INTO libraries (library_uuid, path, uid, gid, count)
VALUES (gen_random_uuid(), '/counttest', 'dev', 'devgroup', 20000);

INSERT INTO media (media_uuid, library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type)
SELECT
    gen_random_uuid(),
    (SELECT library_uuid FROM libraries WHERE path = '/counttest'),
    format('/counttest/ct_%s.jpg', lpad(n::text, 5, '0')),
    1000 + n,
    md5(n::text) || md5((n + 1)::text) || md5((n + 2)::text) || md5((n + 3)::text),
    md5((n + 4)::text) || md5((n + 5)::text),
    1794534900 + n,
    n % 10 = 0,
    to_char(date '2020-01-01' + (n * 419 % 2500), 'YYYY-MM-DD'),
    format('[counttest] item %s', n),
    hstore(CASE WHEN n % 2 = 0 THEN 'ct-even' ELSE 'ct-odd' END, NULL),
    'Image'
FROM generate_series(0, 19999) AS n;

INSERT INTO collections (collection_uuid, uid, gid, name, note, tags, cover)
SELECT gen_random_uuid(), 'dev', 'devgroup', format('counttest-c%s', lpad(k::text, 3, '0')),
       format('[counttest] collection %s', k), ''::hstore, NULL
FROM generate_series(0, 39) AS k;

INSERT INTO collection_contents (media_uuid, collection_uuid)
SELECT m.media_uuid, c.collection_uuid
FROM media m
JOIN collections c
  ON c.name = format('counttest-c%s', lpad((substring(m.path from 'ct_([0-9]+)')::int % 50)::text, 3, '0'))
WHERE m.library_uuid = (SELECT library_uuid FROM libraries WHERE path = '/counttest');

COMMIT;

ANALYZE media;
ANALYZE collections;
ANALYZE collection_contents;
