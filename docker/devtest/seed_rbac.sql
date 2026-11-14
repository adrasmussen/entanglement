-- RBAC seed for docker/devtest/rbac_check.py: records owned by a group the dev user is NOT in
-- (counttest-othergroup), next to a small library dev can see.  Self-contained: it needs no
-- other seed data, only the dev stack's schema and the dev user in devgroup (users.toml).
--
--   docker exec -i entanglement-postgres-1 psql -U entanglement -d entanglement \
--     -v ON_ERROR_STOP=1 < docker/devtest/seed_rbac.sql
--
-- Every row carries the counttest identifier, and the seed is idempotent (it removes its own
-- previous rows first):
--   libraries   /counttest-rbac (foreign), /counttest-rbac-dev (devgroup)
--   media       notes start with '[counttest] rbac'
--   collections counttest-rbac-other (foreign), counttest-rbac-bridge (devgroup)
--   comments    text '[counttest] rbac secret comment'
--
-- layout
--   /counttest-rbac-dev: 20 media dev can see (dev items 1..20)
--   /counttest-rbac:    300 foreign media, every one tagged ct-rbac-secret, notes containing
--                       'lighthouse beach', 30 hidden
--   counttest-rbac-other (foreign): foreign items 1..50 plus the odd dev items -- a foreign
--                       collection holding media dev can see must not be revealed to dev
--   counttest-rbac-bridge (devgroup): foreign items 201..211 except the hidden 210; by design
--                       these 10 become visible to dev through the collection (positive control)
--   comment by uid counttest-rbac on foreign item 3
--   foreign item 1 has the same phash as dev item 7 (SimilarMedia)
--
-- cleanup
--   DELETE FROM comments WHERE text LIKE '[counttest] rbac%';
--   DELETE FROM collection_contents WHERE collection_uuid IN
--       (SELECT collection_uuid FROM collections WHERE name LIKE 'counttest-rbac-%');
--   DELETE FROM collections WHERE name LIKE 'counttest-rbac-%';
--   DELETE FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path LIKE '/counttest-rbac%');
--   DELETE FROM libraries WHERE path LIKE '/counttest-rbac%';
BEGIN;

DELETE FROM comments WHERE text LIKE '[counttest] rbac%';
DELETE FROM collection_contents WHERE collection_uuid IN
    (SELECT collection_uuid FROM collections WHERE name LIKE 'counttest-rbac-%');
DELETE FROM collections WHERE name LIKE 'counttest-rbac-%';
DELETE FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path LIKE '/counttest-rbac%');
DELETE FROM libraries WHERE path LIKE '/counttest-rbac%';

INSERT INTO libraries (library_uuid, path, uid, gid, count) VALUES
    (gen_random_uuid(), '/counttest-rbac', 'counttest-rbac', 'counttest-othergroup', 300),
    (gen_random_uuid(), '/counttest-rbac-dev', 'dev', 'devgroup', 20);

-- dev-visible media
INSERT INTO media (media_uuid, library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type)
SELECT
    gen_random_uuid(),
    (SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac-dev'),
    format('/counttest-rbac-dev/dev_%s.jpg', lpad(n::text, 5, '0')),
    2000 + n,
    rpad(md5('rbac-dev' || n), 128, '0'),
    rpad(md5('rbac-dev-p' || n), 64, '0'),
    1794534907,
    FALSE,
    to_char(date '2021-01-01' + n * 11, 'YYYY-MM-DD'),
    format('[counttest] rbac dev item %s harbour', n),
    hstore('ct-rbac-dev', NULL),
    'Image'
FROM generate_series(1, 20) AS n;

-- foreign media
INSERT INTO media (media_uuid, library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type)
SELECT
    gen_random_uuid(),
    (SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac'),
    format('/counttest-rbac/rbac_%s.jpg', lpad(n::text, 5, '0')),
    1000 + n,
    rpad(md5('rbac' || n), 128, '0'),
    CASE WHEN n = 1
         THEN (SELECT phash FROM media WHERE path = '/counttest-rbac-dev/dev_00007.jpg')
         ELSE rpad(md5('rbac-p' || n), 64, '0') END,
    1794534907,
    n % 10 = 0,
    to_char(date '2020-01-01' + (n * 7 % 2500), 'YYYY-MM-DD'),
    -- the hand-testing tags and phrases (see seed_search.sql on later branches) on every
    -- foreign item, so searching for them must still surface only dev's own and bridged media
    format('[counttest] rbac item %s lighthouse beach / %s', n,
           (ARRAY['a quiet morning by the lake', 'the old bridge at dusk', 'children playing in the park',
                  'fresh bread from the corner bakery', 'a long walk through autumn leaves',
                  'rain on the kitchen window', 'friends laughing around the campfire',
                  'the city lights after midnight'])[n % 8 + 1]),
    hstore(ARRAY['ct-rbac-secret', 'ct-mod7', (ARRAY['sunshine', 'joyful', 'delight', 'cheer'])[n % 4 + 1]],
           ARRAY[NULL, NULL, NULL]),
    'Image'
FROM generate_series(1, 300) AS n;

INSERT INTO collections (collection_uuid, uid, gid, name, note, tags, cover)
VALUES
    (gen_random_uuid(), 'counttest-rbac', 'counttest-othergroup', 'counttest-rbac-other',
     '[counttest] rbac foreign collection lighthouse', hstore(ARRAY['ct-rbac-secret'], ARRAY[NULL]), NULL),
    (gen_random_uuid(), 'dev', 'devgroup', 'counttest-rbac-bridge',
     '[counttest] rbac bridge collection', hstore(ARRAY['ct-rbac-bridge'], ARRAY[NULL]), NULL);

INSERT INTO collection_contents (media_uuid, collection_uuid)
SELECT m.media_uuid, (SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-other')
FROM media m
WHERE m.path IN (SELECT format('/counttest-rbac/rbac_%s.jpg', lpad(n::text, 5, '0')) FROM generate_series(1, 50) n)
   OR m.path IN (SELECT format('/counttest-rbac-dev/dev_%s.jpg', lpad(n::text, 5, '0')) FROM generate_series(1, 19, 2) n);

INSERT INTO collection_contents (media_uuid, collection_uuid)
SELECT m.media_uuid, (SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-bridge')
FROM media m
WHERE m.path IN (SELECT format('/counttest-rbac/rbac_%s.jpg', lpad(n::text, 5, '0')) FROM generate_series(201, 211) n WHERE n <> 210);

INSERT INTO comments (comment_uuid, media_uuid, date, uid, text)
SELECT gen_random_uuid(), media_uuid, 1794534907, 'counttest-rbac', '[counttest] rbac secret comment'
FROM media WHERE path = '/counttest-rbac/rbac_00003.jpg';

COMMIT;
