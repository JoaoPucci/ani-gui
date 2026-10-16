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
import type { ErrorDetails, PlaylistLevelType } from 'hls.js';
import { bufferAheadSeconds, type BufferedRanges } from './buffer-ahead';
import { QualityGovernor, type QualityMove } from './quality-governor';
import { parseQualitySetting, type LadderLevel } from './quality-ladder';
import type { QualityView } from './quality-menu';

// hls.js's own names, spelled out so the engine double of a test that
// mocks the module still meets them, and typed against the engine's
// enums so a renamed one is a compile error.
const MAIN: `${PlaylistLevelType.MAIN}` = 'main';
const BUFFER_STALLED: `${ErrorDetails.BUFFER_STALLED_ERROR}` = 'bufferStalledError';
const levelFailures: `${ErrorDetails}`[] = [
	'levelLoadError',
	'levelLoadTimeOut',
	'levelParsingError',
	'levelEmptyError'
];
const fragmentFailures: `${ErrorDetails}`[] = [
	'fragLoadError',
	'fragLoadTimeOut',
	'fragParsingError'
];
/** A level's playlist failing. */
const LEVEL_FAILURES: ReadonlySet<string> = new Set(levelFailures);
/** A level's segments failing. */
const FRAGMENT_FAILURES: ReadonlySet<string> = new Set(fragmentFailures);

/** What a parsed init segment carries, as far as it is read: hls.js
 *  types the event's data as empty, and hands over the segment's
 *  fragment and the tracks the demuxer found, with the picture's size
 *  the stream's own SPS gave. */
interface InitSegment {
	frag?: { type?: string; level?: number };
	tracks?: Partial<Record<'video' | 'audiovideo', { metadata?: { height?: number } }>>;
}

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

/** The level the viewer pinned, by height, for the rest of the app's
 *  run: the next episode, and a stream recovered in its place, start
 *  at the same height — or the nearest below it — rather than at the
 *  setting's. "Auto" clears it. */
let pinnedHeight: number | null = null;

export const qualityPin = {
	get: (): number | null => pinnedHeight,
	set(height: number): void {
		pinnedHeight = height;
	},
	clear(): void {
		pinnedHeight = null;
	}
};

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
	const now = input.now ?? (() => performance.now());
	const log = input.log ?? ((message: string) => console.debug(`[play] quality: ${message}`));
	let governor: QualityGovernor | null = null;
	let playlist: LadderLevel[] = [];
	/** Heights the levels' segments are encoded at, once parsed. */
	const encoded = new Map<number, number>();
	let playing: number | null = null;

	const levels = (): LadderLevel[] =>
		playlist.map((l, i) => ({ height: encoded.get(i) ?? l.height, bitrate: l.bitrate }));
	const describe = (level: number): string => {
		const { height, bitrate } = levels()[level];
		const mbps = `${(bitrate / 1_000_000).toFixed(1)} Mbps`;
		return height > 0 ? `${height}p (level ${level}, ${mbps})` : `level ${level} (${mbps})`;
	};
	const publish = (g: QualityGovernor): void => {
		onView({
			levels: levels(),
			pinned: g.isPinned ? g.level : null,
			playing,
			unavailable: g.unavailableLevels
		});
	};
	const apply = (move: QualityMove | null): void => {
		if (!move) return;
		if (move.via === 'next') engine.nextLevel = move.level;
		else engine.loadLevel = move.level;
		const from = move.from === null ? '' : `${describe(move.from)} → `;
		log(`${move.reason}: ${from}${describe(move.level)}`);
	};

	engine.on(events.MANIFEST_PARSED, (_event, data) => {
		playlist = data.levels.map((l) => ({ height: l.height, bitrate: l.bitrate }));
		encoded.clear();
		playing = null;
		const setting = input.setting();
		const g = new QualityGovernor(playlist, parseQualitySetting(setting), qualityPin.get());
		governor = g;
		const start = g.start();
		if (start) {
			engine.startLevel = start.level;
			apply(start);
			log(`setting ${setting ?? 'best'}, ${playlist.length} levels`);
		} else if (playlist.length === 1) {
			log(`one level, ${describe(0)}: nothing to choose`);
		}
		publish(g);
	});

	engine.on(events.ERROR, (_event, data) => {
		if (!governor) return;
		const details: string = data.details;
		const engaged = governor.isEngaged;
		const verdict = data.fatal || data.levelRetry === true;
		if (details === BUFFER_STALLED && !data.fatal) {
			if (!video.seeking) apply(governor.stall(now()));
		} else if (LEVEL_FAILURES.has(details) && verdict) {
			const level = data.level ?? data.context?.level;
			if (typeof level === 'number') apply(governor.levelFailed(level));
		} else if (FRAGMENT_FAILURES.has(details) && verdict && String(data.frag?.type) === MAIN) {
			const level = data.frag?.level;
			if (typeof level === 'number') apply(governor.fragmentsFailed(level, now()));
		}
		// hls.js answers a level's error by handing the choice back to
		// its own estimate before any listener runs; the governor takes
		// it back, unless there is nothing left for it to hold.
		if (governor.isEngaged && engine.autoLevelEnabled) {
			engine.loadLevel = governor.level;
			log(`held at ${describe(governor.level)} after ${details}`);
		}
		if (engaged && !governor.isEngaged) {
			log(`released to the engine after ${details}: no level left below the one failing`);
		}
		publish(governor);
	});

	engine.on(events.FRAG_BUFFERED, (_event, data) => {
		if (!governor || String(data.frag.type) !== MAIN) return;
		const move = governor.healthy(now(), bufferAheadSeconds(video.buffered, video.currentTime));
		apply(move);
		if (move) publish(governor);
	});

	engine.on(events.FRAG_CHANGED, (_event, data) => {
		const level = data.frag.level;
		if (!governor || String(data.frag.type) !== MAIN || level === playing) return;
		playing = level;
		log(`playing ${describe(level)}`);
		publish(governor);
	});

	engine.on(events.FRAG_PARSING_INIT_SEGMENT, (_event, data) => {
		const { frag, tracks } = data as InitSegment;
		const level = frag?.level;
		if (!governor || String(frag?.type) !== MAIN || level === undefined) return;
		const height = tracks?.video?.metadata?.height ?? tracks?.audiovideo?.metadata?.height;
		const listed = playlist[level]?.height;
		if (!height || listed === undefined || encoded.get(level) === height) return;
		encoded.set(level, height);
		if (height !== listed) log(`level ${level} encodes ${height}p; its playlist says ${listed}p`);
		publish(governor);
	});

	const onSeeking = () => governor?.seeking(now());
	video.addEventListener('seeking', onSeeking);

	return {
		pick(choice) {
			if (!governor) return;
			const move = governor.pick(choice, now());
			if (choice === 'auto') {
				qualityPin.clear();
				if (!move) log('auto');
			} else if (governor.isPinned && governor.level === choice) {
				const height = levels()[choice].height;
				if (height > 0) qualityPin.set(height);
				if (!move) log(`pinned ${describe(choice)}`);
			}
			apply(move);
			publish(governor);
		},
		dispose() {
			video.removeEventListener('seeking', onSeeking);
			governor = null;
			onView(null);
		}
	};
}
