import { afterEach, describe, expect, it, vi } from 'vitest';
import Hls from 'hls.js';
import { m } from '$lib/paraglide/messages';
import { armQuality, qualityPin } from './arm-quality';
import {
	HEALTHY_BUFFER_S,
	HEALTHY_WINDOWS_MS,
	SEEK_GRACE_MS,
	STEP_DOWN_COOLDOWN_MS
} from './quality-governor';
import { qualityMenu, type QualityView } from './quality-menu';

const E = Hls.Events;
const D = Hls.ErrorDetails;

// What hls.js's error controller decided, as the error carries it:
// another request of the same thing scheduled, or the level given up
// on and the engine switched to `nextAutoLevel`. A playlist retry
// still carries `levelRetry: true` — the level controller sets it to
// tell other controllers a retry is happening — so that flag is no
// verdict.
const RETRY = { action: 5 };
const SWITCHED = (nextAutoLevel: number) => ({ action: 2, nextAutoLevel, flags: 0 });

// The engine double: the levels, the three ways of setting one, and
// whether the engine has taken the choice back — which hls.js does
// on its own after a level's error, before any listener runs.
class FakeEngine {
	levels: { height: number; bitrate: number }[] = [];
	startLevel = -1;
	manual = -1;
	loads: number[] = [];
	nexts: number[] = [];
	starts = 0;
	private handlers = new Map<string, ((event: string, data: unknown) => void)[]>();
	get loadLevel() {
		return this.manual;
	}
	set loadLevel(level: number) {
		this.manual = level;
		this.loads.push(level);
	}
	get nextLevel() {
		return this.manual;
	}
	set nextLevel(level: number) {
		this.manual = level;
		this.nexts.push(level);
	}
	get autoLevelEnabled() {
		return this.manual === -1;
	}
	startLoad() {
		this.starts += 1;
	}
	on(event: string, handler: (event: string, data: unknown) => void) {
		this.handlers.set(event, [...(this.handlers.get(event) ?? []), handler]);
	}
	emit(event: string, data: unknown) {
		for (const h of this.handlers.get(event) ?? []) h(event, data);
	}
}

class FakeVideo extends EventTarget {
	currentTime = 0;
	seeking = false;
	ahead = 0;
	get buffered() {
		const end = this.currentTime + this.ahead;
		return { length: this.ahead > 0 ? 1 : 0, start: () => this.currentTime, end: () => end };
	}
}

const MASTER = [
	{ height: 360, bitrate: 800_000 },
	{ height: 720, bitrate: 2_500_000 },
	{ height: 1080, bitrate: 5_000_000 }
];

function arm(setting = 'best', levels = MASTER) {
	const engine = new FakeEngine();
	const video = new FakeVideo();
	const logs: string[] = [];
	const views: (QualityView | null)[] = [];
	let now = 1_000;
	let settingReads = 0;
	const control = armQuality({
		engine: engine as unknown as Hls,
		events: E,
		video,
		setting: () => {
			settingReads += 1;
			return setting;
		},
		onView: (v) => views.push(v),
		now: () => now,
		log: (msg) => logs.push(msg)
	});
	const parse = () => {
		engine.levels = levels;
		engine.emit(E.MANIFEST_PARSED, { levels });
	};
	return {
		engine,
		video,
		logs,
		views,
		control,
		parse,
		settingReads: () => settingReads,
		at: (t: number) => {
			now = t;
		},
		lastView: () => views[views.length - 1]
	};
}

afterEach(() => qualityPin.clear());

describe('armQuality — the start', () => {
	it('best starts the engine at the highest level, read when the levels arrive', () => {
		const s = arm('best');
		expect(s.settingReads()).toBe(0);
		s.parse();
		expect(s.settingReads()).toBe(1);
		expect(s.engine.startLevel).toBe(2);
		expect(s.engine.loads).toEqual([2]);
		expect(s.engine.autoLevelEnabled).toBe(false);
		expect(s.logs.join('\n')).toMatch(/start.*1080p/);
		expect(s.lastView()).toEqual({
			levels: MASTER,
			pinned: null,
			playing: null,
			unavailable: []
		});
	});

	it('worst starts it at the lowest', () => {
		const s = arm('worst');
		s.parse();
		expect(s.engine.loads).toEqual([0]);
	});

	it('leaves a stream of one level to the engine, and says so', () => {
		const s = arm('best', [MASTER[1]]);
		s.parse();
		expect(s.engine.loads).toEqual([]);
		expect(s.engine.autoLevelEnabled).toBe(true);
		expect(s.logs.join('\n')).toMatch(/one level.*720p/);
	});

	it('starts at the height pinned earlier in the run', () => {
		qualityPin.set(720);
		const s = arm('best');
		s.parse();
		expect(s.engine.loads).toEqual([1]);
		expect(s.lastView()?.pinned).toBe(1);
	});

	it('ignores what the engine reports before its levels arrive', () => {
		const s = arm();
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 0 } });
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'main', level: 0 } });
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, { frag: { type: 'main', level: 0 } });
		s.control.pick(1);
		expect(s.engine.loads).toEqual([]);
		expect(s.engine.nexts).toEqual([]);
		expect(s.views).toEqual([]);
	});
});

