-- entanglement postgres schema
--
-- this is a sql dump of the live production schema, provided by adrasmussen in review
-- of this PR (https://github.com/adrasmussen/entanglement/pull/2#discussion_r4068378934),
-- replacing the earlier reverse-engineered version of this file.
--
-- requirements:
--   * PostgreSQL 18+ -- uuidv7() is called directly in app INSERT statements
--                       (common/src/db/postgres.rs: add_media/add_comment/add_collection/
--                       add_library) and is only a built-in as of Postgres 18. Primary keys
--                       here intentionally have no DEFAULT: the app always supplies the value.
--   * the hstore extension -- `tags` columns are read/written as Rust
--                       HashMap<String, Option<String>> (see set_to_hstore/hstore_to_set
--                       in common/src/db/postgres.rs), which tokio-postgres maps to the
--                       Postgres `hstore` type.
--
-- the block below is reference only (commented out): manual db/user provisioning for
-- environments that don't get it from POSTGRES_DB/POSTGRES_USER/POSTGRES_PASSWORD the way
-- docker-compose.yml does here.
/*
CREATE DATABASE "entanglement-dev";

CREATE USER IF NOT EXISTS "entg-dev" WITH PASSWORD 'hunter2';

GRANT ALL PRIVILEGES ON DATABASE "entanglement-dev" to "entg-dev";

USE "entanglement";

GRANT ALL ON SCHEMA PUBLIC TO "entg-dev";

*/

-- global settings

CREATE EXTENSION IF NOT EXISTS hstore;

CREATE TYPE "media_type" AS ENUM ('Image', 'Video', 'VideoSlice', 'Audio');

CREATE OR REPLACE FUNCTION make_ts_vec(
    p text,
    n text,
    t text[]
) RETURNS tsvector
LANGUAGE sql IMMUTABLE AS $$
    SELECT to_tsvector('english'::regconfig, p || ' ' || n || ' ' || array_to_string(t, ' '));
$$;

--
-- libraries
--

DROP TABLE IF EXISTS "libraries" CASCADE;

CREATE TABLE "libraries" (
    "library_uuid" uuid NOT NULL,
    "path" varchar(1024) NOT NULL,
    "uid" varchar(64) NOT NULL,
    "gid" varchar(64) NOT NULL,
    "count" bigint NOT NULL,
    PRIMARY KEY ("library_uuid"),
    CONSTRAINT "library_path" UNIQUE ("path")
);

CREATE INDEX "idx_libraries_gid" ON "libraries" ("gid");

--
-- media
--

DROP TABLE IF EXISTS "media" CASCADE;

CREATE TABLE "media" (
    "media_uuid" uuid NOT NULL,
    "library_uuid" uuid NOT NULL,
    "path" varchar(1024) NOT NULL,
    "size" bigint NOT NULL,
    "chash" char(128) NOT NULL,
    "phash" char(64) NOT NULL,
    "mtime" bigint NOT NULL,
    "hidden" boolean NOT NULL,
    "date" varchar(64) NOT NULL,
    "note" text NOT NULL,
    "tags" hstore NOT NULL,
    "media_type" media_type NOT NULL,
    "cdate" timestamptz DEFAULT NULL,
    "ts_vec" tsvector GENERATED ALWAYS AS (make_ts_vec("date", "note", akeys("tags"))) STORED,
    PRIMARY KEY ("media_uuid"),
    CONSTRAINT "media_files" UNIQUE ("library_uuid", "path"),
    CONSTRAINT "media_library" FOREIGN KEY ("library_uuid") REFERENCES "libraries" ("library_uuid")
);

CREATE INDEX "idx_media_library" ON "media" ("library_uuid");

CREATE INDEX "idx_media_path" ON "media" ("path");

CREATE INDEX "idx_media_chash" ON "media" ("chash");

CREATE INDEX "idx_media_cdate" ON "media" ("cdate");

CREATE INDEX "idx_media_tags" ON "media" USING GIN ("tags");

CREATE INDEX "idx_media_fulltext" ON "media" USING GIN ("ts_vec");

--
-- comments
--

DROP TABLE IF EXISTS "comments" CASCADE;

CREATE TABLE "comments" (
    "comment_uuid" uuid NOT NULL,
    "media_uuid" uuid NOT NULL,
    "date" bigint NOT NULL,
    "uid" varchar(64) NOT NULL,
    "text" text NOT NULL,
    PRIMARY KEY ("comment_uuid"),
    CONSTRAINT "comment_media" FOREIGN KEY ("media_uuid") REFERENCES "media" ("media_uuid")
);

CREATE INDEX "idx_comment_media" ON "comments" ("media_uuid");

--
-- collections
--

DROP TABLE IF EXISTS "collections" CASCADE;

CREATE TABLE "collections" (
    "collection_uuid" uuid NOT NULL,
    "uid" varchar(64) NOT NULL,
    "gid" varchar(64) NOT NULL,
    "name" varchar(256) NOT NULL,
    "note" text NOT NULL,
    "tags" hstore NOT NULL,
    "cover" uuid DEFAULT NULL,
    "ts_vec" tsvector GENERATED ALWAYS AS (make_ts_vec("name", "note", akeys("tags"))) STORED,
    PRIMARY KEY ("collection_uuid"),
    CONSTRAINT "collection_name" UNIQUE ("uid", "name"),
    CONSTRAINT "collection_cover" FOREIGN KEY ("cover") REFERENCES "media" ("media_uuid")
);

CREATE INDEX "idx_collection_gid" ON "collections" ("gid");

CREATE INDEX "idx_collection_tags" ON "collections" USING GIN ("tags");

CREATE INDEX "idx_collection_fulltext" ON "collections" USING GIN ("ts_vec");

--
-- collection contents
--

DROP TABLE IF EXISTS "collection_contents" CASCADE;

CREATE TABLE "collection_contents" (
    "id" bigint GENERATED ALWAYS AS IDENTITY,
    "media_uuid" uuid NOT NULL,
    "collection_uuid" uuid NOT NULL,
    PRIMARY KEY ("id"),
    CONSTRAINT "cc_unique" UNIQUE ("media_uuid", "collection_uuid"),
    CONSTRAINT "cc_media" FOREIGN KEY ("media_uuid") REFERENCES "media" ("media_uuid"),
    CONSTRAINT "cc_collection" FOREIGN KEY ("collection_uuid") REFERENCES "collections" ("collection_uuid")
);

CREATE INDEX "idx_cc_media" ON "collection_contents" ("media_uuid");

CREATE INDEX "idx_cc_collection" ON "collection_contents" ("collection_uuid");
