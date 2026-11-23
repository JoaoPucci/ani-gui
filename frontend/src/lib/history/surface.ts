/**
 * The surface an in-place link session lives on — Home or the player —
 * and whether the user has left it. Split from
 * $lib/history/in-place-link.svelte.ts for the per-file complexity bar.
 */

/** Where an in-place session is: on its surface, or gone from it. */
export interface Surface {
	readonly gone: boolean;
	/** The page left: whatever answers later answers to nothing here. */
	leave: () => void;
	/** The path the session reports as its own: always the same while
	 *  the surface is on screen, somewhere else once it is gone. */
	here: () => string;
}

export function surface(): Surface {
	let gone = false;
	return {
		get gone() {
			return gone;
		},
		leave: () => {
			gone = true;
		},
		here: () => (gone ? 'gone' : '')
	};
}
