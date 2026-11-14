# docker/devtest

Seeds and end-to-end checks that run against the local dev stack (see [`../QUICKSTART.md`](../QUICKSTART.md)).
The principles, unit-test requirements and the full guide are in [`../../.claude/skills/devtest/SKILL.md`](../../.claude/skills/devtest/SKILL.md).

Every check computes its expected values with raw SQL against the dev database and prints a grid of `case | raw SQL | endpoint(s) | expected | match`. Each exits non-zero on any mismatch.

## Seeds
All seeds are idempotent, and every row they create carries the `counttest` identifier.

```sh
docker exec -i entanglement-postgres-1 psql -U entanglement -d entanglement -v ON_ERROR_STOP=1 < docker/devtest/<seed>.sql
```

| Seed | What it adds |
|---|---|
| `seed_counttest.sql` | A 20,000-item library plus 40 collections. **Optional:** it's a lot of searchable records in your dev database, so only load it if you want realistic sizes. The rows have no image files. |
| `seed_search.sql` | Tags, descriptions, hostile values and a ~1000-item hand-testing set (happy tags and English phrases) on the counttest rows. Needs `seed_counttest.sql`. |
| `seed_rbac.sql` | Records owned by a group the `dev` user isn't in, plus a small library dev can see. Standalone. |

## Checks
| Script | Run with | Checks |
|---|---|---|
| `api_check.sh` | `zsh docker/devtest/api_check.sh` | Paging, counts, search fields, prefixes, rejections, hostile input, paging integrity |
| `rbac_check.py` | `python3 -I docker/devtest/rbac_check.py` | Out-of-group records never surface anywhere |
| `ui_check.js` | `PW_MODULE=… node docker/devtest/ui_check.js` | Search bar, chips, hints, `?q=` URLs, back-navigation |
| `ui_modal_check.js` | `PW_MODULE=… node docker/devtest/ui_modal_check.js` | Collection modal layout and tag entry |

The Playwright scripts need `playwright` installed: `npm i playwright && npx playwright install chromium` in any folder, then point `PW_MODULE` at its `node_modules`. Some scripts arrive with later branches of the search/paging stack.

## Cleanup
Removes everything counttest:

```sql
DELETE FROM comments WHERE text LIKE '[counttest]%';
DELETE FROM collection_contents WHERE collection_uuid IN (SELECT collection_uuid FROM collections WHERE name LIKE 'counttest-%');
DELETE FROM collections WHERE name LIKE 'counttest-%';
DELETE FROM media WHERE library_uuid IN (SELECT library_uuid FROM libraries WHERE path LIKE '/counttest%');
DELETE FROM libraries WHERE path LIKE '/counttest%';
```