describe('armQuality — trouble', () => {
	it('a stall steps the engine down and logs it', () => {
		const s = arm();
		s.parse();
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		expect(s.engine.loads).toEqual([2, 1]);
		expect(s.logs.join('\n')).toMatch(/stall.*1080p.*720p/);
	});

	it('a stall while the element seeks, or just after, is not trouble', () => {
		const s = arm();
		s.parse();
		s.at(10_000);
		s.video.seeking = true;
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		s.video.seeking = false;
		s.video.dispatchEvent(new Event('seeking'));
		s.at(10_000 + SEEK_GRACE_MS - 1);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		expect(s.engine.loads).toEqual([2]);
	});

	it('a level whose playlist the engine gave up on is left, and the menu loses it', () => {
		const s = arm();
		s.parse();
		// A retry still pending is not a verdict, though the engine
		// flags it as a level retry.
		s.engine.emit(E.ERROR, {
			details: D.LEVEL_LOAD_ERROR,
			fatal: false,
			level: 2,
			levelRetry: true,
			errorAction: RETRY
		});
		expect(s.engine.loads).toEqual([2]);
		expect(s.lastView()?.unavailable).toEqual([]);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.LEVEL_LOAD_TIMEOUT,
			fatal: false,
			levelRetry: true,
			errorAction: SWITCHED(1),
			context: { level: 2 }
		});
		expect(s.engine.loads).toEqual([2, 1]);
		expect(s.lastView()?.unavailable).toEqual([2]);
		expect(s.logs.join('\n')).toMatch(/level-failed.*1080p.*720p/);
	});

	it("a level error naming no level only takes the choice back from the engine's fallback", () => {
		const s = arm();
		s.parse();
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, { details: D.LEVEL_PARSING_ERROR, fatal: true });
		expect(s.engine.loads).toEqual([2, 2]);
	});

	it('segments the engine gave up on step down from that level', () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_TIMEOUT,
			fatal: false,
			errorAction: RETRY,
			frag: { type: 'main', level: 2 }
		});
		expect(s.engine.loads).toEqual([2]);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_TIMEOUT,
			fatal: false,
			levelRetry: true,
			errorAction: SWITCHED(1),
			frag: { type: 'main', level: 2 }
		});
		expect(s.engine.loads).toEqual([2, 1]);
		expect(s.engine.autoLevelEnabled).toBe(false);
	});

	it('any error the engine switched levels for steps down rather than reclaiming the level it left', () => {
		// An append or codec error on the level held: hls.js offers the
		// next level with no load errors each time; putting it back on
		// the failing level would fetch the same segment again and again.
		const s = arm();
		s.parse();
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_APPEND_ERROR,
			fatal: false,
			errorAction: SWITCHED(1),
			frag: { type: 'main', level: 2 }
		});
		expect(s.engine.loads).toEqual([2, 1]);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_ADD_CODEC_ERROR,
			fatal: false,
			errorAction: SWITCHED(0)
		});
		expect(s.engine.loads).toEqual([2, 1, 0]);
		expect(s.logs.filter((l) => l.startsWith('fragments-failed'))).toHaveLength(2);
	});

	it('an audio segment hls.js switched levels for follows the engine without counting against the video', () => {
		// Levels with their own audio groups: hls.js leaves a level whose
		// audio fails for one whose audio does. That is no stall of the
		// video and no failure of its level — the governor holds the level
		// the engine moved to, rather than stepping down on its own or
		// putting the engine back on the broken audio.
		const s = arm();
		s.parse();
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(1),
			frag: { type: 'audio', level: 0 }
		});
		expect(s.engine.loads).toEqual([2, 1]);
		expect(s.logs.join('\n')).toMatch(/followed: 1080p.*720p/);
		expect(s.logs.join('\n')).not.toMatch(/fragments-failed|held at/);
		// The level left over its audio is left for the stream.
		expect(s.lastView()?.unavailable).toEqual([2]);
		// Not a step down: healthy playback has nothing to climb back from.
		s.video.ahead = HEALTHY_BUFFER_S + 1;
		s.at(10_000_000);
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 1 } });
		expect(s.engine.loads).toEqual([2, 1]);
	});

	it('a level left over its audio is left for the stream, so a later climb never returns to it', () => {
		const s = arm();
		s.parse();
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		expect(s.engine.loads).toEqual([2, 1]);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(0),
			frag: { type: 'audio', level: 1 }
		});
		expect(s.engine.loads).toEqual([2, 1, 0]);
		expect(s.lastView()?.unavailable).toEqual([1]);
		s.video.ahead = HEALTHY_BUFFER_S + 1;
		for (const t of [1e6, 2e6, 3e6]) {
			s.at(t);
			s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 0 } });
		}
		expect(s.engine.loads.slice(3)).not.toContain(1);
	});

	it('an audio or subtitle playlist, or an audio buffer, failing is a side rendition the video follows', () => {
		for (const failure of [
			{ details: D.AUDIO_TRACK_LOAD_ERROR, context: { type: 'audioTrack' } },
			{ details: D.SUBTITLE_TRACK_LOAD_TIMEOUT, context: { type: 'subtitleTrack' } },
			{ details: D.BUFFER_ADD_CODEC_ERROR, sourceBufferName: 'audio' }
		]) {
			const s = arm();
			s.parse();
			s.engine.manual = -1;
			s.engine.emit(E.ERROR, { ...failure, fatal: false, errorAction: SWITCHED(0) });
			expect(s.engine.loads).toEqual([2, 0]);
			expect(s.logs.join('\n')).toMatch(/followed/);
			expect(s.logs.join('\n')).not.toMatch(/fragments-failed/);
		}
	});

	it('an audio segment switch to a level the engine cannot use, or to the level held, follows nothing', () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.ERROR, { details: D.LEVEL_LOAD_ERROR, fatal: true, level: 1 });
		const loads = [...s.engine.loads];
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(1),
			frag: { type: 'audio', level: 0 }
		});
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(s.engine.manual),
			frag: { type: 'subtitle', level: 0 }
		});
		expect(s.engine.loads).toEqual(loads);
	});

	it('a switched append error with no segment is a failure of the level it names', () => {
		const s = arm();
		s.parse();
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_APPEND_ERROR,
			fatal: false,
			level: 1,
			errorAction: SWITCHED(0)
		});
		// Level 1 is not the one held: the held level is put back.
		expect(s.engine.loads).toEqual([2, 2]);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_APPEND_ERROR,
			fatal: false,
			level: 2,
			errorAction: SWITCHED(1)
		});
		expect(s.engine.loads).toEqual([2, 2, 1]);
	});

	it("an audio rendition's segments failing do not move the video level", () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: true,
			frag: { type: 'audio', level: 0 }
		});
		expect(s.engine.loads).toEqual([2]);
	});

	it('takes the choice back whenever the engine took it after an error', () => {
		const s = arm();
		s.parse();
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, { details: D.FRAG_LOAD_ERROR, fatal: false, frag: { type: 'main' } });
		expect(s.engine.loads).toEqual([2, 2]);
		expect(s.logs.join('\n')).toMatch(/held at 1080p/);
	});

	it('hands the choice to the engine for good once nothing below a failing level is left', () => {
		const s = arm('worst');
		s.parse();
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(1),
			frag: { type: 'main', level: 0 }
		});
		expect(s.engine.loads).toEqual([0]);
		expect(s.engine.autoLevelEnabled).toBe(true);
		expect(s.logs.join('\n')).toMatch(/released/);
		s.engine.emit(E.ERROR, { details: D.FRAG_LOAD_ERROR, fatal: false, frag: { type: 'main' } });
		expect(s.engine.loads).toEqual([0]);
	});
});

