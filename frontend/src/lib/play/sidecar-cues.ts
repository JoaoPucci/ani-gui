/**
 * The cues of the video's sidecar tracks, kept through something that
 * clears the cues of every text track on the video.
 *
 * hls.js clears every text track's cues as a manifest starts loading
 * (1.6.19's TimelineController, on MANIFEST_LOADING), the `<track>`
 * elements' included, which it did not add. The browser loads a
 * `<track>`'s file once: with its src unchanged it never asks again,
 * so a sidecar track cleared that way shows nothing for the rest of
 * the play. The play page loads a manifest again when the viewer comes
 * back from a handoff that cut it off ({@link import('./handoff-gate')}).
 *
 * The cues each sidecar track holds are taken before the clear and
 * put back after it, and no track's mode ends up changed:
 *
 * - a track that is showing or hidden is read as it is;
 * - a disabled track hides its cues, so one whose file is in, or on its
 *   way, is set hidden to read them and disabled again at once, as
 *   hls.js itself does to clear them. What shows never changes, so no
 *   caption change is reported as the viewer's pick
 *   ({@link import('./caption-watch')});
 * - a track whose file was never asked for has no cues to keep and is
 *   left alone (hls.js's own clear sets it hidden, which may have the
 *   browser ask for its file; nothing here adds to that);
 * - a cue still on its track is not added again.
 */

/** The part of a `TextTrackCue` this reads: the track it is on, null
 *  once it is removed. */
export interface KeptCue {
	readonly track: unknown;
}

/** The part of a `TextTrack` this reads and writes. */
export interface CueTrack {
	mode: TextTrackMode;
	readonly cues: ArrayLike<KeptCue> | null;
	addCue(cue: KeptCue): void;
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

// HTMLTrackElement.NONE: the track's file was never asked for.
const NOT_LOADED = 0;

/** Runs `clear`, and puts back the cues the video's `<track>` elements
 *  held before it. */
export function keepingSidecarCues(video: CueVideo, clear: () => void): void {
	const kept = Array.from(video.querySelectorAll('track'), (el) => ({
		track: el.track,
		cues: cuesOf(el)
	}));
	try {
		clear();
	} finally {
		for (const { track, cues } of kept) {
			for (const cue of cues) if (cue.track !== track) track.addCue(cue);
		}
	}
}

function cuesOf({ track, readyState }: CueTrackElement): KeptCue[] {
	if (track.mode !== 'disabled') return Array.from(track.cues ?? []);
	if (readyState === NOT_LOADED) return [];
	track.mode = 'hidden';
	const cues = Array.from(track.cues ?? []);
	track.mode = 'disabled';
	return cues;
}
