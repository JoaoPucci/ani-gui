/**
 * Follows a stream's start on the page's video element. The signature
 * only, so far: the behaviour comes with its tests.
 */

import type { StartPhase } from './start-state';

export interface StartWatch {
	phase(): StartPhase;
	/** A stream attached; `autoplay` is the element's own. */
	attach(autoplay: boolean): void;
	/** The viewer stopped a start before its first frame. */
	cancel(): void;
	dispose(): void;
}

export function watchStart(
	_video: HTMLVideoElement,
	_onPhase: (phase: StartPhase) => void,
	initial: StartPhase = 'idle'
): StartWatch {
	return {
		phase: () => initial,
		attach: () => {},
		cancel: () => {},
		dispose: () => {}
	};
}
