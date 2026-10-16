import { afterEach, describe, expect, it, vi } from 'vitest';
import Hls from 'hls.js';
import { armQuality, qualityPin } from './arm-quality';
import {
	HEALTHY_BUFFER_S,
	HEALTHY_WINDOWS_MS,
	SEEK_GRACE_MS,
	STEP_DOWN_COOLDOWN_MS
} from './quality-governor';
import type { QualityView } from './quality-menu';

const E = Hls.Events;
const D = Hls.ErrorDetails;

// The engine double: the levels, the three ways of setting one, and
// whether the engine has taken the choice back — which hls.js does
// on its own after a level's error, before any listener runs.
class FakeEngine {
	levels: { height: number; bitrate: number }[] = [];
	startLevel = -1;
	manual = -1;
	loads: number[] = [];
	nexts: number[] = [];
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
		// A retry still pending is not a verdict.
		s.engine.emit(E.ERROR, { details: D.LEVEL_LOAD_ERROR, fatal: false, level: 2 });
		expect(s.engine.loads).toEqual([2]);
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.LEVEL_LOAD_TIMEOUT,
			fatal: false,
			levelRetry: true,
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
		s.engine.manual = -1;
		s.engine.emit(E.ERROR, {
			details: D.FRAG_LOAD_TIMEOUT,
			fatal: false,
			levelRetry: true,
			frag: { type: 'main', level: 2 }
		});
		expect(s.engine.loads).toEqual([2, 1]);
		expect(s.engine.autoLevelEnabled).toBe(false);
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
			levelRetry: true,
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

	it('a pick of the level already playing pins it without a switch', () => {
		const s = arm();
		s.parse();
		s.control.pick(2);
		expect(s.engine.nexts).toEqual([]);
		expect(s.lastView()?.pinned).toBe(2);
		expect(qualityPin.get()).toBe(1080);
		expect(s.logs.join('\n')).toMatch(/pinned 1080p/);
	});

	it('the pin is the height the level is encoded at', () => {
		const s = arm();
		s.parse();
		s.engine.emit(E.FRAG_PARSING_INIT_SEGMENT, {
			frag: { type: 'main', level: 0 },
			tracks: { video: { metadata: { width: 854, height: 480 } } }
		});
		s.control.pick(0);
		expect(qualityPin.get()).toBe(480);
	});

	it('a level with no height pins nothing, and a refused pick changes nothing', () => {
		const s = arm('best', [
			{ height: 0, bitrate: 800_000 },
			{ height: 0, bitrate: 2_000_000 }
		]);
		s.parse();
		s.control.pick(0);
		expect(qualityPin.get()).toBeNull();
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

describe('qualityPin', () => {
	it('keeps a height and forgets it', () => {
		qualityPin.set(720);
		expect(qualityPin.get()).toBe(720);
		qualityPin.clear();
		expect(qualityPin.get()).toBeNull();
	});
});
