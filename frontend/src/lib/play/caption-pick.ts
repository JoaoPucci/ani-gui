/**
 * The captions picker's effect on the video's text tracks: the chosen
 * subtitle track is shown and every other one is disabled.
 *
 * Disabled, not hidden: a hidden track is still loaded — the browser
 * fetches its file and parses its cues — so hiding the unchosen ones
 * fetched every track a session lists, dozens for a listing that
 * carries every language, on the first pick. A disabled track is never
 * fetched, so only the track shown costs a request.
 *
 * Only subtitle and captions tracks are the picker's. A track of any
 * other kind — hls.js keeps a `metadata` track for the stream's ID3
 * cues — is left as it is.
 *
 * Pure over the modes, so the rule is tested without a video; the
 * play page hands it the live TextTrackList ({@link applyCaptionPick}).
 */

export type CaptionMode = 'showing' | 'hidden' | 'disabled';

/** What the pick reads of a track: its kind and its current mode. */
export interface PickableTrack {
	kind: string;
	mode: CaptionMode;
}

/** A track's index in the video's TextTrackList, or every track off. */
export type CaptionTarget = number | 'off';

function isCaptionTrack(track: PickableTrack): boolean {
	return track.kind === 'subtitles' || track.kind === 'captions';
}

/**
 * The mode each track takes for `target`, in list order, or `null`
 * where the track is left untouched: a track that is not a subtitle
 * track, or one already in the mode the pick gives it.
 */
export function captionModes(
	tracks: readonly PickableTrack[],
	target: CaptionTarget
): (CaptionMode | null)[] {
	return tracks.map((track, i) => {
		if (!isCaptionTrack(track)) return null;
		const mode: CaptionMode = target === i ? 'showing' : 'disabled';
		return track.mode === mode ? null : mode;
	});
}

/** A TextTrackList as the pick needs it. */
export interface PickableTrackList {
	readonly length: number;
	[index: number]: PickableTrack;
}

/** Apply {@link captionModes} to a live TextTrackList. */
export function applyCaptionPick(list: PickableTrackList, target: CaptionTarget): void {
	const tracks: PickableTrack[] = [];
	for (let i = 0; i < list.length; i++) tracks.push(list[i]);
	captionModes(tracks, target).forEach((mode, i) => {
		if (mode !== null) list[i].mode = mode;
	});
}
