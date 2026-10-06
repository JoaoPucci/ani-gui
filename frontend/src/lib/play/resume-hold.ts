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
 *  seek snapped to a keyframe or nudged over a gap lands within it,
 *  and it is well short of the 15 seconds a kept point is past zero,
 *  so the engine placing the playhead at the stream's start never
 *  reads as the resume. */
export const SEEK_LANDED_WITHIN_S = 2;

/** How long the hold waits before it reveals anyway and plays from
 *  where the stream is. It is the engine's own budget for loading one
 *  fragment: a healthy host has the fragment at the point in well
 *  before, and past it the stall handling has declared the load
 *  failed and owns what the viewer sees — a picture held back past
 *  that would only hide it. */
export const RESUME_HOLD_BOUND_MS = HLS_STALL_LOAD_POLICY.fragLoadPolicy.default.maxLoadTimeMs;

/** `HTMLMediaElement.HAVE_CURRENT_DATA`: the frame at the playhead is in. */
const HAVE_CURRENT_DATA = 2;

export type ResumeHold = {
	/** Seeks to `seconds`; the hold reveals once that seek landed and
	 *  playback started. */
	seekTo(seconds: number): void;
	/** Nothing is to be sought after all: reveal and play from here. */
	release(): void;
	/** Ends the hold without starting anything — the source is gone. */
	end(): void;
};

export function holdForResume(input: {
	video: HTMLVideoElement;
	onHold: (holding: boolean) => void;
	/** The bound ran out before a seek landed. */
	onGiveUp: () => void;
}): ResumeHold {
	const { video, onHold, onGiveUp } = input;
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
	// The frame at the point shows once frames play; a seek reported
	// done before the stream buffered there has nothing to show yet.
	const onPlaying = () => finish();
	const onFrame = () => {
		if (video.readyState >= HAVE_CURRENT_DATA) finish();
	};
	const timer = setTimeout(() => {
		onGiveUp();
		start();
		finish();
	}, RESUME_HOLD_BOUND_MS);

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
			if (done) return;
			target = seconds;
			video.addEventListener('seeked', onSeeked);
			video.currentTime = seconds;
		},
		release() {
			start();
			finish();
		},
		end: finish
	};
}
