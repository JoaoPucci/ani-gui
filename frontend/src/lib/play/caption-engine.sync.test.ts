// The sync against a model of hls.js's subtitle handling
// (tests/hls-subtitle-model.ts) on a video that lists its `<track>`
// elements' tracks first, as Chromium does.
import { describe, expect, it } from 'vitest';
import type Hls from 'hls.js';
import { createCaptionSync, pickCaptions, type CaptionSyncEvents } from './caption-engine';
import {
	HlsSubtitleModel,
	MODEL_EVENTS,
	ModelTrackVideo,
	type ModelTrack
} from '../../../tests/hls-subtitle-model';

const RENDITIONS = [
	{ name: 'English', lang: 'en', default: true },
	{ name: 'Español', lang: 'es' }
];

/** Long enough for the list's `change` and the engine's zero-delay
 *  read of it, twice over. */
async function settle() {
	for (let i = 0; i < 4; i++) await new Promise((r) => setTimeout(r, 0));
}

function setup() {
	const video = new ModelTrackVideo();
	const engine = new HlsSubtitleModel(video.addTextTrack);
	engine.attach(video);
	const sync = createCaptionSync(
		video,
		engine as unknown as Hls,
		MODEL_EVENTS as unknown as CaptionSyncEvents
	);
	return { video, engine, sync };
}

/** The playlist parsed and its group in: the engine has picked its
 *  default, English. */
function playlistIn(engine: HlsSubtitleModel) {
	engine.manifestLoading();
	engine.manifestParsed(RENDITIONS);
	engine.groupArrives();
}

const showing = (video: ModelTrackVideo) => {
	const out: string[] = [];
	for (let i = 0; i < video.textTracks.length; i++) {
		const t = video.textTracks[i];
		if (t.mode === 'showing') out.push(`${t.label}/${t.language}`);
	}
	return out;
};
const indexOf = (video: ModelTrackVideo, track: ModelTrack) => {
	for (let i = 0; i < video.textTracks.length; i++) if (video.textTracks[i] === track) return i;
	return -1;
};

describe('a sidecar track picked while the engine shows a rendition', () => {
	it('stays on, the engine on none', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		const fr = video.addElement('Français', 'fr');
		sync.pick(indexOf(video, fr));
		await settle();
		expect(showing(video)).toEqual(['Français/fr']);
		expect(engine.subtitleTrack).toBe(-1);
		expect(engine.ownSwitches).toEqual([0]);
	});
});

describe('a rendition picked while a sidecar track shows', () => {
	it('is the engine’s pick, shown alone', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		const fr = video.addElement('Français', 'fr');
		sync.pick(indexOf(video, fr));
		await settle();
		sync.pick(indexOf(video, engine.textTracks[1]));
		await settle();
		expect(showing(video)).toEqual(['Español/es']);
		expect(engine.subtitleTrack).toBe(1);
		expect(engine.subtitleDisplay).toBe(true);
	});
});

describe('Off', () => {
	it('leaves nothing showing and the engine on none', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		sync.pick('off');
		await settle();
		expect(showing(video)).toEqual([]);
		expect(engine.subtitleTrack).toBe(-1);
	});

	it('keeps the rendition whose fragment is on its way hidden, its cues kept', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		const sn = engine.fragmentLoading(0);
		sync.pick('off');
		await settle();
		expect(showing(video)).toEqual([]);
		expect(engine.textTracks[0].mode).toBe('hidden');
		engine.fragmentLands(0, sn, 'cue');
		expect(engine.cues[0]).toEqual(['cue']);
		// Once in, the rendition goes off and the engine on none.
		expect(engine.textTracks[0].mode).toBe('disabled');
		expect(engine.subtitleTrack).toBe(-1);
		await settle();
		expect(showing(video)).toEqual([]);
	});
});

// The engine loads one subtitle fragment at a time and starts the next
// as it finishes one — before the sync hears the first is in.
describe('the next fragment, started as one lands', () => {
	it('keeps its rendition hidden', () => {
		const { engine, sync } = setup();
		playlistIn(engine);
		const first = engine.fragmentLoading(0);
		sync.pick('off');
		// The engine's own handler runs first and starts the next one.
		engine.handlers[MODEL_EVENTS.SUBTITLE_FRAG_PROCESSED].unshift(() => {
			if (engine.cues[0].length === 1) engine.fragmentLoading(0);
		});
		engine.fragmentLands(0, first, 'first');
		expect(engine.textTracks[0].mode).toBe('hidden');
		expect(engine.subtitleTrack).toBe(0);
	});
});

describe('a fragment that fails on its way', () => {
	it('lets its rendition go off', () => {
		const { engine, sync } = setup();
		playlistIn(engine);
		expect(engine.fragmentLoading(0)).toBe(0);
		sync.pick('off');
		expect(engine.textTracks[0].mode).toBe('hidden');
		engine.emit(MODEL_EVENTS.ERROR, { frag: { type: 'main', level: 0, sn: 0 } });
		expect(engine.textTracks[0].mode).toBe('hidden');
		engine.emit(MODEL_EVENTS.ERROR, { frag: { type: 'subtitle', level: 0, sn: 0 } });
		expect(engine.textTracks[0].mode).toBe('disabled');
		expect(engine.subtitleTrack).toBe(-1);
	});
});

