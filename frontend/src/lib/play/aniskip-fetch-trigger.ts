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
 *   - `clear`  → caller should clear `skipIntervals`. Emitted when the
 *                episode changes (or the key drops to incomplete)
 *                after a fetch, before the new episode's length is
 *                usable: across a switch the last episode's times are
 *                not this one's while its own wait for its stream.
 *                Once cleared, further steps stay idle until the next
 *                fetch, so the caller doesn't churn the reactive
 *                `SkipInterval[]` slot.
 *   - `idle`   → no-op; either we're waiting for a usable duration,
 *                the key is unchanged, or nothing has been asked for
 *                since the last clear (the episode may have changed
 *                again meanwhile: there is nothing on hand to clear).
 */

export type AniskipDecision =
	| { kind: 'fetch'; showId: string; episode: string }
	| { kind: 'clear' }
	| { kind: 'idle' };

export interface AniskipFetchTrigger {
	step(showId: string, episodeNum: number, durationReady: boolean): AniskipDecision;
	/** Which request the last `fetch` decision issued: an answer is the
	 *  page's to show only while `answers` still says so. */
	asked(): number;
	answers(ask: number): boolean;
}

/**
 * Whether the page's `duration` is a length to ask aniskip with: the
 * stream it describes is the one attached for `session`, the session
 * on screen, and it has learned a usable length.
 */
export function lengthReady(lengthFor: string | null, session: string, duration: number): boolean {
	return lengthFor !== null && lengthFor === session && Number.isFinite(duration) && duration > 0;
}

export function createAniskipFetchTrigger(): AniskipFetchTrigger {
	// The episode the slot is for, whether its times have been asked
	// for, and whether anything asked since the last clear may be on
	// hand.
	let slotKey: string | null = null;
	let fetched = false;
	let held = false;
	// Counts every fetch and clear: an answer is shown only while no
	// later one has happened.
	let ask = 0;
	return {
		step(showId, episodeNum, durationReady) {
			// aniskip numbers episodes as MAL does, in whole numbers; a
			// fractional extra (a 7.5 recap) has no skip times to ask for.
			const ep = episodeNum > 0 && Number.isInteger(episodeNum) ? String(episodeNum) : '';
			const key = showId && ep ? `${showId}|${ep}` : null;
			if (key !== slotKey) {
				slotKey = key;
				fetched = false;
			}
			if (key && durationReady && !fetched) {
				fetched = true;
				held = true;
				ask += 1;
				return { kind: 'fetch', showId, episode: ep };
			}
			if (held && !fetched) {
				held = false;
				ask += 1;
				return { kind: 'clear' };
			}
			return { kind: 'idle' };
		},
		asked() {
			return ask;
		},
		answers(n) {
			return n === ask && fetched;
		}
	};
}
