/**
 * The player's quality control, wired to one engine: the governor
 * (`quality-governor.ts`) decides which level loads, and this hands it
 * what the engine reports — the master's levels, a stall, a level or
 * its segments failing, a segment buffered, the segment playing —
 * and makes the engine do what it decides. Every change, and every
 * change of the level that plays, is logged at debug level, so which
 * level a viewer was watching can be read off the log afterwards.
 *
 * The play page arms one per HLS stream it attaches and disposes of it
 * with the stream.
 */

import type Hls from 'hls.js';
import type { BufferedRanges } from './buffer-ahead';
import { bufferAheadSeconds } from './buffer-ahead';
import { QualityGovernor, type QualityMove } from './quality-governor';
import { parseQualitySetting } from './quality-ladder';
import type { QualityView } from './quality-menu';
import { qualityPin, rememberPick } from './quality-pin';
import {
	consoleQualityLog,
	healthyMove,
	pageNow,
	reactTo,
	readError,
	readInitSegment
} from './quality-report';
import { QualityTrack } from './quality-track';

export { qualityPin } from './quality-pin';

/** The engine events the control listens to, as the page's engine
 *  names them. */
export type QualityEvents = Pick<
	typeof Hls.Events,
	'MANIFEST_PARSED' | 'ERROR' | 'FRAG_BUFFERED' | 'FRAG_CHANGED' | 'FRAG_PARSING_INIT_SEGMENT'
>;

/** As much of the video element as the control reads. */
export interface QualityVideo extends EventTarget {
	readonly buffered: BufferedRanges;
	readonly currentTime: number;
	readonly seeking: boolean;
}

export interface QualityControl {
	/** The viewer chose a level, or "Auto", from the menu. */
	pick(choice: 'auto' | number): void;
	/** The stream retired. */
	dispose(): void;
}

export function armQuality(input: {
	engine: Hls;
	events: QualityEvents;
	video: QualityVideo;
	/** The quality setting, read when the master's levels arrive. */
	setting: () => string | null | undefined;
	/** Told what the menu shows whenever it changes; null once the
	 *  stream retires. */
	onView: (view: QualityView | null) => void;
	now?: () => number;
	log?: (message: string) => void;
}): QualityControl {
	const { engine, events, video, onView } = input;
	const now = input.now ?? pageNow;
	const log = input.log ?? consoleQualityLog;
	const track = new QualityTrack();
	let governor: QualityGovernor | null = null;

	const apply = (move: QualityMove | null): void => {
		if (!move) return;
		if (move.via === 'next') engine.nextLevel = move.level;
		else engine.loadLevel = move.level;
		log(track.describeMove(move));
	};
	const publish = (g: QualityGovernor): void => onView(track.view(g));

	engine.on(events.MANIFEST_PARSED, (_event, data) => {
		track.arrive(data.levels);
		const setting = input.setting();
		const g = new QualityGovernor(
			track.levels(),
			parseQualitySetting(setting),
			qualityPin.choice()
		);
		governor = g;
		const start = g.start();
		if (start) {
			engine.startLevel = start.level;
			apply(start);
			log(`setting ${setting ?? 'best'}, ${data.levels.length} levels`);
		} else if (data.levels.length === 1) {
			log(`one level, ${track.describe(0)}: nothing to choose`);
		}
		publish(g);
	});

	engine.on(events.ERROR, (_event, data) => {
		const g = governor;
		if (!g) return;
		const engaged = g.isEngaged;
		apply(reactTo(g, readError(data, video.seeking), now()));
		// hls.js answers a level's error by handing the choice back to
		// its own estimate before any listener runs; the governor takes
		// it back, unless there is nothing left for it to hold.
		const held = g.reclaim(engine.autoLevelEnabled);
		if (held !== null) {
			engine.loadLevel = held;
			log(`held at ${track.describe(held)} after ${data.details}`);
		}
		if (engaged !== g.isEngaged) {
			log(`released to the engine after ${data.details}: no level left below the one failing`);
		}
		publish(g);
	});

	engine.on(events.FRAG_BUFFERED, (_event, data) => {
		const g = governor;
		if (!g) return;
		const ahead = bufferAheadSeconds(video.buffered, video.currentTime);
		const move = healthyMove(g, data.frag, now(), ahead);
		apply(move);
		if (move) publish(g);
	});

	engine.on(events.FRAG_CHANGED, (_event, data) => {
		if (!governor || !track.notePlaying(data.frag)) return;
		log(`playing ${track.describe(data.frag.level)}`);
		publish(governor);
	});

	engine.on(events.FRAG_PARSING_INIT_SEGMENT, (_event, data) => {
		const g = governor;
		if (!g) return;
		const news = track.noteEncoded(readInitSegment(data));
		if (!news) return;
		if (news.listed > 0 && news.listed !== news.height) {
			log(`level ${news.level} encodes ${news.height}p; its playlist says ${news.listed}p`);
		}
		publish(g);
	});

	const onSeeking = () => governor?.seeking(now());
	video.addEventListener('seeking', onSeeking);

	return {
		pick(choice) {
			const g = governor;
			if (!g) return;
			const move = g.pick(choice, now());
			const kept = rememberPick(choice, g, track);
			if (!move && kept) log(kept);
			apply(move);
			publish(g);
		},
		dispose() {
			video.removeEventListener('seeking', onSeeking);
			governor = null;
			onView(null);
		}
	};
}
