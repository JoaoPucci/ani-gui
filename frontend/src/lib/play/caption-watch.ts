/**
 * Which changes to the video's captions are the viewer's own picks.
 *
 * The captions change two ways: the app sets them — the session's
 * start and its tracks switched off as they go
 * ({@link import('./sidecar-tracks')}), and a pick in the app's own
 * captions menu ({@link import('./caption-pick')}); and the viewer
 * sets them through the browser's own captions menu, which shows when
 * the app's custom controls are turned off. Only the second is a pick
 * the app has not seen, and the show's memory
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
 * not remembered — logged in docs/deferred-work.md as "Native-menu
 * captions picks on a playlist-only stream".
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

/** The caption tracks showing beside the one the app expects, while
 *  that one shows: tracks the browser turned on itself. Its own
 *  captions menu leaves one track showing, or none, so a viewer's pick
 *  never looks like this. */
export function extraBeside(expected: CaptionState | null, list: WatchableTrackList): number[] {
	if (typeof expected !== 'number' || expected >= list.length) return [];
	const isCaption = (t: WatchableTrack) => t.kind === 'subtitles' || t.kind === 'captions';
	const start = list[expected];
	if (!isCaption(start) || start.mode !== 'showing') return [];
	const extra: number[] = [];
	for (let i = 0; i < list.length; i++) {
		if (i !== expected && isCaption(list[i]) && list[i].mode === 'showing') extra.push(i);
	}
	return extra;
}

/** The viewer's pick a change shows, or `null`: none before the app
 *  has set the captions (`expected` null), and none where they are as
 *  the app set them. */
export function viewerPick(expected: CaptionState | null, now: CaptionState): CaptionState | null {
	return expected === null || now === expected ? null : now;
}

const expected = new WeakMap<object, CaptionState | null>();

/** Note what the app set the video's captions to — or `null` while it
 *  is not in charge of them (tracks being removed). */
export function expectCaptions(video: object, state: CaptionState | null): void {
	expected.set(video, state);
}

/** What the app last set the video's captions to, or `null`. */
export function expectedCaptions(video: object): CaptionState | null {
	return expected.get(video) ?? null;
}

/** The viewer's pick the video's captions now show, if they show one;
 *  it becomes what the app expects from then on. A track the browser
 *  turned on beside the one the app expects is no pick
 *  ({@link extraBeside}): the app switches it back off. */
export function noteCaptionChange(video: object, list: WatchableTrackList): CaptionState | null {
	if (extraBeside(expected.get(video) ?? null, list).length > 0) return null;
	const now = showingCaption(list);
	const pick = viewerPick(expected.get(video) ?? null, now);
	if (pick !== null) expected.set(video, now);
	return pick;
}
