// Acceptance: the Settings hint under "External player" quotes the
// button the play page shows, and that button names the configured
// player flavour. Switching the player flavour changes the quoted button with
// it, so the hint never names a button the user cannot find.
//
// A blank command runs the flavour's standard program (mpv, vlc,
// iina), so the hint and the field's greyed placeholder name that
// program, not mpv for every flavour. Custom has no standard program,
// and says so.

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
		http.put(`${API_BASE}/api/settings`, () => new HttpResponse(null, { status: 204 })),
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

const hint = () =>
	Array.from(target.querySelectorAll('.field-hint')).find((h) =>
		(h.textContent ?? '').includes('Command that')
	)?.textContent ?? '';

const field = () =>
	target.querySelector('input[aria-label="External player"]') as HTMLInputElement | null;

const flavour = (label: string) =>
	Array.from(target.querySelectorAll('button[aria-pressed]')).find(
		(b) => (b.textContent ?? '').trim() === label
	) as HTMLButtonElement | undefined;

describe('settings — the external player hint', () => {
	it('quotes the button for the configured player and follows a flavour switch', async () => {
		app = mount(SettingsPage, { target });
		await settle();
		expect(hint()).toContain('"Open in mpv"');

		flavour('VLC')!.click();
		await settle();
		expect(hint()).toContain('"Open in VLC"');

		flavour('Custom')!.click();
		await settle();
		expect(hint()).toContain('"Open in external player"');
		expect(hint()).not.toContain('Open in VLC');
	});

	it("names the flavour's own program in the hint and the blank field", async () => {
		app = mount(SettingsPage, { target });
		await settle();
		expect(field()!.value).toBe('');
		expect(hint()).toContain('Leave it blank to use mpv.');
		expect(field()!.placeholder).toBe('mpv');

		for (const [label, program] of [
			['VLC', 'vlc'],
			['IINA', 'iina'],
			['mpv', 'mpv']
		]) {
			flavour(label)!.click();
			await settle();
			expect(hint(), label).toContain(`Leave it blank to use ${program}.`);
			expect(field()!.placeholder, label).toBe(program);
		}

		flavour('Custom')!.click();
		await settle();
		expect(hint()).not.toContain('Leave it blank');
		expect(hint()).toContain('Custom has no standard program');
		expect(field()!.placeholder).toBe('');
	});
});
