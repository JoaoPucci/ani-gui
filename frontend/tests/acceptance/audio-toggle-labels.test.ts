// Acceptance: the audio toggle speaks the viewer's language.
//
// Portuguese and Spanish spell the audio out (Legendado/Dublado,
// Subtitulado/Doblado) where English and Russian keep SUB/DUB. The
// toggle is drawn on the show page and in Settings; both are mounted
// here in each locale and the rendered buttons are read, so a toggle
// that went back to printing the raw mode cannot hide behind the
// message-value tests.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { setLocale } from '../../src/lib/paraglide/runtime';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import SettingsPage from '../../src/routes/settings/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import type {
	AiringStatus,
	AppInfo,
	AvailabilityBatchResponse,
	AvailabilityResponse,
	Config,
	HistoryEntry,
	HistoryRowByKitsu,
	KitsuAnimeRef,
	KitsuEpisode
} from '../../src/lib/api';
import { appInfoAnswer, availabilityAnswer } from '../answers';

const KITSU_ID = '40684';
const TITLE = 'Two Audio Show';

type Locale = 'en' | 'pt-BR' | 'es-419' | 'ru';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	setLocale('en', { reload: false });
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

function useHandlers(locale: Locale) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json<Config>({ ...appConfig('sub'), locale })
		),
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json<AppInfo>(
				appInfoAnswer({
					version: '0.17.0',
					history_path: '/h',
					proxy_base_url: 'http://127.0.0.1:1'
				})
			)
		),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json<KitsuAnimeRef>({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json<AiringStatus>({
				aired: 12,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json<KitsuEpisode[]>([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json<AvailabilityResponse>(
				availabilityAnswer({ available: true, episode_count: 12, extra_episodes: [] })
			)
		),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json<AvailabilityBatchResponse>({ cached: {}, playable_episode_counts: {} })
		),
		http.post(`${API_BASE}/api/availability/warm`, () => new HttpResponse(null, { status: 202 })),
		http.put(`${API_BASE}/api/settings`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json<HistoryEntry[]>([])),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () =>
			HttpResponse.json<HistoryRowByKitsu | null>(null)
		),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json<string | null>('/dl')),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json<Record<string, number>>({}))
	);
}

const segLabels = () =>
	Array.from(target.querySelectorAll('button.seg-btn')).map((b) => (b.textContent ?? '').trim());

const CASES: Array<[Locale, string, string]> = [
	['pt-BR', 'Legendado', 'Dublado'],
	['es-419', 'Subtitulado', 'Doblado'],
	['en', 'SUB', 'DUB'],
	['ru', 'SUB', 'DUB']
];

describe("the audio toggle is labelled in the viewer's language", () => {
	it.each(CASES)('on the show page in %s: %s / %s', async (locale, sub, dub) => {
		setLocale(locale, { reload: false });
		useHandlers(locale);
		app = mount(DetailPage, { target });
		await until(() => segLabels().includes(sub), `the ${sub} button`);
		expect(segLabels()).toContain(dub);
	});

	it.each(CASES)('in Settings in %s: %s / %s', async (locale, sub, dub) => {
		setLocale(locale, { reload: false });
		useHandlers(locale);
		app = mount(SettingsPage, { target });
		await until(() => segLabels().includes(sub), `the ${sub} button`);
		expect(segLabels()).toContain(dub);
	});
});
