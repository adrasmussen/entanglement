-- searchable test data for api_check.sh's phase 3 grids, in the dev postgres.  touches
-- ONLY the counttest media (note LIKE '[counttest]%'); the note keeps that prefix, so the
-- counttest cleanup (see the header of api_check.sh) still finds every row.  deterministic
-- and idempotent: each row's note and tags are rebuilt from its item number.
--
--   docker exec -i entanglement-postgres-1 psql -U entanglement -d entanglement \
--     -v ON_ERROR_STOP=1 < docker/devtest/seed_search.sql
--
-- tags:        ct-even / ct-odd on every item, ct-mod7 on every 7th, ct-rare on n % 500 = 3
-- description: one of sunset/beach/mountain/forest by n % 4, and "lighthouse" on n % 1000 = 3
-- items 2001..2003 carry the words checkpoint / checko / checkpt, for prefix matching of plain words
--
-- hand-testing set (about 1000 items, picked by hashtext so it looks random but every run gives the
-- same rows): each picked item independently gets
--   a happy tag      sunshine | joyful | delight | cheer | (none)        one in five each
--   a phrase         one of the eight sentences below, or (none)         8 in 10 get one
--   a second phrase  on about a quarter of the items that have a first one
-- so all combinations occur: tag only, phrase only, both, two phrases, and neither.  phrases are
-- appended to the note after " / ", and avoid every word the scripted checks search for
-- items 1001..1006 carry a tag equal to, and a note containing, a LIKE/SQL-hostile value
-- (the safety grid searches for each literally; none of them is hidden)

WITH phrases(i, p) AS (
  VALUES
    (0, 'a quiet morning by the lake'),
    (1, 'the old bridge at dusk'),
    (2, 'children playing in the park'),
    (3, 'fresh bread from the corner bakery'),
    (4, 'a long walk through autumn leaves'),
    (5, 'rain on the kitchen window'),
    (6, 'friends laughing around the campfire'),
    (7, 'the city lights after midnight')
),
special(n, v) AS (
  VALUES
    (1001, $q$'$q$),
    (1002, '%'),
    (1003, '_'),
    (1004, E'\\'),
    (1005, $q$'; DROP TABLE media; --$q$),
    (1006, '$1')
),
items AS (
  SELECT m.media_uuid, i.n, special.v,
         -- the hand-testing set: about 1 in 20 of the 20000 counttest items
         abs(hashtext('ct-pick-' || i.n)) % 20 = 0 AS picked,
         abs(hashtext('ct-tag-' || i.n)) % 5 AS tag_pick,       -- 0..3 a tag, 4 none
         abs(hashtext('ct-desc-' || i.n)) % 10 AS desc_pick,    -- 0..7 a phrase, 8..9 none
         abs(hashtext('ct-desc2-' || i.n)) % 8 AS desc2_pick,   -- 0..7, used when two_phrases
         abs(hashtext('ct-two-' || i.n)) % 4 = 0 AS two_phrases
  FROM media m
  CROSS JOIN LATERAL (SELECT substring(m.note from 'item ([0-9]+)')::int AS n) i
  LEFT JOIN special ON special.n = i.n
  WHERE m.note LIKE '[counttest]%'
)
UPDATE media m SET
  note = '[counttest] item ' || items.n
         || ' ' || (ARRAY['sunset', 'beach', 'mountain', 'forest'])[items.n % 4 + 1]
         || CASE WHEN items.n % 1000 = 3 THEN ' lighthouse' ELSE '' END
         || CASE items.n WHEN 2001 THEN ' checkpoint' WHEN 2002 THEN ' checko' WHEN 2003 THEN ' checkpt' ELSE '' END
         || COALESCE(' x' || items.v || 'x', '')
         || CASE WHEN items.picked AND items.desc_pick < 8
                 THEN ' / ' || (SELECT p FROM phrases WHERE i = items.desc_pick)
                      || CASE WHEN items.two_phrases AND items.desc2_pick <> items.desc_pick
                              THEN ' / ' || (SELECT p FROM phrases WHERE i = items.desc2_pick)
                              ELSE '' END
                 ELSE '' END,
  tags = hstore(CASE WHEN items.n % 2 = 0 THEN 'ct-even' ELSE 'ct-odd' END, NULL)
         || CASE WHEN items.n % 7 = 0 THEN hstore('ct-mod7', NULL) ELSE ''::hstore END
         || CASE WHEN items.n % 500 = 3 THEN hstore('ct-rare', NULL) ELSE ''::hstore END
         || CASE WHEN items.v IS NOT NULL THEN hstore(items.v, NULL) ELSE ''::hstore END
         || CASE WHEN items.picked AND items.tag_pick < 4
                 THEN hstore((ARRAY['sunshine', 'joyful', 'delight', 'cheer'])[items.tag_pick + 1], NULL)
                 ELSE ''::hstore END
FROM items
WHERE m.media_uuid = items.media_uuid;

ANALYZE media;

-- phase 4: collections (api_check.sh's collection and library grids).  every row has a name
-- starting with counttest-, so the counttest cleanup deletes them all.  the libraries need no
-- seed: the four existing paths (testlib, pagetest, bulktest, /counttest) are enough
--
-- counttest-cNN: ct-even / ct-odd on every one, ct-mod7 on every 7th; the description is
--   sunset/beach/mountain/forest by n % 4, plus "harbour" on n % 10 = 3
-- counttest-phrase big wave: a name with spaces, for name:"phrase big"
-- counttest-evil <v>: a collection per LIKE/SQL-hostile value, whose name, note and tag all
--   contain it (the safety grid searches for each literally)

UPDATE collections c SET
  note = '[counttest] collection ' || s.n
         || ' ' || (ARRAY['sunset', 'beach', 'mountain', 'forest'])[s.n % 4 + 1]
         || CASE WHEN s.n % 10 = 3 THEN ' harbour' ELSE '' END,
  tags = hstore(CASE WHEN s.n % 2 = 0 THEN 'ct-even' ELSE 'ct-odd' END, NULL)
         || CASE WHEN s.n % 7 = 0 THEN hstore('ct-mod7', NULL) ELSE ''::hstore END
FROM (
  SELECT collection_uuid, substring(name from 'counttest-c([0-9]+)')::int AS n
  FROM collections
  WHERE name ~ '^counttest-c[0-9]+$'
) s
WHERE c.collection_uuid = s.collection_uuid;

INSERT INTO collections (collection_uuid, uid, gid, name, note, tags, cover)
SELECT gen_random_uuid(), 'dev', 'devgroup', 'counttest-' || x.suffix,
       '[counttest] ' || x.note, hstore(x.tag, NULL), NULL
FROM (
  VALUES
    ('phrase big wave', 'a name with spaces', 'ct-phrase'),
    ($q$evil '$q$, $q$evil x'x$q$, $q$'$q$),
    ('evil %', 'evil x%x', '%'),
    ('evil _', 'evil x_x', '_'),
    ($q$evil \$q$, $q$evil x\x$q$, $q$\$q$),
    ($q$evil '; DROP TABLE media; --$q$, $q$evil x'; DROP TABLE media; --x$q$,
     $q$'; DROP TABLE media; --$q$),
    ('evil $1', 'evil x$1x', '$1')
) AS x(suffix, note, tag)
ON CONFLICT (uid, name) DO UPDATE SET note = EXCLUDED.note, tags = EXCLUDED.tags;

ANALYZE collections;
