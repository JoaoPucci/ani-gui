// Acceptance: changing the external player's type in Settings changes
// its command along with it while the command is the previous type's
// bare default ("mpv", "vlc", "iina"), and never touches a command the
// user wrote: a full path, a wrapper, a blank. Custom has no command of
// its own, so going to it keeps the command. Leaving it clears a
// command that is only a standard program's name ("mpv", "vlc",
// "iina") to blank, and a blank command runs the new type's own
// program. A command with a path or flags is kept. Changing the type
// guesses no path: where a program is installed is the backend's
// lookup, which these scenarios answer as "not found".
//
// What the command becomes is the backend's rule
// (`backend/src/commands/external_player_kind_change.rs`), asked over
// `/api/external-player/after-kind-change`. These scenarios answer it
// from the table below, the rule's answers for exactly the changes
// they make, and fail on a question the table does not hold; the rule
// itself is the backend's tests' to hold.

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
import type { AppInfo, Config, StandardProgram } from '../../src/lib/api';
import { appInfoAnswer, configAnswer } from '../answers';

const CONFIG: Config = configAnswer({
	locale: 'en',
	mode: 'sub',
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
});

const APP_INFO: AppInfo = appInfoAnswer({
	version: '0.12.0',
	history_path: '/home/u/.local/state/ani-gui/history',
	proxy_base_url: 'http://127.0.0.1:31337'
});

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let saved: Record<string, unknown>[];
let unanswered: string[];

/** The backend rule's answers, by `command|from|to`. */
const ANSWERS: Record<string, string> = {
	'mpv|mpv|vlc': 'vlc',
	'vlc|vlc|iina': 'iina',
	'iina|iina|mpv': 'mpv',
	'vlc|vlc|custom': 'vlc',
	'vlc|custom|mpv': '',
	'/opt/mpv/bin/mpv|custom|vlc': '/opt/mpv/bin/mpv',
	'mpv --fs|custom|vlc': 'mpv --fs',
	'flatpak run org.videolan.VLC|custom|vlc': 'flatpak run org.videolan.VLC',
	'/usr/bin/mpv|mpv|vlc': '/usr/bin/mpv',
	'C:\\Program Files\\VideoLAN\\VLC\\vlc.exe|mpv|vlc': 'C:\\Program Files\\VideoLAN\\VLC\\vlc.exe',
	'flatpak run io.mpv.Mpv|mpv|vlc': 'flatpak run io.mpv.Mpv',
	'|mpv|vlc': ''
};

function serve(over: Partial<typeof CONFIG> = {}) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json<Config>({ ...CONFIG, ...over })),
		http.put(`${API_BASE}/api/settings`, async ({ request }) => {
			saved.push((await request.json()) as Record<string, unknown>);
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json<AppInfo>(APP_INFO)),
		http.get(`${API_BASE}/api/external-player/after-kind-change`, ({ request }) => {
			const q = new URL(request.url).searchParams;
			const key = `${q.get('command')}|${q.get('from')}|${q.get('to')}`;
			if (!(key in ANSWERS)) {
				unanswered.push(key);
				return new HttpResponse(null, { status: 500 });
			}
			return HttpResponse.json<string>(ANSWERS[key]);
		}),
		// Where each type's program is installed: nowhere, here.
		http.get(`${API_BASE}/api/external-player/standard`, ({ request }) => {
			const kind = new URL(request.url).searchParams.get('kind');
			return HttpResponse.json<StandardProgram>({
				name: kind === 'custom' ? null : kind,
				path: null,
				supported: true
			});
		})
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	saved = [];
	unanswered = [];
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	expect(unanswered, 'a change the table does not answer').toEqual([]);
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

describe('settings — the backend decides what the command becomes', () => {
	it('saves the command the backend answers, whatever it is', async () => {
		await open();
		server.use(
			http.get(`${API_BASE}/api/external-player/after-kind-change`, () =>
				HttpResponse.json<string>('/from/the/backend/vlc')
			)
		);
		await choose(m.settings_external_player_kind_vlc());
		expect(command().value).toBe('/from/the/backend/vlc');
		expect(saved.at(-1)).toMatchObject({
			external_player_kind: 'vlc',
			external_player: '/from/the/backend/vlc'
		});
	});

	it('keeps the type and the command, and says so, when the backend cannot answer', async () => {
		await open();
		server.use(
			http.get(
				`${API_BASE}/api/external-player/after-kind-change`,
				() => new HttpResponse(null, { status: 500 })
			)
		);
		await choose(m.settings_external_player_kind_vlc());
		expect(command().value).toBe('mpv');
		expect(type(m.settings_external_player_kind_mpv()).getAttribute('aria-pressed')).toBe('true');
		expect(saved).toEqual([]);
		expect(target.querySelector('[role="alert"] .state-headline')?.textContent?.trim()).toBe(
			m.settings_error_save_headline()
		);
	});
});