describe('armQuality — recovery and what plays', () => {
	it('steps back up after a healthy window with buffer in hand', () => {
		const s = arm();
		s.parse();
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		s.video.ahead = HEALTHY_BUFFER_S + 1;
		s.at(10_000 + HEALTHY_WINDOWS_MS[0]);
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'audio', level: 0 } });
		expect(s.engine.loads).toEqual([2, 1]);
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 1 } });
		expect(s.engine.loads).toEqual([2, 1, 2]);
		expect(s.logs.join('\n')).toMatch(/healthy.*720p.*1080p/);
	});

	it('logs and publishes the level that plays, once per change', () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'main', level: 2 } });
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'main', level: 2 } });
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'audio', level: 0 } });
		expect(s.logs.filter((l) => l.startsWith('playing'))).toEqual([
			'playing 1080p (level 2, 5.0 Mbps)'
		]);
		expect(s.lastView()?.playing).toBe(2);
	});

	it('names a level by the height its segments are encoded at', () => {
		const s = arm();
		s.parse();
		const init = (level: number, height?: number, key = 'video') =>
			s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
				frag: { type: 'main', level },
				tracks: height === undefined ? {} : { [key]: { metadata: { width: 1, height } } }
			});
		init(0, 480);
		expect(s.lastView()?.levels[0]).toEqual({ height: 480, bitrate: 800_000 });
		expect(s.logs.join('\n')).toMatch(/level 0 encodes 480p; its playlist says 360p/);
		const published = s.views.length;
		init(0, 480);
		init(1);
		init(7, 720);
		init(2, 1080, 'audiovideo');
		expect(s.views.length).toBe(published + 1);
		expect(s.logs.filter((l) => l.includes('encodes'))).toHaveLength(1);
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, { frag: { type: 'audio', level: 0 }, tracks: {} });
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {});
		expect(s.views.length).toBe(published + 1);
	});
});

