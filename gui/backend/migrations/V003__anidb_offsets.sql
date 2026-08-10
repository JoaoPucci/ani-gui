-- Provider-numbering offsets for shared-history translation.
-- Deliberately NOT a meta_cache row: ani-hsts entries live
-- indefinitely, so the translation that makes them readable must
-- survive TTL expiry and the diagnostics metadata-cache clear.
CREATE TABLE anidb_offsets (
    slug      TEXT PRIMARY KEY,
    ep_offset INTEGER NOT NULL
);
