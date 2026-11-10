// Acceptance: changing the external player's type in Settings changes
// its command along with it while the command is the previous type's
// bare default ("mpv", "vlc", "iina"), and never touches a command the
// user wrote: a full path, a wrapper, a blank. Custom has no command of
// its own, so going to it keeps the command. Leaving it clears a
// command that is only a standard program's name ("mpv", "vlc",
// "iina") to blank, and a blank command runs the new type's own
// program. A command with a path or flags is kept. No path is guessed
// and nothing is looked up: on Windows a bare `vlc` may not be found,
// and the user browses to it, as before.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { m } from '../../src/lib/paraglide/messages';

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
	external_player: 'mpv',
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
let saved: Record<string, unknown>[];

function serve(over: Partial<typeof CONFIG> = {}) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json({ ...CONFIG, ...over })),
		http.put(`${API_BASE}/api/settings`, async ({ request }) => {
			saved.push((await request.json()) as Record<string, unknown>);
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json(APP_INFO))
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	saved = [];
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

const command = () =>
	target.querySelector<HTMLInputElement>(
		`input[aria-label="${m.settings_field_external_player_key()}"]`
	)!;

const type = (label: string) =>
	Array.from(target.querySelectorAll<HTMLButtonElement>('button[aria-pressed]')).find(
		(b) => (b.textContent ?? '').trim() === label
	)!;

async function choose(label: string) {
	type(label).click();
	await settle();
}

async function open(over: Partial<typeof CONFIG> = {}) {
	serve(over);
	app = mount(SettingsPage, { target });
	await settle();
}

describe('settings — the external player command follows its type', () => {
	it("swaps the previous type's bare command for the new one's, and saves both together", async () => {
		await open();
		expect(command().value).toBe('mpv');
		await choose(m.settings_external_player_kind_vlc());
		expect(command().value).toBe('vlc');
		expect(saved.at(-1)).toMatchObject({ external_player_kind: 'vlc', external_player: 'vlc' });
		await choose(m.settings_external_player_kind_iina());
		expect(command().value).toBe('iina');
		expect(saved.at(-1)).toMatchObject({ external_player_kind: 'iina', external_player: 'iina' });
		await choose(m.settings_external_player_kind_mpv());
		expect(command().value).toBe('mpv');
	});

	it('keeps the command going to Custom, and clears a bare name coming back from it', async () => {
		await open({ external_player: 'vlc', external_player_kind: 'vlc' });
		await choose(m.settings_external_player_kind_custom());
		expect(command().value).toBe('vlc');
		expect(saved.at(-1)).toMatchObject({ external_player_kind: 'custom', external_player: 'vlc' });
		// Leaving Custom: "vlc" is only a standard program's name, so it
		// clears, and the blank field runs mpv, the new type's program.
		await choose(m.settings_external_player_kind_mpv());
		expect(command().value).toBe('');
		expect(command().placeholder).toBe('mpv');
		expect(saved.at(-1)).toMatchObject({ external_player_kind: 'mpv', external_player: '' });
	});

	it('keeps a command with a path or flags when leaving Custom', async () => {
		for (const written of ['/opt/mpv/bin/mpv', 'mpv --fs', 'flatpak run org.videolan.VLC']) {
			await open({ external_player: written, external_player_kind: 'custom' });
			await choose(m.settings_external_player_kind_vlc());
			expect(command().value).toBe(written);
			expect(saved.at(-1)).toMatchObject({ external_player_kind: 'vlc', external_player: written });
			unmount(app!);
			app = null;
		}
	});

	it('never touches a full path, a wrapper or a blank', async () => {
		for (const written of [
			'/usr/bin/mpv',
			'C:\\Program Files\\VideoLAN\\VLC\\vlc.exe',
			'flatpak run io.mpv.Mpv',
			''
		]) {
			await open({ external_player: written, external_player_kind: 'mpv' });
			await choose(m.settings_external_player_kind_vlc());
			expect(command().value).toBe(written);
			expect(saved.at(-1)).toMatchObject({ external_player_kind: 'vlc', external_player: written });
			unmount(app!);
			app = null;
		}
	});

	it('swaps a bare command the user typed in, once it is saved', async () => {
		await open({ external_player: '/usr/bin/mpv', external_player_kind: 'mpv' });
		const input = command();
		input.value = 'mpv';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		await choose(m.settings_external_player_kind_vlc());
		expect(command().value).toBe('vlc');
		expect(saved.at(-1)).toMatchObject({ external_player_kind: 'vlc', external_player: 'vlc' });
	});
});
