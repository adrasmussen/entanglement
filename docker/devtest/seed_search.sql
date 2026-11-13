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
-- items 1001..1006 carry a tag equal to, and a note containing, a LIKE/SQL-hostile value
-- (the safety grid searches for each literally; none of them is hidden)

WITH special(n, v) AS (
  VALUES
    (1001, $q$'$q$),
    (1002, '%'),
    (1003, '_'),
    (1004, E'\\'),
    (1005, $q$'; DROP TABLE media; --$q$),
    (1006, '$1')
),
items AS (
  SELECT m.media_uuid, i.n, special.v
  FROM media m
  CROSS JOIN LATERAL (SELECT substring(m.note from 'item ([0-9]+)')::int AS n) i
  LEFT JOIN special ON special.n = i.n
  WHERE m.note LIKE '[counttest]%'
)
UPDATE media m SET
  note = '[counttest] item ' || items.n
         || ' ' || (ARRAY['sunset', 'beach', 'mountain', 'forest'])[items.n % 4 + 1]
         || CASE WHEN items.n % 1000 = 3 THEN ' lighthouse' ELSE '' END
         || COALESCE(' x' || items.v || 'x', ''),
  tags = hstore(CASE WHEN items.n % 2 = 0 THEN 'ct-even' ELSE 'ct-odd' END, NULL)
         || CASE WHEN items.n % 7 = 0 THEN hstore('ct-mod7', NULL) ELSE ''::hstore END
         || CASE WHEN items.n % 500 = 3 THEN hstore('ct-rare', NULL) ELSE ''::hstore END
         || CASE WHEN items.v IS NOT NULL THEN hstore(items.v, NULL) ELSE ''::hstore END
FROM items
WHERE m.media_uuid = items.media_uuid;

ANALYZE media;
