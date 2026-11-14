-- RBAC seed: records owned by a group the dev user is NOT in (counttest-othergroup).
-- Everything carries the counttest identifier: library path '/counttest-rbac',
-- media notes '[counttest] rbac ...', collections 'counttest-rbac-*', comment text
-- '[counttest] rbac ...'.  Idempotent: removes its own previous rows first.
--
-- layout
--   library  /counttest-rbac (gid counttest-othergroup), 300 media:
--     every one tagged ct-rbac-secret plus common tags (ct-mod7 / ct-even), notes with
--     the common words 'lighthouse beach', dates inside the counttest range, 30 hidden
--   collection counttest-rbac-other (gid counttest-othergroup): 50 foreign media + 50
--     dev-visible counttest media (a foreign collection must not change what dev sees)
--   collection counttest-rbac-bridge (gid devgroup): 10 foreign (visible) media; by
--     design these become visible to dev through the collection -- the positive control
--   comment by uid 'counttest-rbac' on a foreign, non-bridged media item
--   phash of foreign media item #1 = phash of a dev-visible counttest item (SimilarMedia)
BEGIN;

DELETE FROM comments WHERE text LIKE '[counttest] rbac%';
DELETE FROM collection_contents WHERE collection_uuid IN
    (SELECT collection_uuid FROM collections WHERE name LIKE 'counttest-rbac-%');
DELETE FROM collections WHERE name LIKE 'counttest-rbac-%';
DELETE FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac');
DELETE FROM libraries WHERE path = '/counttest-rbac';

INSERT INTO libraries (library_uuid, path, uid, gid, count)
VALUES (gen_random_uuid(), '/counttest-rbac', 'counttest-rbac', 'counttest-othergroup', 300);

INSERT INTO media (media_uuid, library_uuid, path, size, chash, phash, mtime, hidden, date, note, tags, media_type)
SELECT
    gen_random_uuid(),
    (SELECT library_uuid FROM libraries WHERE path = '/counttest-rbac'),
    format('/counttest-rbac/rbac_%s.jpg', lpad(n::text, 5, '0')),
    1000 + n,
    rpad(md5('rbac' || n), 128, '0'),
    CASE WHEN n = 1
         THEN (SELECT phash FROM media WHERE path = '/counttest/ct_00007.jpg')
         ELSE rpad(md5('rbac-p' || n), 64, '0') END,
    1794534907,
    n % 10 = 0,
    to_char(date '2020-01-01' + (n * 7 % 2500), 'YYYY-MM-DD'),
    -- the same happy tags and phrases as seed_search.sql's hand-testing set, on every foreign
    -- item, so searching for them must still surface only dev's own and the bridged media
    format('[counttest] rbac item %s lighthouse beach / %s', n,
           (ARRAY['a quiet morning by the lake', 'the old bridge at dusk', 'children playing in the park',
                  'fresh bread from the corner bakery', 'a long walk through autumn leaves',
                  'rain on the kitchen window', 'friends laughing around the campfire',
                  'the city lights after midnight'])[n % 8 + 1]),
    (CASE WHEN n % 2 = 0 THEN hstore(ARRAY['ct-rbac-secret', 'ct-even', 'ct-mod7'], ARRAY[NULL, NULL, NULL])
          ELSE hstore(ARRAY['ct-rbac-secret', 'ct-odd', 'ct-mod7'], ARRAY[NULL, NULL, NULL]) END)
      || hstore((ARRAY['sunshine', 'joyful', 'delight', 'cheer'])[n % 4 + 1], NULL),
    'Image'
FROM generate_series(1, 300) AS n;

INSERT INTO collections (collection_uuid, uid, gid, name, note, tags, cover)
VALUES
    (gen_random_uuid(), 'counttest-rbac', 'counttest-othergroup', 'counttest-rbac-other',
     '[counttest] rbac foreign collection lighthouse', hstore(ARRAY['ct-rbac-secret'], ARRAY[NULL]), NULL),
    (gen_random_uuid(), 'dev', 'devgroup', 'counttest-rbac-bridge',
     '[counttest] rbac bridge collection', hstore(ARRAY['ct-rbac-bridge'], ARRAY[NULL]), NULL);

-- foreign collection: 50 foreign media (items 1..50, none hidden except 10,20,..) and 50
-- dev-visible counttest media
INSERT INTO collection_contents (media_uuid, collection_uuid)
SELECT m.media_uuid, (SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-other')
FROM media m WHERE m.path IN (SELECT format('/counttest-rbac/rbac_%s.jpg', lpad(n::text, 5, '0')) FROM generate_series(1, 50) n);
INSERT INTO collection_contents (media_uuid, collection_uuid)
SELECT m.media_uuid, (SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-other')
FROM media m WHERE m.path IN (SELECT format('/counttest/ct_%s.jpg', lpad(n::text, 5, '0')) FROM generate_series(1, 99, 2) n);

-- bridge: foreign items 201..211 except 210 (hidden) -> 10 visible bridged media
INSERT INTO collection_contents (media_uuid, collection_uuid)
SELECT m.media_uuid, (SELECT collection_uuid FROM collections WHERE name = 'counttest-rbac-bridge')
FROM media m WHERE m.path IN (SELECT format('/counttest-rbac/rbac_%s.jpg', lpad(n::text, 5, '0')) FROM generate_series(201, 211) n WHERE n <> 210);

INSERT INTO comments (comment_uuid, media_uuid, date, uid, text)
SELECT gen_random_uuid(), media_uuid, 1794534907, 'counttest-rbac', '[counttest] rbac secret comment'
FROM media WHERE path = '/counttest-rbac/rbac_00003.jpg';

COMMIT;
