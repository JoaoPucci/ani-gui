/**
 * A brake on hls.js asking for one fragment again and again.
 *
 * When a fragment loads but nothing of it reaches the buffer — every
 * frame of it landing before zero, say — hls.js forgets it and asks
 * for it again, and does so as fast as the host answers. A live run
 * of exactly that did a hundred requests a second until the host
 * refused the address with 429s, which took every other stream with
 * it, and the requests went on after the user had left the page,
 * because the engine outlives the route for picture-in-picture.
 *
 * A playing stream loads each fragment once. A rewind past the back
 * buffer loads one again, minutes apart. The guard counts loads of one
 * fragment inside a short window, and past the allowance it tells the
 * player to stop the engine rather than ask again. The allowance is
 * above hls.js's own retries for a fragment that fails to load — one
 * on timeout, three on error — so it never trips on a retry that the
 * load policy would have made anyway.
 */

export const FRAGMENT_LOAD_ALLOWANCE = 4;
export const FRAGMENT_LOAD_WINDOW_MS = 10_000;

/** What hls.js hands a fragment-loaded listener, as much of it as the
 *  key needs: the rendition (`main`, `audio`, `subtitle`), whose
 *  playlists number their fragments independently, then the level
 *  and the sequence number within it. */
export interface FragmentIdentity {
	type?: string;
	level?: number;
	sn?: number | string;
}

/** The key one fragment loads under: the same fragment of the same
 *  rendition, and nothing else, shares it. */
export function fragmentLoopKey(frag: FragmentIdentity): string {
	return `${frag.type ?? 'main'}:${frag.level ?? 0}:${frag.sn ?? ''}`;
}

export class FragmentLoopGuard {
	private readonly loads = new Map<string, number[]>();

	constructor(private readonly now: () => number = () => Date.now()) {}

	/** Records a load of the fragment `key` names — its level and
	 *  sequence number. True when that fragment has now loaded more
	 *  times inside the window than the allowance. */
	loaded(key: string): boolean {
		const at = this.now();
		const recent = (this.loads.get(key) ?? []).filter((t) => at - t < FRAGMENT_LOAD_WINDOW_MS);
		recent.push(at);
		this.loads.set(key, recent);
		return recent.length > FRAGMENT_LOAD_ALLOWANCE;
	}
}
