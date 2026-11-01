/**
 * The captions picker's effect on the video's text tracks: the chosen
 * track is shown and every other one is set aside.
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

/**
 * The mode each track takes for `target`, in list order, or `null`
 * where the track is already in that mode and is left untouched.
 */
export function captionModes(
	tracks: readonly PickableTrack[],
	target: CaptionTarget
): (CaptionMode | null)[] {
	return tracks.map((track, i) => {
		const mode: CaptionMode = target === i ? 'showing' : 'hidden';
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
