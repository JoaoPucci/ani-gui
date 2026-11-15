/**
 * What the captions sync ({@link import('./caption-sync')}) watches the
 * engine for: whether its playlist has named its renditions yet, and
 * the subtitle fragment on its way, whose rendition must not be left
 * disabled until it is in.
 */

import type Hls from 'hls.js';
import type { CaptionSyncEvents } from './caption-sync-tracks';

export interface Readiness {
	/** Run `fn` once the engine's playlist is parsed — at once if it is
	 *  already. The returned function cancels it. */
	whenReady(fn: () => void): () => void;
	/** Drop every `fn` still waiting. */
	clear(): void;
}

/** Whether the engine's playlist is parsed: set as it is, unset as a
 *  manifest starts loading again. */
export function watchReadiness(engine: Hls, events: CaptionSyncEvents): Readiness {
	let ready = false;
	let waiting: (() => void)[] = [];
	engine.on(events.MANIFEST_LOADING, () => {
		ready = false;
	});
	engine.on(events.MANIFEST_PARSED, () => {
		ready = true;
		const run = waiting;
		waiting = [];
		for (const fn of run) fn();
	});
	return {
		whenReady(fn) {
			if (ready) {
				fn();
				return () => {};
			}
			waiting.push(fn);
			return () => {
				waiting = waiting.filter((w) => w !== fn);
			};
		},
		clear() {
			waiting = [];
		}
	};
}

export interface FragmentWatch {
	/** The rendition whose fragment is on its way, or null. */
	level(): number | null;
}

/**
 * The subtitle fragment on its way, if any. The engine loads one at a
 * time, and starts the next as it finishes one, before this hears that
 * the one before is in; so a fragment is matched by its rendition and
 * its number. `landed` runs when the one on its way is in or failed.
 * A seek aborts the one on its way with no event, which its loader's
 * stats record: an aborted fragment is on its way no longer.
 */
export function watchFragments(
	engine: Hls,
	events: CaptionSyncEvents,
	landed: () => void
): FragmentWatch {
	let inFlight: { level: number; sn: unknown; stats?: { aborted?: boolean } } | null = null;
	const isSubtitle = (frag: { type: unknown }) => `${frag.type}` === 'subtitle';
	const done = (frag: { level: number; sn: unknown }): void => {
		if (!inFlight || frag.level !== inFlight.level || frag.sn !== inFlight.sn) return;
		inFlight = null;
		landed();
	};
	// A new subtitle group renumbers the renditions: a fragment of the
	// group left names none of the new one.
	const forget = () => {
		inFlight = null;
	};
	engine.on(events.MANIFEST_LOADING, forget);
	engine.on(events.SUBTITLE_TRACKS_UPDATED, forget);
	engine.on(events.FRAG_LOADING, (_event, data) => {
		if (isSubtitle(data.frag)) inFlight = data.frag;
	});
	engine.on(events.SUBTITLE_FRAG_PROCESSED, (_event, data) => done(data.frag));
	engine.on(events.ERROR, (_event, data) => {
		if (data.frag && isSubtitle(data.frag)) done(data.frag);
	});
	return {
		level: () => (inFlight && !inFlight.stats?.aborted ? inFlight.level : null)
	};
}
