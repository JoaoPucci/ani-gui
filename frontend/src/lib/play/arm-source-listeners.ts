/**
 * Source-scoped listener orchestration for a media attach: consume
 * any pending recovery position for this (show, episode), arm the
 * seek-back and the progress marking on the singleton video, and
 * register their removal with the source-scoped cleanup — so they
 * die when the NEXT source attaches (which is also what cancels a
 * superseded attach's pending seek) and survive route unmounts,
 * where PiP keeps this exact stream playing. The play page's attach
 * path is one call into here (AGENTS.md §2).
 */

import { recoveryResume } from '$lib/play/resume-after-recovery';
import { stallMachine } from '$lib/play/stall-machine';
import { replaceSourceScopedCleanup } from '$lib/play/global-video';

export function armSourceScopedListeners(input: {
	video: HTMLVideoElement;
	showId: string;
	episode: number;
}): void {
	void input;
	void recoveryResume;
	void stallMachine;
	void replaceSourceScopedCleanup;
}
