// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { expectCaptions, noteCaptionChange } from './caption-watch';
import {
	armSidecarTracks,
	attachSidecarTracks,
	fetchSidecarTracks,
	type SidecarTrack
} from './sidecar-tracks';

const en: SidecarTrack = {
	lang: 'en',
	label: 'English',
	default: true,
	url: 'http://127.0.0.1:31337/s/session-1/sub/0.vtt'
};
const es: SidecarTrack = {
	lang: 'es',
	label: 'Español',
	default: false,
	url: 'http://127.0.0.1:31337/s/session-1/sub/1.vtt'
};

function video(): HTMLVideoElement {
	return document.createElement('video');
}

async function settle() {
	await new Promise((r) => setTimeout(r, 0));
}

describe('attachSidecarTracks', () => {
	it('appends one <track> per listing and the remover takes them back', () => {
		const v = video();
		const remove = attachSidecarTracks(v, [en, es]);
		const tracks = Array.from(v.querySelectorAll('track'));
		expect(tracks.map((t) => t.getAttribute('srclang'))).toEqual(['en', 'es']);
		expect(tracks[0].getAttribute('kind')).toBe('subtitles');
		expect(tracks[0].getAttribute('label')).toBe('English');
		expect(tracks[0].getAttribute('src')).toBe(en.url);
		expect(tracks[0].hasAttribute('default')).toBe(true);
		expect(tracks[1].hasAttribute('default')).toBe(false);
		remove();
		expect(v.querySelectorAll('track').length).toBe(0);
	});
});

describe('the track a session starts on', () => {
	// A track the stream's own playlist carries — hls.js adds it with
	// addTextTrack — may already be showing when the sidecar tracks
	// arrive, and the browser honours a `default` attribute only while
	// no subtitle track shows. The start is applied to the video's
	// tracks, so it holds whatever was showing.
	it('turns off a track already showing when the start is off', () => {
		const v = video();
		const inManifest = v.addTextTrack('subtitles', 'Manifest', 'en');
		inManifest.mode = 'showing';
		attachSidecarTracks(v, [en, es], 'en', 'off');
		expect(inManifest.mode).toBe('disabled');
	});

	it('turns off a track already showing when a sidecar track is the start', () => {
		const v = video();
		const inManifest = v.addTextTrack('subtitles', 'Manifest', 'en');
		inManifest.mode = 'showing';
		attachSidecarTracks(v, [en, es], 'es');
		expect(inManifest.mode).toBe('disabled');
		expect(v.querySelectorAll('track')[1].hasAttribute('default')).toBe(true);
	});
});

// Chromium runs its own track selection a moment after `<track>`
// elements join a video, and while a subtitle track shows it turns on
// one more of the tracks no script has set since they joined: a start
// switched on after the tracks joined left a second language showing
// beside it — Arabic over English on a nine-language episode. A track
// already in its mode when it joins gives the browser nothing to
// choose, and tracks that leave switched off leave nothing showing to
// force a choice in the next session.
//
// happy-dom builds a fresh TextTrack on every `track` read, so a mode
// set on one is gone by the next read; these tests hand the adapter
// track elements whose TextTrack keeps its mode, and a video that
// records what joined it and how.
describe('the start shows alone', () => {
	interface FakeTrack {
		kind: string;
		label: string;
		mode: string;
	}

	/** Track elements whose TextTrack keeps its mode, and a video that
	 *  notes each track's mode as it joins. */
	function fakeVideo() {
		const tracks: FakeTrack[] = [];
		const joined: string[] = [];
		const create = document.createElement.bind(document);
		vi.spyOn(document, 'createElement').mockImplementation(((tag: string) => {
			if (tag !== 'track') return create(tag);
			const attrs = new Map<string, string>();
			const track: FakeTrack = { kind: 'subtitles', label: '', mode: 'disabled' };
			const el = {
				track,
				setAttribute(name: string, value: string) {
					attrs.set(name, value);
					if (name === 'label') track.label = value;
				},
				hasAttribute: (name: string) => attrs.has(name),
				remove() {
					tracks.splice(tracks.indexOf(track), 1);
				}
			};
			return el as unknown as HTMLElement;
		}) as typeof document.createElement);
		const v = {
			textTracks: tracks,
			appendChild(el: { track: FakeTrack }) {
				joined.push(`${el.track.label}:${el.track.mode}`);
				tracks.push(el.track);
				return el;
			}
		};
		return { v: v as unknown as HTMLVideoElement, tracks, joined };
	}

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it('the start track is already showing when it joins the video, every other one off', () => {
		const { v, joined } = fakeVideo();
		attachSidecarTracks(v, [en, es], 'es');
		expect(joined).toEqual(['English:disabled', 'Español:showing']);
	});

	it('an Off start adds every track switched off', () => {
		const { v, joined } = fakeVideo();
		attachSidecarTracks(v, [en, es], 'es', 'off');
		expect(joined).toEqual(['English:disabled', 'Español:disabled']);
	});

	it('the tracks leave the video switched off', () => {
		const { v, tracks } = fakeVideo();
		const remove = attachSidecarTracks(v, [en, es], 'en');
		const attached = [...tracks];
		expect(attached.map((t) => t.mode)).toEqual(['showing', 'disabled']);
		remove();
		expect(tracks).toEqual([]);
		expect(attached.map((t) => t.mode)).toEqual(['disabled', 'disabled']);
	});

	it('the start is what the app expects, so the browser showing it is no pick', () => {
		const { v } = fakeVideo();
		attachSidecarTracks(v, [en, es], 'es');
		expect(noteCaptionChange(v, v.textTracks)).toBeNull();
	});
});

