/**
 * Fixture data for the watch-order mockups (0.19.0, phase 0).
 *
 * Every entry is a real Kitsu anime, read from Kitsu's
 * `media-relationships` on 2026-11-02: titles, formats, start dates,
 * episode counts and image URLs are as Kitsu served them. `links` are
 * the prequel/sequel pairs between run members, from either side's
 * listing; which members form a run (and which entries are other
 * versions, films, side stories or recaps) was decided by hand from
 * the proposal's rules. `parents` names, for three side stories
 * (*Fate/Zero: Onegai! Einzbern Soudanshitsu*, *Gintama°: Aizome
 * Kaori-hen*, *Kishibe Rohan wa Ugokanai*), the entries Kitsu lists as
 * their `parent_story`, read the same day. `progress` is made up for the mockups so each
 * state shows on screen; JoJo's and Gintama's follow the development
 * profile's watch history. The data lives in `fixtures.json`: titles are
 * Kitsu's, not interface copy. Nothing here is fetched or computed at
 * runtime; the real feature replaces this module with the backend's
 * walk.
 */
import raw from './fixtures.json';
import type { MockRun } from './types';

export const MOCK_RUNS: readonly MockRun[] = raw as unknown as MockRun[];
