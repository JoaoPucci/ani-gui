/**
 * Where the player stands between a stream attaching and frames
 * playing, read from the element's own events.
 *
 * The element says `paused` until an autoplaying stream has enough
 * data to play: from the attach to the first frame it reads exactly
 * as a viewer's pause does, and it fires no `waiting` (that event is
 * only for an element that is playing). So a page that reads
 * `paused` shows Play over a black frame for as long as the first
 * fragment takes, and nothing says the stream is on its way. The
 * phase here carries the viewer's intent instead: a stream attached
 * to an autoplaying element, or a play pressed, is `starting` until
 * the `playing` event; a `waiting` or a seek while playing is a
 * `stalled` that the next `playing` ends.
 */

export type StartPhase = 'idle' | 'starting' | 'playing' | 'stalled' | 'paused' | 'ended';

type Snapshot = { paused: boolean; readyState: number };

export type StartEvent =
	| { kind: 'attach'; autoplay: boolean }
	| { kind: 'play' }
	| { kind: 'playing' }
	| { kind: 'waiting' }
	| { kind: 'pause' }
	| { kind: 'ended' }
	| { kind: 'error' }
	| { kind: 'detach' }
	| ({ kind: 'seeking' | 'seeked' | 'canplay' } & Snapshot);

/** `HTMLMediaElement.HAVE_FUTURE_DATA`. */
const HAVE_FUTURE_DATA = 3;
/** `HTMLMediaElement.HAVE_ENOUGH_DATA`. */
const HAVE_ENOUGH_DATA = 4;

/** How long a start or a stall lasts before the loader shows: a start
 *  from the cache or a seek inside the buffer lands sooner and never
 *  flashes it. */
export const LOADER_DELAY_MS = 400;

export function nextStartPhase(phase: StartPhase, event: StartEvent): StartPhase {
	switch (event.kind) {
		case 'attach':
			return event.autoplay ? 'starting' : 'paused';
		case 'play':
			return phase === 'playing' ? 'playing' : 'starting';
		case 'playing':
			return 'playing';
		case 'waiting':
			return phase === 'playing' ? 'stalled' : phase;
		case 'pause':
			return 'paused';
		case 'ended':
			return 'ended';
		case 'error':
		case 'detach':
			return 'idle';
		case 'seeking':
			return phase === 'playing' && !event.paused ? 'stalled' : phase;
		case 'seeked':
			if (phase === 'stalled' && !event.paused && event.readyState >= HAVE_FUTURE_DATA) {
				return 'playing';
			}
			return phase;
		case 'canplay':
			// Autoplay refused: the data to play is in and nothing plays.
			if (phase === 'starting' && event.paused && event.readyState >= HAVE_ENOUGH_DATA) {
				return 'paused';
			}
			return phase;
	}
}

/** Whether the chrome shows the stream as playing (the pause glyph):
 *  the viewer asked for playback in a start and a stall as much as
 *  while frames play, so Play is never offered over a stream on its
 *  way. */
export function chromeReadsPlaying(phase: StartPhase): boolean {
	return phase === 'starting' || phase === 'playing' || phase === 'stalled';
}

/** Whether the loader is due `now`, for a phase entered at `since`. */
export function loaderDue(phase: StartPhase, since: number, now: number): boolean {
	if (phase !== 'starting' && phase !== 'stalled') return false;
	return now - since >= LOADER_DELAY_MS;
}
