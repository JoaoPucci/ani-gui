/**
 * Follows a stream's start on the page's video element: the phase
 * `$lib/play/start-state` reads from the element's own events, told
 * to the page whenever it changes.
 *
 * The page tells it about the one thing the element does not announce:
 * a stream attaching, which unloads the last one with `load()` — that
 * sets `paused` without a `pause` event. And `cancel` stops a start:
 * `pause()` on an element still waiting to autoplay cancels the
 * autoplay, but the element was already paused, so it fires nothing.
 */

import { nextStartPhase, type StartEvent, type StartPhase } from './start-state';

export interface StartWatch {
	phase(): StartPhase;
	/** A stream attached; `autoplay` is the element's own. */
	attach(autoplay: boolean): void;
	/** The viewer stopped a start before its first frame. */
	cancel(): void;
	dispose(): void;
}

/** Plain events, and the ones read with the element's state. */
const PLAIN = ['play', 'playing', 'waiting', 'pause', 'ended', 'error'] as const;
const WITH_STATE: [string, 'seeking' | 'seeked' | 'canplay'][] = [
	['seeking', 'seeking'],
	['seeked', 'seeked'],
	['canplay', 'canplay'],
	['canplaythrough', 'canplay']
];

export function watchStart(
	video: HTMLVideoElement,
	onPhase: (phase: StartPhase) => void,
	initial: StartPhase = 'idle'
): StartWatch {
	let phase = initial;
	const step = (event: StartEvent) => {
		const next = nextStartPhase(phase, event);
		if (next === phase) return;
		phase = next;
		onPhase(next);
	};
	const listeners: [string, () => void][] = [
		...PLAIN.map((kind): [string, () => void] => [kind, () => step({ kind })]),
		...WITH_STATE.map(([name, kind]): [string, () => void] => [
			name,
			() => step({ kind, paused: video.paused, readyState: video.readyState })
		])
	];
	for (const [name, fn] of listeners) video.addEventListener(name, fn);
	return {
		phase: () => phase,
		attach: (autoplay) => step({ kind: 'attach', autoplay }),
		cancel: () => {
			video.pause();
			step({ kind: 'pause' });
		},
		dispose: () => {
			for (const [name, fn] of listeners) video.removeEventListener(name, fn);
		}
	};
}
