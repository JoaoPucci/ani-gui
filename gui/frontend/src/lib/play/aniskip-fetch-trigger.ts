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
 *   - `clear`  → caller should clear `skipIntervals`. Only emitted on
 *                the transition where the key drops to incomplete
 *                after a previous fetch — repeated incomplete steps
 *                stay idle, so the caller doesn't churn the
 *                reactive `SkipInterval[]` slot.
 *   - `idle`   → no-op; either we're waiting for a usable duration,
 *                the key is unchanged, or the key has been incomplete
 *                since the last clear.
 */

export type AniskipDecision =
	| { kind: 'fetch'; showId: string; episode: string }
	| { kind: 'clear' }
	| { kind: 'idle' };

export interface AniskipFetchTrigger {
	step(showId: string, episodeNum: number, durationReady: boolean): AniskipDecision;
}

export function createAniskipFetchTrigger(): AniskipFetchTrigger {
	let fetchedKey: string | null = null;
	return {
		step(showId, episodeNum, durationReady) {
			const ep = episodeNum > 0 ? String(episodeNum) : '';
			const key = showId && ep ? `${showId}|${ep}` : null;
			if (!key) {
				if (fetchedKey === null) return { kind: 'idle' };
				fetchedKey = null;
				return { kind: 'clear' };
			}
			if (!durationReady) return { kind: 'idle' };
			if (key === fetchedKey) return { kind: 'idle' };
			fetchedKey = key;
			return { kind: 'fetch', showId, episode: ep };
		}
	};
}
