/**
 * The cues of the video's sidecar tracks, kept through something that
 * clears the cues of every text track on the video.
 */

/** The part of a `TextTrack` this reads and writes. */
export interface CueTrack {
	mode: TextTrackMode;
	readonly cues: ArrayLike<TextTrackCue> | null;
	addCue(cue: TextTrackCue): void;
}

/** The part of an `HTMLTrackElement` this reads. */
export interface CueTrackElement {
	readonly readyState: number;
	readonly track: CueTrack;
}

/** The part of the video this reads: its `<track>` elements. */
export interface CueVideo {
	querySelectorAll(selector: 'track'): Iterable<CueTrackElement>;
}

/** Runs `clear`. */
export function keepingSidecarCues(_video: CueVideo, clear: () => void): void {
	clear();
}
