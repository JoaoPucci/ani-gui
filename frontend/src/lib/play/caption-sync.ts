/**
 * hls.js's subtitle selection wired to the page's video and one engine,
 * by the rules of {@link import('./caption-engine')}.
 */

import type Hls from 'hls.js';
import { enginePlan } from './caption-engine';
import { applyCaptionPick } from './caption-pick';
import {
	indexIn,
	renditionIndex,
	wantOf,
	type CaptionSync,
	type CaptionSyncEvents,
	type CaptionVideo,
	type Desired
} from './caption-sync-tracks';
import { watchFragments, watchReadiness } from './caption-sync-watch';

/** Keep `engine`'s subtitle selection in step with the captions shown
 *  on `video`. */
export function createCaptionSync(
	video: CaptionVideo,
	engine: Hls,
	events: CaptionSyncEvents
): CaptionSync {
	let desired: Desired | null = null;
	let applying = false;
	let disposed = false;
	const readiness = watchReadiness(engine, events);
	const fragments = watchFragments(engine, events, () => apply());

	/** `force`: set the engine's subtitle track even where it is the
	 *  plan's already, which also turns off the engine's own pick of its
	 *  playlist's default once the playlist's group arrives. */
	const apply = (force = false): void => {
		if (disposed || desired === null) return;
		const list = video.textTracks;
		const sidecars = new Set(Array.from(video.querySelectorAll('track'), (el) => el.track));
		const renditions = engine.subtitleTracks;
		const at = typeof desired === 'string' ? -1 : indexIn(list, desired);
		const plan = enginePlan(wantOf(desired, at, sidecars, renditions), fragments.level());
		applying = true;
		try {
			if (force || engine.subtitleTrack !== plan.subtitleTrack)
				engine.subtitleTrack = plan.subtitleTrack;
			if (engine.subtitleDisplay !== plan.subtitleDisplay)
				engine.subtitleDisplay = plan.subtitleDisplay;
		} finally {
			applying = false;
		}
		const hidden =
			plan.hidden === null ? -1 : renditionIndex(list, renditions[plan.hidden], sidecars);
		applyCaptionPick(list, at === -1 ? 'off' : at, hidden === -1 ? undefined : hidden);
	};

	const show = (next: Desired, force = false): void => {
		desired = next;
		apply(force);
	};

	// The engine's own switch is brought back to the app's pick inside
	// the event, so no `change` event ever reports it.
	engine.on(events.SUBTITLE_TRACK_SWITCH, () => {
		if (!applying) apply();
	});

	return {
		pick(target) {
			show(target === 'off' ? 'off' : (video.textTracks[target] ?? 'off'));
		},
		pickTrack(track) {
			show(track);
		},
		// The engine's own default pick, made later, would switch the
		// tracks against the start before it could be brought back.
		beforeOwnTracks(start) {
			show(start, true);
		},
		release() {
			desired = null;
		},
		whenReady: (fn) => readiness.whenReady(fn),
		renditions() {
			return engine.allSubtitleTracks;
		},
		dispose() {
			disposed = true;
			desired = null;
			readiness.clear();
		}
	};
}
