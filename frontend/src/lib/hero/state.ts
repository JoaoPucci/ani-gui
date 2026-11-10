/**
 * What Home's hero shows, from what the trending fetch has produced so
 * far. The route renders one branch per kind; this decides which.
 */

/** The hero's state: the show it features, or why it features none. */
export type HeroState<T> =
	| { kind: 'loading' }
	| { kind: 'featured'; show: T }
	| { kind: 'empty' }
	| { kind: 'failed' };

export interface HeroStateInput<T> {
	/** The hero's rotation ($lib/hero/rotation's `hero`), or `null`
	 *  while the trending list has not arrived. */
	rotation: readonly T[] | null;
	/** Whether the trending fetch failed. */
	failed: boolean;
	/** The rotation index the pager or the timer has reached. */
	index: number;
}

export function heroState<T>({ rotation, failed, index }: HeroStateInput<T>): HeroState<T> {
	if (rotation && rotation.length > 0) {
		return { kind: 'featured', show: rotation[index % rotation.length] };
	}
	if (failed) return { kind: 'failed' };
	// The list arrived with no show the hero can take: it came back
	// empty, or the availability filter dropped every show in it.
	if (rotation) return { kind: 'empty' };
	return { kind: 'loading' };
}
