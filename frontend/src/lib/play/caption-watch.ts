/**
 * Which changes to the video's captions are the viewer's own picks.
 *
 * The captions change three ways: the app sets them — the session's
 * start ({@link import('./sidecar-tracks')}) and a pick in the app's
 * own captions menu ({@link import('./caption-pick')}); the browser
 * sets them by itself while the tracks are being attached; and the
 * viewer sets them through the browser's own captions menu, which
 * shows when the app's custom controls are turned off. Only the last
 * is a pick the app has not seen, and the show's memory
 * ({@link import('./caption-choice')}) should take it.
 *
 * The app notes what it set, per video ({@link expectCaptions}); a
 * change the video's TextTrackList reports is the viewer's when the
 * app had set the captions and they now differ. Before the start is
 * set, and once the session goes, nothing is. The page asks only
 * while its own controls are off: with them on there is no browser
 * menu, and a change the app did not make is hls.js's. The start is
 * set where a session has sidecar tracks, so on a stream that carries
 * its subtitles only in its playlist a pick in the browser's menu is
 * not remembered — logged in docs/deferred-work.md as "The starting
 * captions on a playlist-only stream".
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
	for (let i = 0; i < list.length; i++) {
		const t = list[i];
		if ((t.kind === 'subtitles' || t.kind === 'captions') && t.mode === 'showing') return i;
	}
	return 'off';
}

/** The viewer's pick a change shows, or `null`: none before the app
 *  has set the captions (`expected` null), and none where they are as
 *  the app set them. */
export function viewerPick(expected: CaptionState | null, now: CaptionState): CaptionState | null {
	return expected === null || now === expected ? null : now;
}

const expected = new WeakMap<object, CaptionState | null>();

/** Note what the app set the video's captions to — or `null` while it
 *  is not in charge of them (tracks being attached or removed). */
export function expectCaptions(video: object, state: CaptionState | null): void {
	expected.set(video, state);
}

/** The viewer's pick the video's captions now show, if they show one;
 *  it becomes what the app expects from then on. */
export function noteCaptionChange(video: object, list: WatchableTrackList): CaptionState | null {
	const now = showingCaption(list);
	const pick = viewerPick(expected.get(video) ?? null, now);
	if (pick !== null) expected.set(video, now);
	return pick;
}
