// Acceptance: what /search shows when the search call itself fails.
//
// The failure state has two lines, a headline and a reason. The
// headline used to be an English literal in the route's script, so it
// read the same in every locale; the reason used to be the payload's
// `detail` — free text the backend documents as for logs only — or,
// failing that, the raw `kind` token.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setQuery } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));

import SearchPage from '../../src/routes/search/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { describeError } from '../../src/lib/play/error-copy';
import { setLocale } from '../../src/lib/paraglide/runtime';

const CONFIG = {
	locale: 'en',
	mode: 'sub',
	quality: 'best',
	external_player: '',
	external_player_kind: 'mpv',
	external_player_custom_args: '',
	syncplay_binary: '',
	image_cache_cap_mb: 100,
	auto_play_next: false,
	download_bottom_bar_enabled: true,
	auto_skip_op: false,
	auto_skip_ed: false,
	use_custom_player_controls: true,
	auto_update_anicli: false,
	update_include_prereleases: false,
	primary_account: ''
};

const DETAIL = 'invalid type: null, expected a string at line 1 column 42';
const FAILURE = { kind: 'parse_failed', key: 'error.scraper.parse_failed', detail: DETAIL };

function failingSearch() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(CONFIG)),
		http.get(`${API_BASE}/api/kitsu/trending`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json(FAILURE, { status: 502 }))
	);
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setLocale('en', { reload: false });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	setLocale('en', { reload: false });
});

async function until<T>(read: () => T | null, what: string, timeoutMs = 8000): Promise<T> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const value = read();
		if (value !== null) return value;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

/** Mount the page on a failing query and return the alert's two lines. */
async function failureState(): Promise<{ headline: string; reason: string | null }> {
	failingSearch();
	setQuery('cowboy');
	app = mount(SearchPage, { target });
	const alert = await until(
		() => target.querySelector<HTMLElement>('[role="alert"]'),
		'the failure state'
	);
	return {
		headline: alert.querySelector('.state-headline')?.textContent?.trim() ?? '',
		reason: alert.querySelector('.state-detail')?.textContent?.trim() ?? null
	};
}

describe('/search failure copy', () => {
	it('gives the reason as user copy, never the payload detail or kind', async () => {
		const { reason } = await failureState();
		expect(reason).toBe(describeError(FAILURE));
		expect(target.textContent).not.toContain(DETAIL);
		expect(target.textContent).not.toContain('parse_failed');
	});

	it('localizes the headline', async () => {
		const english = (await failureState()).headline;
		unmount(app!);
		app = null;

		setLocale('pt-BR', { reload: false });
		const portuguese = (await failureState()).headline;

		expect(english).not.toBe('');
		expect(portuguese).not.toBe('');
		expect(portuguese).not.toBe(english);
	});
});
