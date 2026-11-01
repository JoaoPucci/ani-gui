/**
 * Which changes to the video's captions are the viewer's own picks.
 */

/** Which subtitle track shows: its index in the TextTrackList, or off. */
export type CaptionState = number | 'off';

/** What the watch reads of a track. */
export interface WatchableTrack {
	kind: string;
	mode: string;
}

/** A TextTrackList as the watch reads it. */
export interface WatchableTrackList {
	readonly length: number;
	[index: number]: WatchableTrack;
}

/** The subtitle or captions track showing, or off. */
export function showingCaption(list: WatchableTrackList): CaptionState {
	void list; // not read yet
	return 'off';
}

/** The viewer's pick a change shows, or `null`. */
export function viewerPick(expected: CaptionState | null, now: CaptionState): CaptionState | null {
	void expected;
	void now;
	return null;
}

/** Note what the app set the video's captions to. */
export function expectCaptions(video: object, state: CaptionState | null): void {
	void video;
	void state;
}

/** The viewer's pick the video's captions now show, if they show one. */
export function noteCaptionChange(video: object, list: WatchableTrackList): CaptionState | null {
	void video;
	void list;
	return null;
}
