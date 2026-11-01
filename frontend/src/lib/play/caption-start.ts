/**
 * Which of a session's sidecar tracks the player starts on: the one
 * attached with the `default` attribute, which the browser turns on
 * by itself when the tracks are added. Every other track starts
 * disabled and is never fetched until the captions picker shows it
 * ({@link import('./caption-pick')}).
 */

/** What the start reads of a listed track. */
export interface StartableTrack {
	lang: string;
	default: boolean;
}

/**
 * The index of the track to start on, or `null` for none: the
 * provider's default.
 */
export function startingTrack(
	tracks: readonly StartableTrack[],
	locale: string,
	remembered?: string
): number | null {
	void locale; // not read yet
	void remembered; // not read yet
	const at = tracks.findIndex((t) => t.default === true);
	return at === -1 ? null : at;
}
