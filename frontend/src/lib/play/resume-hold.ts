/**
 * Holds a stream's picture back while its resume seek is on its way.
 *
 * An episode opened at a point would otherwise show its first frame —
 * and autoplay from it — until the metadata, or a known length, lets
 * the resume seek move the playhead. While the hold is on, the page
 * shows its loading treatment over a hidden picture with its controls
 * inert, the element's autoplay is off and anything else that starts
 * playback is paused: the resume wins. The hold reveals once the seek
 * landed at the point and playback started there, or, autoplay
 * refused, once the frame at the point is in.
 */

import { HLS_STALL_LOAD_POLICY } from './hls-load-policy';

/** How far from its point a seek may land and count as landed: a
 *  seek snapped to a keyframe or nudged over a gap lands within it.
 *  A kept point is at least 15 seconds in, so the engine placing the
 *  playhead at the stream's start never reads as its resume; a
 *  recovery's point may be nearer zero than this, where the start is
 *  as good as the point. */
export const SEEK_LANDED_WITHIN_S = 2;

/** How long each wait of the hold lasts before it reveals anyway:
 *  the wait for a seek to issue — the metadata, or a known length —
 *  and, once it is issued, the wait for it to land and play. Each is
 *  a wait on one fragment, the first one or the one at the point, and
 *  this is how long the engine lets one fragment take to load before
 *  it counts the load as timed out. */
export const RESUME_HOLD_BOUND_MS = HLS_STALL_LOAD_POLICY.fragLoadPolicy.default.maxLoadTimeMs;

/** `HTMLMediaElement.HAVE_CURRENT_DATA`: the frame at the playhead is in. */
const HAVE_CURRENT_DATA = 2;

export type ResumeHold = {
	/** Seeks to `seconds`; while the hold is on, it reveals once that
	 *  seek landed and playback started. */
	seekTo(seconds: number): void;
	/** Nothing is to be sought after all: reveal and play from here. */
	release(): void;
	/** Ends the hold without starting anything — the source is gone. */
	end(): void;
};

export function holdForResume(input: {
	video: HTMLVideoElement;
	onHold: (holding: boolean) => void;
}): ResumeHold {
	const { video, onHold } = input;
	const autoplay = video.autoplay;
	let target: number | null = null;
	// Set once the hold starts playback itself, so its own `play` is
	// not held back.
	let starting = false;
	let done = false;

	const holdBack = () => {
		if (!starting) video.pause();
	};
	const onSeeked = () => {
		if (target !== null && Math.abs(video.currentTime - target) <= SEEK_LANDED_WITHIN_S) start();
	};
	// The picture is revealed once frames play at the point, not at
	// the seek's own event: what the element shows between the two is
	// not yet a frame playing at the point.
	const onPlaying = () => finish();
	const onFrame = () => {
		if (video.readyState >= HAVE_CURRENT_DATA) finish();
	};
	// The bound only stops hiding the picture: the seek, issued or
	// not, still happens when its turn comes, and where the episode
	// was left is not given up.
	const reveal = () => {
		start();
		finish();
	};
	let timer = setTimeout(reveal, RESUME_HOLD_BOUND_MS);

	const detach = () => {
		clearTimeout(timer);
		video.removeEventListener('play', holdBack);
		video.removeEventListener('seeked', onSeeked);
		video.removeEventListener('playing', onPlaying);
		video.removeEventListener('canplay', onFrame);
		video.autoplay = autoplay;
	};
	const finish = () => {
		if (done) return;
		done = true;
		detach();
		onHold(false);
	};
	const start = () => {
		if (starting) return;
		starting = true;
		video.removeEventListener('seeked', onSeeked);
		video.addEventListener('playing', onPlaying);
		void video.play().then(finish, () => {
			// Autoplay refused: reveal paused, once there is a frame.
			if (done) return;
			video.addEventListener('canplay', onFrame);
			onFrame();
		});
	};

	video.autoplay = false;
	video.addEventListener('play', holdBack);
	onHold(true);

	return {
		seekTo(seconds) {
			if (!done) {
				target = seconds;
				video.addEventListener('seeked', onSeeked);
				clearTimeout(timer);
				timer = setTimeout(reveal, RESUME_HOLD_BOUND_MS);
			}
			video.currentTime = seconds;
		},
		release: reveal,
		end: finish
	};
}