describe('fetchSidecarTracks', () => {
	const originalFetch = globalThis.fetch;
	afterEach(() => {
		globalThis.fetch = originalFetch;
	});

	it('returns the listing the proxy answers', async () => {
		globalThis.fetch = vi.fn(async () => new Response(JSON.stringify([en]))) as typeof fetch;
		expect(await fetchSidecarTracks('http://127.0.0.1:31337', 'session-1')).toEqual([en]);
	});

	it('answers no tracks for a refusal, a throw, or a body that is not a list', async () => {
		globalThis.fetch = vi.fn(async () => new Response('nope', { status: 404 })) as typeof fetch;
		expect(await fetchSidecarTracks('http://127.0.0.1:31337', 'session-1')).toEqual([]);
		globalThis.fetch = vi.fn(async () => {
			throw new TypeError('network');
		}) as typeof fetch;
		expect(await fetchSidecarTracks('http://127.0.0.1:31337', 'session-1')).toEqual([]);
		globalThis.fetch = vi.fn(
			async () => new Response(JSON.stringify({ not: 'a list' }))
		) as typeof fetch;
		expect(await fetchSidecarTracks('http://127.0.0.1:31337', 'session-1')).toEqual([]);
	});
});

describe('armSidecarTracks', () => {
	const originalFetch = globalThis.fetch;
	beforeEach(() => {
		globalThis.fetch = vi.fn(async () => new Response(JSON.stringify([en]))) as typeof fetch;
	});
	afterEach(() => {
		globalThis.fetch = originalFetch;
	});

	it('attaches what arrives and the cleanup removes it', async () => {
		const v = video();
		const cleanup = armSidecarTracks(v, 'http://127.0.0.1:31337', 'session-1');
		await settle();
		expect(v.querySelectorAll('track').length).toBe(1);
		cleanup();
		expect(v.querySelectorAll('track').length).toBe(0);
	});

	it('attaches nothing when cancelled before the listing arrives', async () => {
		const v = video();
		const cleanup = armSidecarTracks(v, 'http://127.0.0.1:31337', 'session-1');
		cleanup();
		await settle();
		expect(v.querySelectorAll('track').length).toBe(0);
	});

	// The session going is no pick of the viewer's, whether or not it
	// had sidecar tracks: whatever its teardown does to the captions —
	// a playlist's own track switched off — is not remembered.
	it('stops taking caption changes for picks when the session goes, tracks or none', async () => {
		globalThis.fetch = vi.fn(async () => new Response(JSON.stringify([]))) as typeof fetch;
		const v = video();
		expectCaptions(v, 0);
		const cleanup = armSidecarTracks(v, 'http://127.0.0.1:31337', 'session-1');
		await settle();
		cleanup();
		const list = [
			{ kind: 'subtitles', mode: 'disabled' },
			{ kind: 'subtitles', mode: 'showing' }
		];
		expect(noteCaptionChange(v, list)).toBeNull();
	});

	it('attaches nothing for an empty listing', async () => {
		globalThis.fetch = vi.fn(async () => new Response(JSON.stringify([]))) as typeof fetch;
		const v = video();
		armSidecarTracks(v, 'http://127.0.0.1:31337', 'session-1');
		await settle();
		expect(v.querySelectorAll('track').length).toBe(0);
	});
});

// The listing is asked from the origin the media URL carries. The
// backend builds that URL on its own address, so the ask reaches the
// backend whether the preload bridge is on `window` (Electron) or
// nothing is (browser-only dev, where the base comes from an env
// variable): a page that read the bridge alone asked the dev server
// instead, and the swallowed failure showed as a session without
// subtitles.
describe('the listing is asked from the origin of the media URL', () => {
	const originalFetch = globalThis.fetch;
	afterEach(() => {
		globalThis.fetch = originalFetch;
	});

	it('asks the media origin, with no preload bridge on window', () => {
		const asked: string[] = [];
		globalThis.fetch = vi.fn(async (input: RequestInfo | URL) => {
			asked.push(String(input));
			return new Response('[]');
		}) as typeof fetch;
		expect('aniGui' in window, 'no preload bridge').toBe(false);
		const cleanup = armSidecarTracks(
			video(),
			'http://127.0.0.1:4567/s/session-1/master.m3u8',
			'session-1'
		);
		expect(asked).toEqual(['http://127.0.0.1:4567/s/session-1/subtitles']);
		cleanup();
	});
});
