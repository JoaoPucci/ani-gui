/**
 * A brake on hls.js asking for one fragment again and again.
 *
 * When a fragment loads but nothing of it reaches the buffer — every
 * frame of it landing before zero, say — hls.js forgets it and asks
 * for it again, and does so as fast as the host answers. A live run
 * of exactly that did well over a hundred requests a second until the
 * host refused the address with 429s, which took every other stream
 * with it, and the requests went on after the user had left the page,
 * because the engine then outlived the route.
 *
 * A playing stream loads each fragment once. A seek back past the
 * back buffer loads one again, and a viewer going back and forth
 * between two spots further apart than the back buffer does so on
 * every return, as quickly as they move; each of those loads follows
 * the engine letting the fragment go, so a release forgets the loads
 * of every fragment it took out of the buffer. A fragment that loads
 * and never buffers sits at the playhead, which no release reaches.
 * The guard counts loads of one fragment inside a short window, and
 * past the allowance it tells the player to stop the engine rather
 * than ask again. The allowance is
 * above hls.js's own retries for a fragment that loads but cannot be
 * parsed or appended — one on timeout, three on error, four loads
 * before its own fatal — so it never trips on a retry the load policy
 * would have made anyway.
 */

import type { Fragment } from 'hls.js';

export const FRAGMENT_LOAD_ALLOWANCE = 4;
export const FRAGMENT_LOAD_WINDOW_MS = 10_000;

/** As much of hls.js's fragment as the key needs: the rendition
 *  (`main`, `audio`, `subtitle`), whose playlists number their
 *  fragments independently, then the level and the sequence number
 *  within it. Typed against hls.js's own fragment so a renamed field
 *  is a compile error. */
export type FragmentIdentity = Pick<Fragment, 'type' | 'level' | 'sn'>;

/** The key one fragment loads under: the same fragment of the same
 *  rendition, and nothing else, shares it. */
export function fragmentLoopKey(frag: FragmentIdentity): string {
	return `${frag.type}:${frag.level}:${frag.sn}`;
}

export class FragmentLoopGuard {
	// Per fragment: the times it loaded inside the window, and where it
	// ends in the stream (seconds).
	private readonly loads = new Map<string, { at: number[]; end: number }>();
	private tripped = false;

	constructor(private readonly now: () => number = () => Date.now()) {}

	/** Whether the guard has tripped: the engine it stopped is stopped,
	 *  and nothing else should start it again. */
	get hasTripped(): boolean {
		return this.tripped;
	}

	/** Records a load of the fragment `key` names — its rendition,
	 *  level and sequence number — ending at `end` seconds. True the one time a fragment has
	 *  loaded more times inside the window than the allowance: the
	 *  engine the caller then stops is stopped, and a late report is
	 *  not a second trip. */
	loaded(key: string, end: number = Infinity): boolean {
		if (this.tripped) return false;
		const at = this.now();
		const recent = (this.loads.get(key)?.at ?? []).filter((t) => at - t < FRAGMENT_LOAD_WINDOW_MS);
		recent.push(at);
		this.loads.set(key, { at: recent, end });
		this.tripped = recent.length > FRAGMENT_LOAD_ALLOWANCE;
		return this.tripped;
	}

	/** The engine let go of everything buffered before `upTo` seconds.
	 *  A fragment ending there or earlier left the buffer, so its next
	 *  load is the first since; one the release did not reach keeps its
	 *  count. */
	released(upTo: number): void {
		for (const [key, { end }] of this.loads) if (end <= upTo) this.loads.delete(key);
	}
}
