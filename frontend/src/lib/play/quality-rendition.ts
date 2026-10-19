/**
 * Which rendition an engine report is about: the main one — the video
 * the levels are levels of — or a side one, audio or subtitles, whose
 * failure hls.js answers by switching to a level of another group.
 */

import type { ErrorData, PlaylistContextType, PlaylistLevelType } from 'hls.js';

// hls.js's own names, spelled out so the engine double of a test that
// mocks the module still meets them, and typed against the engine's
// enums so a renamed one is a compile error.
const MAIN: `${PlaylistLevelType.MAIN}` = 'main';
const sidePlaylists: `${PlaylistContextType}`[] = ['audioTrack', 'subtitleTrack'];
const SIDE_PLAYLISTS: ReadonlySet<string> = new Set(sidePlaylists);

/** Whether a fragment is of the main rendition. */
export function isMain(frag: { type?: unknown } | undefined): boolean {
	return String(frag?.type) === MAIN;
}

/** Whether an error is a side rendition's: an audio or subtitle
 *  segment, an audio-track or subtitle-track playlist, or the audio
 *  source buffer. */
export function isSideRendition(
	data: Pick<ErrorData, 'frag' | 'context' | 'sourceBufferName'>
): boolean {
	if (data.frag) return !isMain(data.frag);
	return SIDE_PLAYLISTS.has(String(data.context?.type)) || data.sourceBufferName === 'audio';
}
