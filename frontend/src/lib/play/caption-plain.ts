/**
 * The captions of a stream with no engine to steer — one the element
 * plays from its own source — and the one way the play page picks
 * captions, with an engine or without.
 */

import { applyCaptionPick, type CaptionTarget } from './caption-pick';
import type { CaptionSync, CaptionVideo } from './caption-sync-tracks';

/** The captions without an engine: picks set the tracks alone, tracks
 *  of the app's own join with every track off, and nothing waits. */
export function plainCaptions(video: CaptionVideo): CaptionSync {
	return {
		pick: (target) => applyCaptionPick(video.textTracks, target),
		pickTrack: () => {},
		beforeOwnTracks: () => applyCaptionPick(video.textTracks, 'off'),
		release: () => {},
		whenReady: (fn) => {
			fn();
			return () => {};
		},
		renditions: () => [],
		dispose: () => {}
	};
}

/** Show the track at `target`, or none, through the engine where the
 *  stream has one. */
export function pickCaptions(
	video: CaptionVideo,
	target: CaptionTarget,
	sync: CaptionSync | null
): void {
	(sync ?? plainCaptions(video)).pick(target);
}
