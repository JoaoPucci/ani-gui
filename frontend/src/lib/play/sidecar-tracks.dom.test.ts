// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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

	it('attaches nothing for an empty listing', async () => {
		globalThis.fetch = vi.fn(async () => new Response(JSON.stringify([]))) as typeof fetch;
		const v = video();
		armSidecarTracks(v, 'http://127.0.0.1:31337', 'session-1');
		await settle();
		expect(v.querySelectorAll('track').length).toBe(0);
	});
});
