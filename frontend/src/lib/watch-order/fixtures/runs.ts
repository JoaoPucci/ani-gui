/**
 * Six real franchises as Kitsu served them on 2026-11-02 (*Bleach*,
 * *Attack on Titan*, *JoJo*, *Gintama*, *Fate* seen from *Fate/Zero*,
 * *Monogatari*): titles, formats, start dates, episode counts and
 * image URLs are Kitsu's, read from its `media-relationships`. `links`
 * are the prequel/sequel pairs between run members, from either side's
 * listing; which members form a run (and which entries are other
 * versions, films, side stories or recaps) was decided by hand from
 * the proposal's rules. `parents` names, for three side stories
 * (*Fate/Zero: Onegai! Einzbern Soudanshitsu*, *Gintama°: Aizome
 * Kaori-hen*, *Kishibe Rohan wa Ugokanai*), the entries Kitsu lists as
 * their `parent_story`, read the same day. `progress` is made up so
 * every state shows; JoJo's and Gintama's follow the development
 * profile's watch history.
 *
 * The ordering's tests read these runs, and the design mockups draw
 * from them. The data lives in `runs.json`: titles are Kitsu's, not
 * interface copy.
 */
import raw from './runs.json';
import type { Progress, WatchRun } from '../types';

export interface FixtureRun extends WatchRun {
	readonly key: string;
	readonly progress: Progress;
}

export const FIXTURE_RUNS: readonly FixtureRun[] = raw as unknown as FixtureRun[];
