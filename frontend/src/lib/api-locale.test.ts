// Every request whose answer the backend orders subtitle tracks for —
// a play, an external or Syncplay handoff, a prefetch, a download —
// names the locale the renderer runs in. The built-in player starts by
// that locale; the backend would otherwise order by the raw `locale`
// in config.toml, which the renderer adopts only when it is one of the
// app's own (`$lib/settings/boot-locale.ts`), so a hand-edited value
// sent the two orders to different tracks.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getLocale, overwriteGetLocale } from '$lib/paraglide/runtime';
import { startingTrack } from './play/caption-start';
import {
	__resetApiBaseForTests,
	downloadStream,
	play,
	playExternal,
	playStream,
	playSyncplay
} from './api';

const API = 'http://127.0.0.1:1';

type G = {
	window?: { aniGui?: { apiBase?: string } };
	EventSource?: unknown;
	fetch: typeof fetch;
};
const g = globalThis as unknown as G;

class FakeEventSource {
	static urls: string[] = [];
	constructor(url: string) {
		FakeEventSource.urls.push(url);
	}
	addEventListener() {}
	close() {}
}

let bodies: unknown[] = [];
const initialLocale = getLocale();

beforeEach(() => {
	bodies = [];
	FakeEventSource.urls = [];
	__resetApiBaseForTests(API);
	g.window = { aniGui: { apiBase: API } };
	g.EventSource = FakeEventSource;
	g.fetch = vi.fn(async (_url: string, init?: RequestInit) => {
		bodies.push(JSON.parse(String(init?.body)));
		return new Response('{}', { status: 200 });
	}) as unknown as typeof fetch;
	overwriteGetLocale(() => 'pt-BR');
});

afterEach(() => {
	delete g.window;
	delete g.EventSource;
	overwriteGetLocale(() => initialLocale);
});

describe("requests carry the renderer's locale", () => {
	it('a play and both handoffs name it in their body', async () => {
		const args = { title: 'X', episode: '1', mode: 'sub' as const };
		await play(args);
		await playExternal(args);
		await playSyncplay(args);
		expect(bodies).toEqual([
			{ ...args, locale: 'pt-BR' },
			{ ...args, locale: 'pt-BR' },
			{ ...args, locale: 'pt-BR' }
		]);
	});

	it('a renderer whose locale Paraglide cannot resolve still plays, naming an empty one', async () => {
		// The play page reads the same empty locale for the built-in
		// player; the backend reads it as no language, as the player does.
		overwriteGetLocale(() => {
			throw new Error('no strategy resolved');
		});
		const args = { title: 'X', episode: '1', mode: 'sub' as const };
		await play(args);
		void downloadStream(args, () => {});
		await vi.waitFor(() => expect(FakeEventSource.urls).toHaveLength(1));
		expect(bodies).toEqual([{ ...args, locale: '' }]);
		expect(new URL(FakeEventSource.urls[0]).searchParams.get('locale')).toBe('');
	});

	it("a play's progress stream and a download's name it in their query", async () => {
		void playStream({ title: 'X', episode: '1', mode: 'sub', prefetch: true }, () => {});
		void downloadStream({ title: 'X', episode: '1', mode: 'sub' }, () => {});
		await vi.waitFor(() => expect(FakeEventSource.urls).toHaveLength(2));
		for (const url of FakeEventSource.urls) {
			expect(new URL(url).searchParams.get('locale')).toBe('pt-BR');
		}
	});
});

// The backend's twin of this suite (`play_ui_locale_test.rs`) expects
// the handoff to lead with the track the built-in player starts on in
// each renderer locale. These are those starts, on the same listing.
describe('where the built-in player starts on the listing the backend checks', () => {
	const listing = [
		{ lang: 'zh-Hans', default: false },
		{ lang: 'pt-BR', default: false },
		{ lang: 'es-419', default: true },
		{ lang: 'en', default: false }
	];
	it.each([
		['en', 3],
		['pt-BR', 1],
		['', 2]
	])('in %j it starts on track %i', (locale, start) => {
		expect(startingTrack(listing, locale)).toBe(start);
	});
});