describe('armQuality — the viewer chooses', () => {
	it('a pick switches at once, is pinned, and is kept for the run', () => {
		const s = arm();
		s.parse();
		s.control.pick(1);
		expect(s.engine.nexts).toEqual([1]);
		expect(s.lastView()?.pinned).toBe(1);
		expect(qualityPin.get()).toBe(720);
		expect(s.logs.join('\n')).toMatch(/pick.*720p/);
		s.control.pick('auto');
		expect(s.engine.nexts).toEqual([1, 2]);
		expect(s.lastView()?.pinned).toBeNull();
		expect(qualityPin.get()).toBeNull();
	});

	it('pinning the level held while a lower one still plays switches at once', () => {
		// Auto climbed with loadLevel: 720p segments still play from the
		// buffer while 1080p is held. Picking 1080p asks for it now.
		const s = arm();
		s.parse();
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		s.video.ahead = HEALTHY_BUFFER_S + 1;
		s.at(10_000 + HEALTHY_WINDOWS_MS[0]);
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 1 } });
		expect(s.engine.loads).toEqual([2, 1, 2]);
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'main', level: 1 } });
		s.control.pick(2);
		expect(s.engine.nexts).toEqual([2]);
		expect(s.lastView()?.pinned).toBe(2);
		expect(qualityPin.get()).toBe(1080);
	});

	it('choosing Auto while a lower level than the one held plays switches at once', () => {
		const s = arm();
		s.parse();
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		s.video.ahead = HEALTHY_BUFFER_S + 1;
		s.at(10_000 + HEALTHY_WINDOWS_MS[0]);
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 1 } });
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'main', level: 1 } });
		s.control.pick('auto');
		expect(s.engine.nexts).toEqual([2]);
	});

	it('a pick of the level already playing pins it without a switch', () => {
		const s = arm();
		s.parse();
		s.control.pick(2);
		expect(s.engine.nexts).toEqual([]);
		expect(s.lastView()?.pinned).toBe(2);
		expect(qualityPin.get()).toBe(1080);
		expect(s.logs.join('\n')).toMatch(/pinned 1080p/);
	});

	it('the pin is the height the playlist lists for the level, which the next stream is matched by', () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
			frag: { type: 'main', level: 0 },
			tracks: { video: { metadata: { width: 854, height: 480 } } }
		});
		s.control.pick(0);
		expect(qualityPin.get()).toBe(360);
	});

	it('a level whose playlist lists no height is pinned by its bandwidth rank from the top', () => {
		const unnamed = [
			{ height: 0, bitrate: 2_000_000 },
			{ height: 0, bitrate: 800_000 },
			{ height: 0, bitrate: 5_000_000 }
		];
		const s = arm('best', unnamed);
		s.parse();
		s.control.pick(0);
		expect(qualityPin.get()).toBeNull();
		expect(qualityPin.choice()).toEqual({ rank: 1 });
		// The next stream starts at the level of that rank, pinned.
		const next = arm('best', unnamed);
		next.parse();
		expect(next.engine.loads).toEqual([0]);
		expect(next.lastView()?.pinned).toBe(0);
		// A height picked later replaces the rank.
		const named = arm();
		named.parse();
		named.control.pick(1);
		expect(qualityPin.choice()).toBe(720);
	});

	it('a refused pick changes nothing', () => {
		const t = arm();
		t.parse();
		t.control.pick(9);
		expect(t.engine.nexts).toEqual([]);
		expect(qualityPin.get()).toBeNull();
		expect(t.lastView()?.pinned).toBeNull();
	});

	it('the stream retiring clears the menu and stops listening to the element', () => {
		const s = arm();
		s.parse();
		s.control.dispose();
		expect(s.lastView()).toBeNull();
		s.video.dispatchEvent(new Event('seeking'));
		s.at(STEP_DOWN_COOLDOWN_MS * 10);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		expect(s.engine.loads).toEqual([2]);
	});
});

describe('armQuality — the edges', () => {
	it('logs to the console at debug level by default', () => {
		const debug = vi.spyOn(console, 'debug').mockImplementation(() => {});
		try {
			const engine = new FakeEngine();
			armQuality({
				engine: engine as unknown as Hls,
				events: E,
				video: new FakeVideo(),
				setting: () => undefined,
				onView: () => {}
			});
			engine.emit(E.MANIFEST_PARSED, { levels: MASTER });
			expect(engine.loads).toEqual([2]);
			expect(debug).toHaveBeenCalledWith(expect.stringMatching(/^\[play\] quality: start: 1080p/));
			expect(debug).toHaveBeenCalledWith('[play] quality: setting best, 3 levels');
			// The clock is the page's own when none is given.
			engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
			expect(engine.loads).toEqual([2, 1]);
		} finally {
			debug.mockRestore();
		}
	});

	it('a level whose playlist lists no height learns one without claiming the playlist said 0p', () => {
		const s = arm('best', [
			{ height: 0, bitrate: 800_000 },
			{ height: 0, bitrate: 2_000_000 }
		]);
		s.parse();
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
			frag: { type: 'main', level: 1 },
			tracks: { video: { metadata: { width: 1920, height: 1080 } } }
		});
		expect(s.lastView()?.levels[1]).toEqual({ height: 1080, bitrate: 2_000_000 });
		expect(s.logs.join('\n')).not.toMatch(/says 0p/);
	});

	it('a manifest of no levels sets nothing', () => {
		const s = arm('best', []);
		s.parse();
		expect(s.engine.loads).toEqual([]);
		expect(s.logs).toEqual([]);
		expect(s.lastView()?.levels).toEqual([]);
	});

	it('a fatal playlist failure naming its level leaves that level', () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.ERROR, { details: D.LEVEL_EMPTY_ERROR, fatal: true, level: 2 });
		expect(s.engine.loads).toEqual([2, 1]);
		expect(s.lastView()?.unavailable).toEqual([2]);
	});

	it('a segment failure naming no level moves nothing', () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.ERROR, { details: D.FRAG_PARSING_ERROR, fatal: true, frag: { type: 'main' } });
		expect(s.engine.loads).toEqual([2]);
	});

	it('a segment buffered while healthy publishes nothing new', () => {
		const s = arm();
		s.parse();
		const published = s.views.length;
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 2 } });
		expect(s.views.length).toBe(published);
	});

	it('auto at the level already held logs the choice and switches nothing', () => {
		const s = arm();
		s.parse();
		s.control.pick('auto');
		expect(s.engine.nexts).toEqual([]);
		expect(s.logs).toContain('auto');
	});
});

