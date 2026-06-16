-- PR #1 of the account-integration chain.
--
-- Per-provider snapshot of the user's anime list. Populated by
-- `commands::account::resync_list` (PR #1) on connect / manual refresh
-- and on the 5-min TTL boundary. Consumed by:
--
--   - PR #2 home Watch Later rail (filters on status='planning')
--   - PR #4 mark-watched optimistic write-through
--
-- Cross-provider dedupe pivots on `mal_id`: AniList exposes this as
-- `idMal`, MAL returns the same value as `media_id`. The Watch Later
-- merger joins on `mal_id` to collapse rows present on both lists.
--
-- TTL semantics live in `db.rs` — the schema only stores the timestamp.

CREATE TABLE user_list_cache (
    provider     TEXT    NOT NULL,           -- 'anilist' | 'mal' | 'inhouse'
    user_id      TEXT    NOT NULL,           -- provider-stable user id
    media_id     INTEGER NOT NULL,           -- provider-native media id
    mal_id       INTEGER,                    -- cross-provider bridge; nullable
    status       TEXT    NOT NULL,           -- unified ListStatus snake_case
    progress     INTEGER NOT NULL DEFAULT 0, -- episodes watched
    score_x100   INTEGER,                    -- 0..=100 unified scale; NULL if unrated
    updated_at   INTEGER NOT NULL,           -- provider's updated timestamp (epoch s)
    fetched_at   INTEGER NOT NULL,           -- when we cached this row (epoch s)
    title        TEXT,                       -- fallback display only
    PRIMARY KEY (provider, user_id, media_id)
);

-- Cross-provider dedupe lookup (Watch Later merger):
CREATE INDEX user_list_cache_mal_idx ON user_list_cache(mal_id);

-- Rail composition (per-user, per-status filters):
CREATE INDEX user_list_cache_status_idx
    ON user_list_cache(provider, user_id, status);
