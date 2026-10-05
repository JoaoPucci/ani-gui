-- Mark the reverse mappings a play stored before plays marked them.
--
-- A play stores the `show key -> kitsu id` mapping
-- (`allmanga2kitsu:v3:<show>`) with a mark beside it naming the id it
-- stored (`allmanga2kitsu:played:v1:<show>`); a mapping is the play's
-- while the mark names it. Earlier builds left no mark, and read a
-- mapping as the play's when it was written from a second before to
-- ten seconds after the show's watch stamp (`watched-at:v1:<show>`,
-- epoch milliseconds), both rows unexpired. This marks, once, every
-- mapping that rule read as played, so a play stored before the
-- upgrade keeps its standing. The mark takes the mapping's moment and
-- lifetime, and so expires with it.

INSERT OR REPLACE INTO meta_cache(key, body, fetched_at, ttl_seconds)
SELECT 'allmanga2kitsu:played:v1:' || substr(m.key, 19),
       m.body,
       m.fetched_at,
       m.ttl_seconds
FROM meta_cache AS m
JOIN meta_cache AS w
  ON w.key = 'watched-at:v1:' || substr(m.key, 19)
WHERE substr(m.key, 1, 18) = 'allmanga2kitsu:v3:'
  AND CAST(strftime('%s', 'now') AS INTEGER) - m.fetched_at < m.ttl_seconds
  AND CAST(strftime('%s', 'now') AS INTEGER) - w.fetched_at < w.ttl_seconds
  AND w.body <> ''
  AND w.body NOT GLOB '*[^0-9]*'
  AND m.fetched_at * 1000 - CAST(w.body AS INTEGER) BETWEEN -1000 AND 10000;