describe('a rendition whose fragment is on its way when another track is picked', () => {
	it('keeps its cues, and shows them when picked again', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		const fr = video.addElement('Français', 'fr');
		const sn = engine.fragmentLoading(0);
		sync.pick(indexOf(video, fr));
		await settle();
		engine.fragmentLands(0, sn, 'cue');
		await settle();
		expect(showing(video)).toEqual(['Français/fr']);
		sync.pick(indexOf(video, engine.textTracks[0]));
		await settle();
		expect(showing(video)).toEqual(['English/en']);
		expect(engine.cues[0]).toEqual(['cue']);
	});

	it('keeps its cues when another rendition is picked', async () => {
		const { engine, sync, video } = setup();
		playlistIn(engine);
		const sn = engine.fragmentLoading(0);
		sync.pick(indexOf(video, engine.textTracks[1]));
		engine.fragmentLands(0, sn, 'cue');
		expect(engine.cues[0]).toEqual(['cue']);
		await settle();
		expect(showing(video)).toEqual(['Español/es']);
	});
});

describe("the engine's own switches", () => {
	it('are brought back to the pick in the same event: its default after the playlist loads again', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		const fr = video.addElement('Français', 'fr');
		sync.pick(indexOf(video, fr));
		await settle();
		const before = engine.ownSwitches.length;
		engine.manifestLoading();
		engine.manifestParsed(RENDITIONS);
		engine.groupArrives();
		// The engine did pick its default, and nothing shows for it.
		expect(engine.ownSwitches.length).toBe(before + 1);
		expect(showing(video)).toEqual(['Français/fr']);
		expect(engine.subtitleTrack).toBe(-1);
		await settle();
		expect(showing(video)).toEqual(['Français/fr']);
	});

	it('are left alone while the app has made no pick', () => {
		const { video, engine } = setup();
		playlistIn(engine);
		expect(showing(video)).toEqual(['English/en']);
		expect(engine.subtitleTrack).toBe(0);
	});

	it('are left alone once the app lets go', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		sync.pick('off');
		sync.release();
		engine.manifestLoading();
		engine.groupArrives();
		await settle();
		expect(showing(video)).toEqual(['English/en']);
	});

	it('are left alone once the engine retires', () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		sync.pick('off');
		sync.dispose();
		engine.manifestLoading();
		engine.groupArrives();
		expect(showing(video)).toEqual(['English/en']);
	});
});

describe('tracks of the app’s own about to join', () => {
	it('set the engine on none first, so a track already showing goes off and its fragment keeps its cues', () => {
		const { engine, sync } = setup();
		playlistIn(engine);
		const sn = engine.fragmentLoading(0);
		sync.beforeOwnTracks('own');
		expect(engine.subtitleTrack).toBe(-1);
		expect(engine.textTracks[0].mode).toBe('hidden');
		engine.fragmentLands(0, sn, 'cue');
		expect(engine.cues[0]).toEqual(['cue']);
	});

	it('then the start joining showing stays on', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		sync.beforeOwnTracks('own');
		const fr = video.addElement('Français', 'fr', 'showing');
		sync.pickTrack(fr);
		await settle();
		expect(showing(video)).toEqual(['Français/fr']);
		expect(engine.subtitleTrack).toBe(-1);
	});
});

describe('a sidecar track labelled as a rendition', () => {
	it('is no pick of the rendition’s, once its label is kept apart', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		sync.beforeOwnTracks('own');
		const label = (await import('./caption-engine')).distinctLabel(
			'English',
			'en',
			sync.renditions()
		);
		const side = video.addElement(label, 'en', 'showing');
		sync.pickTrack(side);
		await settle();
		await settle();
		expect(engine.subtitleTrack).toBe(-1);
		expect(side.mode).toBe('showing');
		expect(engine.textTracks[0].mode).toBe('disabled');
	});
});

describe('whenReady', () => {
	it('runs once the playlist is parsed, or at once after', () => {
		const { engine, sync } = setup();
		const ran: string[] = [];
		sync.whenReady(() => ran.push('early'));
		const cancel = sync.whenReady(() => ran.push('cancelled'));
		cancel();
		engine.manifestLoading();
		expect(ran).toEqual([]);
		engine.manifestParsed(RENDITIONS);
		expect(ran).toEqual(['early']);
		sync.whenReady(() => ran.push('late'));
		expect(ran).toEqual(['early', 'late']);
		expect(sync.renditions()).toBe(RENDITIONS);
	});
});

describe('pickCaptions', () => {
	it('picks through the engine where there is one', async () => {
		const { video, engine, sync } = setup();
		playlistIn(engine);
		const fr = video.addElement('Français', 'fr');
		pickCaptions(video, indexOf(video, fr), sync);
		await settle();
		expect(showing(video)).toEqual(['Français/fr']);
	});

	it('sets the tracks alone without one', () => {
		const video = new ModelTrackVideo();
		const fr = video.addElement('Français', 'fr');
		pickCaptions(video, indexOf(video, fr), null);
		expect(fr.mode).toBe('showing');
	});
});
