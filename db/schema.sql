-- entanglement postgres schema
--
-- there is no migration/schema file checked into this repo anywhere; this file was
-- reverse-engineered by reading every query in common/src/db/postgres.rs (the only
-- Postgres backend implementation) and cross-referencing the Rust struct definitions
-- in api/src/{media,library,collection,comment,search}.rs for field names/types.
--
-- requirements this schema assumes:
--   * PostgreSQL 18+     -- uuidv7() is called directly in INSERT statements
--                           (common/src/db/postgres.rs: add_media/add_comment/add_collection/
--                           add_library) and is only a built-in as of Postgres 18.
--   * the hstore extension -- `tags` columns are read/written as Rust
--                           HashMap<String, Option<String>> (see set_to_hstore/hstore_to_set
--                           in common/src/db/postgres.rs), which tokio-postgres maps to the
--                           Postgres `hstore` type.
--
-- one deliberate inference: the `ts_vec` column on `media` is referenced by every search
-- query (search_media, similar_media, search_media_in_collection, search_media_in_library
-- via SearchOptions::format_postgres) but is never written to by any INSERT/UPDATE in the
-- Rust code. The only way that's consistent is if it's a generated/derived column, so it's
-- kept in sync here via a BEFORE INSERT OR UPDATE trigger over path + note + tag keys
-- (a plain `GENERATED ALWAYS AS ... STORED` column won't work: to_tsvector() is classified
-- STABLE, not IMMUTABLE, in Postgres, and generated columns require an immutable expression).
-- If the original deployment computed it differently (e.g. included media_type), full-text
-- search results may differ slightly from production, but every query that reads ts_vec
-- will still work correctly against this definition.

CREATE EXTENSION IF NOT EXISTS hstore;

-- media.metadata (Rust: MediaMetadata) is a Rust enum stored via #[postgres(name = "media_type")]
-- with no per-variant renaming, so the SQL enum labels must match the Rust variant identifiers
-- exactly (api/src/media.rs: Image, Video, VideoSlice, Audio).
CREATE TYPE media_type AS ENUM ('Image', 'Video', 'VideoSlice', 'Audio');

-- libraries
--
-- api/src/library.rs: Library { path, uid, gid, count }
-- common/src/db/postgres.rs add_library(): ON CONFLICT (path) DO NOTHING => path is unique
CREATE TABLE libraries (
    library_uuid    uuid PRIMARY KEY DEFAULT uuidv7(),
    path            text NOT NULL UNIQUE,
    uid             text NOT NULL,
    gid             text NOT NULL,
    count           bigint NOT NULL DEFAULT 0
);

CREATE INDEX libraries_gid_idx ON libraries (gid);

-- media
--
-- api/src/media.rs: Media { library_uuid, path, size, chash, phash, mtime, hidden, date,
--                            note, tags, metadata }
-- common/src/db/postgres.rs add_media(): ON CONFLICT (library_uuid, path) DO NOTHING
-- similar_media(): bit_count(('x' || phash)::bit & ...) => phash is a hex string in text form
CREATE TABLE media (
    media_uuid      uuid PRIMARY KEY DEFAULT uuidv7(),
    library_uuid    uuid NOT NULL REFERENCES libraries (library_uuid) ON DELETE CASCADE,
    path            text NOT NULL,
    size            bigint NOT NULL,
    chash           text NOT NULL,
    phash           text NOT NULL,
    mtime           bigint NOT NULL,
    hidden          boolean NOT NULL DEFAULT false,
    date            text NOT NULL,
    note            text NOT NULL DEFAULT '',
    tags            hstore NOT NULL DEFAULT ''::hstore,
    media_type      media_type NOT NULL,
    ts_vec          tsvector,
    UNIQUE (library_uuid, path)
);

CREATE INDEX media_library_uuid_idx ON media (library_uuid);
CREATE INDEX media_chash_idx ON media (library_uuid, chash);
CREATE INDEX media_path_idx ON media (path);
CREATE INDEX media_ts_vec_idx ON media USING GIN (ts_vec);

CREATE FUNCTION media_update_ts_vec() RETURNS trigger AS $$
BEGIN
    NEW.ts_vec := to_tsvector(
        'english',
        coalesce(NEW.path, '') || ' ' || coalesce(NEW.note, '') || ' ' ||
        coalesce(array_to_string(akeys(NEW.tags), ' '), '')
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER media_ts_vec_trigger
    BEFORE INSERT OR UPDATE ON media
    FOR EACH ROW EXECUTE FUNCTION media_update_ts_vec();

-- collections
--
-- api/src/collection.rs: Collection { uid, gid, name, note, tags, cover }
-- common/src/db/postgres.rs add_collection(): ON CONFLICT (uid, name) DO NOTHING
-- cover is Option<MediaUuid> => nullable FK to media
CREATE TABLE collections (
    collection_uuid uuid PRIMARY KEY DEFAULT uuidv7(),
    uid             text NOT NULL,
    gid             text NOT NULL,
    name            text NOT NULL,
    note            text NOT NULL DEFAULT '',
    tags            hstore NOT NULL DEFAULT ''::hstore,
    cover           uuid REFERENCES media (media_uuid) ON DELETE SET NULL,
    UNIQUE (uid, name)
);

CREATE INDEX collections_gid_idx ON collections (gid);

-- collection_contents
--
-- common/src/db/postgres.rs add_media_to_collection(): INSERT INTO collection_contents
-- (media_uuid, collection_uuid) ... ON CONFLICT (media_uuid, collection_uuid) DO NOTHING
-- RETURNING id => id is a synthetic bigint identity, not part of the api crate structs
CREATE TABLE collection_contents (
    id              bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    media_uuid      uuid NOT NULL REFERENCES media (media_uuid) ON DELETE CASCADE,
    collection_uuid uuid NOT NULL REFERENCES collections (collection_uuid) ON DELETE CASCADE,
    UNIQUE (media_uuid, collection_uuid)
);

CREATE INDEX collection_contents_collection_uuid_idx ON collection_contents (collection_uuid);
CREATE INDEX collection_contents_media_uuid_idx ON collection_contents (media_uuid);

-- comments
--
-- api/src/comment.rs: Comment { media_uuid, uid, date, text }
-- common/src/db/postgres.rs add_comment(): date is `SystemTime::now().duration_since(UNIX_EPOCH)`
-- cast to i64, i.e. unix seconds, not a native timestamp column
CREATE TABLE comments (
    comment_uuid    uuid PRIMARY KEY DEFAULT uuidv7(),
    media_uuid      uuid NOT NULL REFERENCES media (media_uuid) ON DELETE CASCADE,
    uid             text NOT NULL,
    date            bigint NOT NULL,
    text            text NOT NULL
);

CREATE INDEX comments_media_uuid_idx ON comments (media_uuid);
