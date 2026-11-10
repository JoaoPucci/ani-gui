/**
 * Reactive-fetch trigger for the Player route's aniskip $effect.
 *
 * Before this helper landed, the effect read `duration` reactively to
 * pass it into `aniskipGet`. `timeupdate` drives `duration` (and the
 * hls.js manifest pass refines its estimate at least once), so the
 * effect body re-ran dozens of times per minute even though an inline
 * same-key guard short-circuited the actual fetch. The factory below
 * pulls that guard out so the component can subscribe only to the
 * episode key and a `boolean` "duration is usable" derived — both of
 * which stay stable across duration ticks within an episode.
 *
 * Each `step()` returns:
 *   - `fetch`  → caller should clear stale `skipIntervals` and fire a
 *                fresh `aniskipGet`; the helper records this key as
 *                fetched so further same-key steps stay idle.
 *   - `clear`  → caller should clear `skipIntervals`. Emitted once on
 *                every change of episode key (to another episode, or
 *                to none) that cannot fetch at once, so the last
 *                episode's skip times never drive the new one. The
 *                change also forgets what was asked: an episode come
 *                back to is asked for again.
 *   - `idle`   → no-op; either we're waiting for a usable duration,
 *                or the key is unchanged since its fetch or clear.
 */

export type AniskipDecision =
	| { kind: 'fetch'; showId: string; episode: string }
	| { kind: 'clear' }
	| { kind: 'idle' };

export interface AniskipFetchTrigger {
	step(showId: string, episodeNum: number, durationReady: boolean): AniskipDecision;
}

export function createAniskipFetchTrigger(): AniskipFetchTrigger {
	// The key of the last step (undefined before the first), and the
	// key asked for since it last changed.
	let seenKey: string | null | undefined = undefined;
	let fetchedKey: string | null = null;
	return {
		step(showId, episodeNum, durationReady) {
			// aniskip numbers episodes as MAL does, in whole numbers; a
			// fractional extra (a 7.5 recap) has no skip times to ask for.
			const ep = episodeNum > 0 && Number.isInteger(episodeNum) ? String(episodeNum) : '';
			const key = showId && ep ? `${showId}|${ep}` : null;
			const changed = seenKey !== undefined && key !== seenKey;
			seenKey = key;
			if (changed) fetchedKey = null;
			if (!key || !durationReady || key === fetchedKey) {
				return changed ? { kind: 'clear' } : { kind: 'idle' };
			}
			fetchedKey = key;
			return { kind: 'fetch', showId, episode: ep };
		}
	};
}
