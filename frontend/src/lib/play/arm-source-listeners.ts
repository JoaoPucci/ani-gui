/**
 * Source-scoped listener orchestration for a media attach: consume
 * any pending recovery position for this (show, episode), arm the
 * seek-back and the progress marking on the singleton video, and
 * register their removal with the source-scoped cleanups — the
 * attach path flushes the registry before arming, so they die when
 * the NEXT source attaches (which is also what cancels a superseded
 * attach's pending seek) and survive route unmounts,
 * where PiP keeps this exact stream playing. The play page's attach
 * path is one call into here (AGENTS.md §2).
 */

import { recoveryResume } from '$lib/play/resume-after-recovery';
import { stallMachine } from '$lib/play/stall-machine';
import { addSourceScopedCleanup } from '$lib/play/global-video';

export function armSourceScopedListeners(input: {
	video: HTMLVideoElement;
	showId: string;
	episode: number;
}): void {
	const { video } = input;
	const resumeAt = recoveryResume.consume(input.showId, input.episode);
	// Progress means frames actually rendered — the `playing` event —
	// never a bare timeupdate: the resume seek below emits one at the
	// old timestamp before the fresh source has delivered anything.
	const markProgress = () => {
		stallMachine.progressed();
	};
	const seekBack =
		resumeAt !== null
			? () => {
					video.currentTime = resumeAt;
				}
			: null;
	addSourceScopedCleanup(() => {
		video.removeEventListener('playing', markProgress);
		if (seekBack) video.removeEventListener('loadedmetadata', seekBack);
	});
	video.addEventListener('playing', markProgress);
	if (seekBack) video.addEventListener('loadedmetadata', seekBack, { once: true });
}
