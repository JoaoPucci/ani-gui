// Acceptance: the rendered Settings page carries no trace of the
// retired script.
//
// The message-source sweep pins the localized copy, and this pins the
// delivery: the About section once carried a "Built atop" row whose
// link target was hardcoded markup, not a message — a shape the copy
// sweep cannot see, and one the lint allowlists (attributes are
// exempt from the no-hardcoded-strings rule). Only the mounted route
// shows what a user actually meets.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

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

import SettingsPage from '../../src/routes/settings/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const CONFIG = {
	locale: 'en',
	mode: 'sub',
	quality: 'best',
	external_player: '',
	external_player_kind: 'mpv',
	external_player_custom_args: '',
	syncplay_binary: '',
	image_cache_cap_mb: 512,
	auto_play_next: true,
	download_bottom_bar_enabled: true,
	auto_skip_op: false,
	auto_skip_ed: false,
	use_custom_player_controls: true,
	disable_auto_pip_on_leave: false,
	update_include_prereleases: false,
	primary_account: ''
};

const APP_INFO = {
	version: '0.12.0',
	history_path: '/home/u/.local/state/ani-gui/history',
	proxy_base_url: 'http://127.0.0.1:31337',
	removed_legacy_paths: []
};

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(CONFIG)),
		http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json(APP_INFO))
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function settle() {
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
}

describe('settings — the About section', () => {
	it('renders without naming the retired script anywhere', async () => {
		app = mount(SettingsPage, { target });
		await settle();

		// The page loaded for real — the version from app-info is on
		// screen — so the absence below is an answer about a rendered
		// section, not about a failed fetch.
		expect(target.textContent).toContain(APP_INFO.version);

		expect(target.textContent).not.toMatch(/ani-cli|pystardust/i);
		const external = Array.from(target.querySelectorAll('a[href]'))
			.map((a) => a.getAttribute('href') ?? '')
			.filter((h) => /ani-cli|pystardust/i.test(h));
		expect(external, `links reaching the retired script: ${external.join(', ')}`).toEqual([]);
	});
});
