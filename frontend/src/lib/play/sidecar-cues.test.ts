import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { expectCaptions, noteCaptionChange } from './caption-watch';
import { keepingSidecarCues, type CueTrackElement, type KeptCue } from './sidecar-cues';

// A <track> element's TextTrack as Chromium keeps it: its cues are
// hidden (null) while the track is disabled and kept all the same, a
// cue knows the track it is on, and a cue already on the track is not
// added twice.
class FakeTrack {
	readonly kind = 'subtitles';
	private own: FakeCue[] = [];
	private current: TextTrackMode;
	/** Every mode the track was set to, and whether the engine's clear
	 *  set it. */
	readonly writes: { mode: TextTrackMode; byClear: boolean }[] = [];
	clearing = false;
	constructor(mode: TextTrackMode) {
		this.current = mode;
	}
	get mode(): TextTrackMode {
		return this.current;
	}
	set mode(mode: TextTrackMode) {
		this.writes.push({ mode, byClear: this.clearing });
		this.current = mode;
	}
	get cues(): FakeCue[] | null {
		return this.current === 'disabled' ? null : [...this.own];
	}
	/** The cues on the track, whatever its mode. */
	held(): FakeCue[] {
		return [...this.own];
	}
	addCue(cue: KeptCue): void {
		const c = cue as FakeCue;
		if (c.track === this) return;
		c.track = this;
		this.own.push(c);
	}
	removeCue(cue: FakeCue): void {
		this.own = this.own.filter((c) => c !== cue);
		cue.track = null;
	}
}

interface FakeCue {
	text: string;
	track: FakeTrack | null;
}

// HTMLTrackElement.readyState.
const NONE = 0;
const LOADING = 1;
const LOADED = 2;

class FakeElement implements CueTrackElement {
	constructor(
		readonly readyState: number,
		readonly track: FakeTrack
	) {}
}

function sidecar(mode: TextTrackMode, readyState: number, texts: string[]): FakeElement {
	const el = new FakeElement(readyState, new FakeTrack(mode));
	for (const text of texts) el.track.addCue({ text, track: null } as FakeCue);
	el.track.writes.length = 0;
	return el;
}

function videoOf(elements: FakeElement[]) {
	const textTracks = elements.map((el) => el.track);
	return {
		textTracks,
		querySelectorAll: () => elements
	};
}

/** hls.js 1.6.19's TimelineController `_cleanTracks`, run as a
 *  manifest starts loading: every text track on the video loses its
 *  cues, a disabled one set hidden for the while, as its cues are
 *  null otherwise. */
function engineClear(elements: FakeElement[]): () => void {
	return () => {
		for (const { track } of elements) {
			track.clearing = true;
			const mode = track.mode;
			if (mode === 'disabled') track.mode = 'hidden';
			for (const cue of track.cues ?? []) track.removeCue(cue);
			if (mode === 'disabled') track.mode = mode;
			track.clearing = false;
		}
	};
}

const texts = (el: FakeElement) => el.track.held().map((c) => c.text);

describe('keepingSidecarCues', () => {
	for (const mode of ['showing', 'hidden', 'disabled'] as const) {
		it(`a loaded sidecar track ${mode} at the reload has its cues after it, still ${mode}`, () => {
			const el = sidecar(mode, LOADED, ['a', 'b']);
			keepingSidecarCues(videoOf([el]), engineClear([el]));
			expect(texts(el)).toEqual(['a', 'b']);
			expect(el.track.mode).toBe(mode);
		});
	}

	it('a track turned off before the reload shows its cues once it is picked again', () => {
		const el = sidecar('disabled', LOADED, ['a']);
		keepingSidecarCues(videoOf([el]), engineClear([el]));
		el.track.mode = 'showing';
		expect(el.track.cues?.map((c) => c.text)).toEqual(['a']);
	});

	it('a track whose file is still on its way keeps what it has, and the rest arrives as before', () => {
		const el = sidecar('showing', LOADING, ['a']);
		keepingSidecarCues(videoOf([el]), engineClear([el]));
		el.track.addCue({ text: 'b', track: null } as FakeCue);
		expect(texts(el)).toEqual(['a', 'b']);
	});

	it('a track never loaded is left alone: it has no cues to keep, and the keeper never switches it', () => {
		const el = sidecar('disabled', NONE, []);
		keepingSidecarCues(videoOf([el]), () => {});
		expect(el.track.writes).toEqual([]);
	});

	it('several tracks keep their own cues each', () => {
		const en = sidecar('showing', LOADED, ['en 0', 'en 2']);
		const fr = sidecar('disabled', LOADED, ['fr 0']);
		const es = sidecar('disabled', NONE, []);
		const all = [en, fr, es];
		keepingSidecarCues(videoOf(all), engineClear(all));
		expect(all.map(texts)).toEqual([['en 0', 'en 2'], ['fr 0'], []]);
	});

	it('a reload after a reload finds the cues there again', () => {
		const el = sidecar('showing', LOADED, ['a']);
		for (let i = 0; i < 3; i++) keepingSidecarCues(videoOf([el]), engineClear([el]));
		expect(texts(el)).toEqual(['a']);
	});

	it('a clear that takes nothing leaves each cue there once', () => {
		const el = sidecar('hidden', LOADED, ['a', 'b']);
		keepingSidecarCues(videoOf([el]), () => {});
		expect(texts(el)).toEqual(['a', 'b']);
	});

	it('what shows is never changed, so the captions watch reads no pick of the viewer', () => {
		const en = sidecar('showing', LOADED, ['en']);
		const fr = sidecar('disabled', LOADED, ['fr']);
		const video = videoOf([en, fr]);
		expectCaptions(video, 0);
		keepingSidecarCues(video, engineClear([en, fr]));
		for (const { track } of [en, fr]) {
			expect(track.writes.filter((w) => !w.byClear && w.mode === 'showing')).toEqual([]);
		}
		expect(en.track.writes.filter((w) => !w.byClear)).toEqual([]);
		expect(noteCaptionChange(video, video.textTracks)).toBeNull();
	});

	it('property: any tracks, any number of reloads — every cue is back once, each mode as it was', () => {
		const track = fc
			.record({
				mode: fc.constantFrom<TextTrackMode>('showing', 'hidden', 'disabled'),
				readyState: fc.constantFrom(NONE, LOADING, LOADED),
				cues: fc.integer({ min: 0, max: 4 })
			})
			// A track that never loaded has no cues.
			.map((t) => ({ ...t, cues: t.readyState === NONE ? 0 : t.cues }));
		fc.assert(
			fc.property(fc.array(track, { maxLength: 5 }), fc.integer({ min: 1, max: 3 }), (specs, n) => {
				const all = specs.map((s, i) =>
					sidecar(
						s.mode,
						s.readyState,
						Array.from({ length: s.cues }, (_, j) => `${i}:${j}`)
					)
				);
				const before = all.map(texts);
				for (let k = 0; k < n; k++) keepingSidecarCues(videoOf(all), engineClear(all));
				expect(all.map(texts)).toEqual(before);
				all.forEach((el, i) => {
					expect(el.track.mode).toBe(specs[i].mode);
					const own = el.track.writes.filter((w) => !w.byClear);
					if (specs[i].readyState === NONE || specs[i].mode !== 'disabled') expect(own).toEqual([]);
					else expect(own.every((w) => w.mode !== 'showing')).toBe(true);
				});
			})
		);
	});
});