describe('armQuality — levels hls.js removes', () => {
	// On a mixed-codec master hls.js removes the variants the browser
	// refuses (`removeLevel`): the remaining levels are re-indexed and
	// LEVELS_UPDATED carries the same level objects, fewer of them. It
	// does so inside the error's own dispatch, before the app's ERROR
	// listener runs, and that error's `nextAutoLevel` still names the
	// old list.
	// One LEVELS_UPDATED per level removed, highest index first, as
	// hls.js's `switchLevel` loop removes them.
	function removeFrom(engine: FakeEngine, gone: number[]) {
		let left = engine.levels;
		for (const index of [...gone].sort((a, b) => b - a)) {
			left = left.filter((_, i) => i !== index);
			engine.levels = left;
			engine.emit(E.LEVELS_UPDATED, { levels: left });
		}
		return left;
	}

	function armObjects(setting = 'best', levels = MASTER) {
		const s = arm(setting, levels);
		const objects = levels.map((l) => ({ ...l }));
		s.engine.levels = objects;
		s.engine.emit(E.MANIFEST_PARSED, { levels: objects });
		const remove = (...gone: number[]) => removeFrom(s.engine, gone);
		return { ...s, objects, remove };
	}

	const LADDER5 = [
		{ height: 240, bitrate: 400_000 },
		{ height: 360, bitrate: 800_000 },
		{ height: 480, bitrate: 1_400_000 },
		{ height: 720, bitrate: 2_500_000 },
		{ height: 1080, bitrate: 5_000_000 }
	];
	const tickOver = () => new Promise((r) => setTimeout(r, 0));

	it('a video codec error whose levels hls.js removed steps nothing down', () => {
		// The codec of 1080p and 360p is refused: hls.js removes both and
		// the hold re-seats on 720p, the best level left. The error that
		// caused it is no failure of 720p.
		const s = armObjects('best', LADDER5);
		s.engine.manual = -1;
		s.remove(4, 1);
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_ADD_CODEC_ERROR,
			fatal: false,
			sourceBufferName: 'video',
			errorAction: SWITCHED(2)
		});
		expect(s.logs.join('\n')).not.toMatch(/fragments-failed/);
		expect(s.engine.loads.at(-1)).toBe(2);
		expect(s.lastView()?.levels.map((l) => l.height)).toEqual([240, 480, 720]);
	});

	it('the policy keeps reading playlist heights after a removal, whatever a segment revealed', () => {
		// Two variants tagged 720p; the one the setting holds is removed,
		// and the survivor's segments turned out to be encoded at 1080p.
		// The ceiling is the 720p-tagged survivor, so after a stall Auto
		// climbs back to it.
		const ladder = [
			{ height: 360, bitrate: 800_000 },
			{ height: 720, bitrate: 2_000_000 },
			{ height: 720, bitrate: 3_000_000 }
		];
		const s = armObjects('720', ladder);
		expect(s.engine.loads).toEqual([2]);
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
			frag: { type: 'main', level: 1 },
			tracks: { video: { metadata: { width: 1920, height: 1080 } } }
		});
		s.remove(2);
		// The menu names the survivor by what it is encoded at.
		expect(s.lastView()?.levels[1].height).toBe(1080);
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		expect(s.engine.loads.at(-1)).toBe(0);
		s.video.ahead = HEALTHY_BUFFER_S + 1;
		s.at(10_000 + HEALTHY_WINDOWS_MS[0]);
		s.engine.emit(E.FRAG_BUFFERED, { frag: { type: 'main', level: 0 } });
		expect(s.engine.loads.at(-1)).toBe(1);
	});

	it('the menu lists levels in the order the player moves through them, named as encoded', () => {
		// Two variants tagged 720p; the higher-bitrate one turns out to be
		// encoded at 480p. The player's "below" and "above" follow the
		// playlists, so the menu does too: a step down or a pin's re-seat
		// is always a row further down, whatever the labels say.
		const ladder = [
			{ height: 360, bitrate: 800_000 },
			{ height: 720, bitrate: 2_000_000 },
			{ height: 720, bitrate: 3_000_000 }
		];
		const s = armObjects('best', ladder);
		for (const [level, height] of [
			[1, 720],
			[2, 480]
		]) {
			s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
				frag: { type: 'main', level },
				tracks: { video: { metadata: { width: 1, height } } }
			});
		}
		const menu = qualityMenu(s.lastView() ?? null);
		expect(menu?.items.map((i) => i.key)).toEqual(['auto', 2, 1, 0]);
		expect(menu?.items[1].label).toBe(m.play_quality_height({ height: '480' }));
		// The log names a level by both heights when they differ.
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		expect(s.logs.join('\n')).toMatch(/stall: 480p \(level 2, listed 720p.*→ 720p \(level 1/);
	});

	it('a pin is kept at the playlist height of the level picked, whatever it was encoded at', () => {
		const s = armObjects();
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
			frag: { type: 'main', level: 0 },
			tracks: { video: { metadata: { width: 854, height: 480 } } }
		});
		s.control.pick(0);
		expect(qualityPin.get()).toBe(360);
		// A full replacement re-pins by that same playlist height.
		const fresh = s.engine.levels.map((l) => ({ ...l }));
		s.engine.levels = fresh;
		s.engine.emit(E.LEVELS_UPDATED, { levels: fresh });
		expect(s.lastView()?.pinned).toBe(0);
	});

	it('a pinned level removed re-seats at the nearest level below it, still pinned', async () => {
		const s = armObjects();
		s.control.pick(1);
		s.remove(1);
		expect(s.lastView()?.pinned).toBe(0);
		await tickOver();
		expect(s.engine.loads.at(-1)).toBe(0);
		expect(s.lastView()?.levels[0].height).toBe(360);
	});

	it('a pinned level removed with nothing below it shows as Auto on the level nearest above', () => {
		const s = armObjects();
		s.control.pick(0);
		s.remove(0);
		expect(s.lastView()?.pinned).toBeNull();
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		// Held at 720p (now level 0): nothing below to step to.
		expect(s.engine.loads.filter((l) => l === 1)).toEqual([]);
		// The run keeps the viewer's height for the next stream.
		expect(qualityPin.get()).toBe(360);
	});

	it('a stepped-down hold removed re-seats below it, not back at the setting', () => {
		const s = armObjects();
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		expect(s.engine.loads).toEqual([2, 1]);
		s.remove(1);
		expect(s.logs.join('\n')).toMatch(/holding 360p/);
	});

	it('a pinned level removed never re-seats above the pin while a level below survives unloadable', () => {
		const ladder = [
			{ height: 480, bitrate: 1_400_000 },
			{ height: 720, bitrate: 2_500_000 },
			{ height: 1080, bitrate: 5_000_000 }
		];
		const s = armObjects('worst', ladder);
		s.engine.emit(E.ERROR, {
			details: D.LEVEL_LOAD_ERROR,
			fatal: false,
			level: 0,
			errorAction: SWITCHED(1)
		});
		s.control.pick(1);
		s.remove(1);
		// 480p is left but cannot load; 1080p is above the pin.
		expect(s.lastView()?.pinned).toBeNull();
	});

	it('picking the level held after a release takes the choice back from the engine', () => {
		const s = arm('worst', [MASTER[0], MASTER[1]]);
		s.parse();
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'main', level: 0 } });
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(1),
			frag: { type: 'main', level: 0 }
		});
		expect(s.engine.autoLevelEnabled).toBe(true);
		s.control.pick(0);
		expect(s.engine.autoLevelEnabled).toBe(false);
		expect(s.engine.nexts.at(-1)).toBe(0);
		expect(s.lastView()?.pinned).toBe(0);
	});

	it('Auto after a release takes the choice back from the engine', () => {
		const s = arm('worst', [MASTER[0], MASTER[1]]);
		s.parse();
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(1),
			frag: { type: 'main', level: 0 }
		});
		s.control.pick('auto');
		expect(s.engine.autoLevelEnabled).toBe(false);
	});

	it('levels all replaced keep the stream pinned at the pinned height among the new ones', async () => {
		const s = armObjects('best', [MASTER[0], { height: 480, bitrate: 1_400_000 }, MASTER[2]]);
		s.control.pick(1);
		const fresh = s.engine.levels.map((l) => ({ ...l }));
		s.engine.levels = fresh;
		s.engine.emit(E.LEVELS_UPDATED, { levels: fresh });
		await tickOver();
		expect(s.lastView()?.pinned).toBe(1);
		expect(s.engine.loadLevel).toBe(1);
	});

	it('levels all replaced with none at or below the pin show as Auto', () => {
		const s = armObjects();
		s.control.pick(0);
		const fresh = [{ ...MASTER[1] }, { ...MASTER[2] }];
		s.engine.levels = fresh;
		s.engine.emit(E.LEVELS_UPDATED, { levels: fresh });
		expect(s.lastView()?.pinned).toBeNull();
	});

	it('a run pin below every level of the next stream starts it as Auto, not pinned above', () => {
		qualityPin.set(240);
		const s = arm('best');
		s.parse();
		expect(s.lastView()?.pinned).toBeNull();
		expect(s.engine.loads).toEqual([2]);
	});

	it('the choice released to the engine clears the pin the menu shows', () => {
		const s = armObjects();
		s.control.pick(0);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(1),
			frag: { type: 'main', level: 0 }
		});
		expect(s.lastView()?.pinned).toBeNull();
		expect(qualityPin.get()).toBe(360);
	});

	it('a switch followed after the hold was re-seated blames no level', () => {
		// 480p is held for a `480` setting; the audio codec of 480p and
		// 240p is refused. hls.js removes both, the hold re-seats on 360p,
		// and the error's switch goes to 600p. 360p never failed: it must
		// stay a level to step down to.
		const ladder = [
			{ height: 240, bitrate: 400_000 },
			{ height: 360, bitrate: 800_000 },
			{ height: 480, bitrate: 1_400_000 },
			{ height: 600, bitrate: 2_000_000 }
		];
		const s = armObjects('480', ladder);
		s.engine.manual = -1;
		s.remove(2, 0);
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_ADD_CODEC_ERROR,
			fatal: false,
			sourceBufferName: 'audio',
			errorAction: SWITCHED(3)
		});
		expect(s.engine.loads.at(-1)).toBe(1);
		expect(s.lastView()?.unavailable).toEqual([]);
	});

	it('several removals in one dispatch are read from the list the error named', () => {
		const s = armObjects('worst', LADDER5);
		s.engine.manual = -1;
		s.remove(3, 1);
		// The audio buffer refused its codec: hls.js switched to what was
		// level 4 (1080p), level 2 of the three left.
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_ADD_CODEC_ERROR,
			fatal: false,
			sourceBufferName: 'audio',
			errorAction: SWITCHED(4)
		});
		expect(s.engine.loads.at(-1)).toBe(2);
		expect(s.logs.join('\n')).toMatch(/followed: 240p.*→ 1080p/);
	});

	it('a removal outside an error puts the engine back on the level held', async () => {
		const s = armObjects();
		// hls.js drops to its own choice when the level it was on goes.
		s.engine.manual = -1;
		s.remove(2);
		await tickOver();
		expect(s.engine.loads.at(-1)).toBe(1);
		// A level below removed shifts the one held: its new index is set.
		const t = armObjects();
		t.remove(0);
		await tickOver();
		expect(t.engine.loads.at(-1)).toBe(1);
	});

	it('a removal while the settings are on their way is in the levels the start is chosen from', async () => {
		let answer: (q: string) => void = () => {};
		const engine = new FakeEngine();
		const objects = MASTER.map((l) => ({ ...l }));
		armQuality({
			engine: engine as unknown as Hls,
			events: E,
			video: new FakeVideo(),
			setting: () => new Promise<string>((r) => (answer = r)),
			holdsStart: true,
			onView: () => {},
			now: () => 1_000,
			log: () => {}
		});
		engine.levels = objects;
		engine.emit(E.MANIFEST_PARSED, { levels: objects });
		removeFrom(engine, [2]);
		answer('best');
		await tickOver();
		expect(engine.startLevel).toBe(1);
		expect(engine.loads).toEqual([1]);
	});

	it('keeps holding the same level under its new index', () => {
		const s = armObjects();
		s.remove(1);
		expect(s.lastView()?.levels).toEqual([MASTER[0], MASTER[2]]);
		s.at(10_000);
		s.engine.emit(E.ERROR, { details: D.BUFFER_STALLED_ERROR, fatal: false });
		// 1080p was level 2 and is level 1 now; the step down is to 360p.
		expect(s.engine.loads).toEqual([2, 0]);
		s.control.pick(1);
		expect(s.engine.nexts).toEqual([1]);
		expect(qualityPin.get()).toBe(1080);
	});

	it('the level held removed moves to the best level left, and the engine is put there', () => {
		const s = armObjects();
		s.remove(2);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, { details: D.FRAG_LOAD_ERROR, fatal: false, frag: { type: 'main' } });
		expect(s.engine.loads.at(-1)).toBe(1);
		expect(s.lastView()?.levels).toEqual([MASTER[0], MASTER[1]]);
	});

	it('remaps the levels left unavailable, the heights learned and the level playing', () => {
		const s = armObjects();
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
			frag: { type: 'main', level: 2 },
			tracks: { video: { metadata: { width: 1920, height: 1088 } } }
		});
		s.engine.emit(E.FRAG_CHANGED, { frag: { type: 'main', level: 2 } });
		s.engine.emit(E.ERROR, { details: D.LEVEL_LOAD_ERROR, fatal: true, level: 1 });
		s.remove(0);
		expect(s.lastView()).toEqual({
			levels: [MASTER[1], { height: 1088, bitrate: MASTER[2].bitrate }],
			pinned: null,
			playing: 1,
			unavailable: [0]
		});
	});

	it("an error's switch named in the old list is read in the new one", () => {
		const s = armObjects();
		s.remove(0);
		s.engine.manual = -1;
		// The audio buffer refused its codec: hls.js switched to what was
		// level 1 (720p), which is level 0 now.
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_ADD_CODEC_ERROR,
			fatal: false,
			sourceBufferName: 'audio',
			errorAction: SWITCHED(1)
		});
		expect(s.engine.loads.at(-1)).toBe(0);
		expect(s.logs.join('\n')).toMatch(/followed: 1080p.*→ 720p/);
	});

	it('a switch named in the old list for a level removed follows nothing', () => {
		const s = armObjects();
		s.remove(1);
		s.engine.emit(E.ERROR, {
			details: D.BUFFER_ADD_CODEC_ERROR,
			fatal: false,
			sourceBufferName: 'audio',
			errorAction: SWITCHED(1)
		});
		expect(s.logs.join('\n')).not.toMatch(/followed/);
	});

	it('a later error is read in the new list as it is', async () => {
		const s = armObjects();
		s.remove(0);
		await new Promise((r) => setTimeout(r, 0));
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_ERROR,
			fatal: false,
			errorAction: SWITCHED(0),
			frag: { type: 'audio', level: 1 }
		});
		expect(s.engine.loads.at(-1)).toBe(0);
	});

	it('a removal that leaves one level hands the choice to the engine', () => {
		const s = armObjects();
		s.remove(0, 1);
		expect(s.lastView()?.levels).toEqual([MASTER[2]]);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, { details: D.FRAG_LOAD_ERROR, fatal: false, frag: { type: 'main' } });
		expect(s.engine.autoLevelEnabled).toBe(true);
	});

	it('a removal before the levels are known, or after the stream retires, changes nothing', () => {
		const s = arm();
		s.engine.emit(E.LEVELS_UPDATED, { levels: [MASTER[0]] });
		expect(s.views).toEqual([]);
		const t = armObjects();
		t.control.dispose();
		const published = t.views.length;
		t.remove(0);
		expect(t.views.length).toBe(published);
	});
});

describe('armQuality — settings still on their way', () => {
	function armHeld(setting: () => string | null | undefined | Promise<string | null | undefined>) {
		const engine = new FakeEngine();
		const views: (QualityView | null)[] = [];
		const control = armQuality({
			engine: engine as unknown as Hls,
			events: E,
			video: new FakeVideo(),
			setting,
			holdsStart: true,
			onView: (v) => views.push(v),
			now: () => 1_000,
			log: () => {}
		});
		const parse = (levels = MASTER) => engine.emit(E.MANIFEST_PARSED, { levels });
		return { engine, views, control, parse };
	}
	const settle = () => new Promise((r) => setTimeout(r, 0));

	it('chooses the start level once the setting arrives, and only then starts the engine', async () => {
		let answer: (q: string) => void = () => {};
		const s = armHeld(() => new Promise((r) => (answer = r)));
		s.parse();
		await settle();
		expect(s.engine.loads).toEqual([]);
		expect(s.engine.starts).toBe(0);
		expect(s.views).toEqual([]);
		answer('worst');
		await settle();
		expect(s.engine.startLevel).toBe(0);
		expect(s.engine.loads).toEqual([0]);
		expect(s.engine.starts).toBe(1);
		expect(s.views.at(-1)?.levels).toEqual(MASTER);
	});

	it('settings that fail read as best', async () => {
		const s = armHeld(() => Promise.reject(new Error('settings down')));
		s.parse();
		await settle();
		expect(s.engine.loads).toEqual([2]);
		expect(s.engine.starts).toBe(1);
	});

	it('a setting in hand starts the engine at once, and a stream of one level is started too', () => {
		const s = armHeld(() => '720');
		s.parse();
		expect(s.engine.loads).toEqual([1]);
		expect(s.engine.starts).toBe(1);
		const one = armHeld(() => 'best');
		one.parse([MASTER[1]]);
		expect(one.engine.loads).toEqual([]);
		expect(one.engine.starts).toBe(1);
	});

	it('a stream retired before the setting arrives starts nothing', async () => {
		let answer: (q: string) => void = () => {};
		const s = armHeld(() => new Promise((r) => (answer = r)));
		s.parse();
		s.control.dispose();
		answer('best');
		await settle();
		expect(s.engine.loads).toEqual([]);
		expect(s.engine.starts).toBe(0);
	});

	it('without the option the engine is left to start itself', () => {
		const s = arm('best');
		s.parse();
		expect(s.engine.starts).toBe(0);
	});
});

describe('qualityPin', () => {
	it('keeps a height and forgets it', () => {
		qualityPin.set(720);
		expect(qualityPin.get()).toBe(720);
		qualityPin.clear();
		expect(qualityPin.get()).toBeNull();
	});
});
